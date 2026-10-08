//! Schema-aware compilation with explicit relational stages.
//!
//! The legacy API remains available for compilation without source metadata.
//! This path binds every reference before rendering SQL and never substitutes
//! a computed expression for a reference to an earlier stage.

mod across;
mod bindings;
mod discovery;
mod distinct_window;
mod extended;
mod join;
mod native;
mod rows;
pub mod schema;
mod selection;
mod slice;
mod sql;
mod statistics;
mod tidyr;

pub use schema::{SchemaColumn, SchemaInput, SourceSchema};

use crate::error::{GenerationError, GenerationResult};
use crate::parser::{
    BinaryOp, DplyrNode, DplyrOperation, Expr, JoinType, LiteralValue, OrderDirection, OrderExpr,
    SetOperation, SliceSpec, UnaryOp,
};
use crate::sql_generator::SqlGenerator;
use sql::{SelectExpression, SelectItem, SqlQuery, SqlSource};

/// SQL and the ordered, visible output schema of a compiled pipeline.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CompiledQuery {
    pub sql: String,
    pub columns: Vec<SchemaColumn>,
    /// Number of SELECT stages before database optimization.
    pub stages: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ColumnId(usize);

#[derive(Clone)]
struct Column {
    id: ColumnId,
    schema: SchemaColumn,
    hidden: bool,
}

#[derive(Clone)]
enum BoundExpr {
    Column(ColumnId),
    Literal(LiteralValue),
    Unary(UnaryOp, Box<BoundExpr>),
    In(Box<BoundExpr>, Vec<LiteralValue>),
    Binary(Box<BoundExpr>, BinaryOp, Box<BoundExpr>),
    Function(String, Vec<BoundExpr>),
    CaseWhen(Vec<(BoundExpr, BoundExpr)>, Option<Box<BoundExpr>>),
    NamedArg(String, Box<BoundExpr>),
}

#[derive(Clone)]
struct Projection {
    column: Column,
    expression: BoundExpr,
}

#[derive(Clone)]
struct Measure {
    column: Column,
    expression: BoundExpr,
}

#[derive(Clone)]
struct JoinProjection {
    column: Column,
    left: Option<ColumnId>,
    right: Option<ColumnId>,
}

#[derive(Clone)]
struct Relation {
    node: RelNode,
    columns: Vec<Column>,
    groups: Vec<ColumnId>,
    order: Vec<(ColumnId, OrderDirection)>,
    frame: Option<(i64, i64)>,
}

#[derive(Clone)]
enum RelNode {
    Scan(String),
    Query(Box<SqlQuery>),
    Project {
        input: Box<Relation>,
        items: Vec<Projection>,
        partition: Vec<ColumnId>,
        frame: Option<(i64, i64)>,
    },
    Filter {
        input: Box<Relation>,
        predicate: BoundExpr,
    },
    Aggregate {
        input: Box<Relation>,
        keys: Vec<ColumnId>,
        measures: Vec<Measure>,
    },
    Distinct(Box<Relation>),
    Slice {
        input: Box<Relation>,
        spec: SliceSpec,
        by: Vec<ColumnId>,
    },
    Join {
        left: Box<Relation>,
        right: Box<Relation>,
        join_type: JoinType,
        keys: Vec<(ColumnId, ColumnId)>,
        predicates: Vec<(ColumnId, BinaryOp, ColumnId)>,
        closest: Option<(ColumnId, BinaryOp, ColumnId)>,
        items: Vec<JoinProjection>,
        na_matches: bool,
    },
    Set {
        left: Box<Relation>,
        right: Box<Relation>,
        operation: SetOperation,
    },
}

struct Planner<'a> {
    generator: &'a SqlGenerator,
    schemas: &'a [SourceSchema],
    next_id: usize,
    stages: usize,
    checks: Option<Vec<crate::execution::ValidationQuery>>,
    bindings: &'a std::collections::HashMap<String, serde_json::Value>,
}

fn invalid(reason: impl Into<String>) -> GenerationError {
    GenerationError::InvalidAst {
        reason: reason.into(),
    }
}

fn add_suffixes(names: Vec<String>, reserved: &[String], suffix: &str) -> Vec<String> {
    let mut output = names;
    loop {
        let mut seen = reserved
            .iter()
            .cloned()
            .collect::<std::collections::HashSet<_>>();
        let mut changed = false;
        for name in &mut output {
            if !seen.insert(name.clone()) {
                name.push_str(suffix);
                changed = true;
            }
        }
        if !changed {
            return output;
        }
    }
}

fn visible_column<'a>(columns: &'a [Column], name: &str) -> GenerationResult<&'a Column> {
    columns
        .iter()
        .find(|column| !column.hidden && column.schema.name == name)
        .ok_or_else(|| GenerationError::InvalidColumnReference {
            column: name.to_string(),
            table: None,
        })
}

fn column_by_id(columns: &[Column], id: ColumnId) -> GenerationResult<&Column> {
    columns
        .iter()
        .find(|column| column.id == id)
        .ok_or_else(|| invalid("a bound column is missing from the input relation"))
}

impl BoundExpr {
    fn bind(expr: &Expr, columns: &[Column]) -> GenerationResult<Self> {
        Ok(match expr {
            Expr::Identifier(name) => Self::Column(visible_column(columns, name)?.id),
            Expr::Literal(value) => Self::Literal(value.clone()),
            Expr::Unary { operator, expr } => {
                Self::Unary(operator.clone(), Box::new(Self::bind(expr, columns)?))
            }
            // `values` are constants, so only the operand can reference columns.
            Expr::In { expr, values } => {
                Self::In(Box::new(Self::bind(expr, columns)?), values.clone())
            }
            Expr::Binary {
                left,
                operator,
                right,
            } => Self::Binary(
                Box::new(Self::bind(left, columns)?),
                operator.clone(),
                Box::new(Self::bind(right, columns)?),
            ),
            Expr::Function { name, args } => Self::Function(
                name.clone(),
                args.iter()
                    .map(|arg| Self::bind(arg, columns))
                    .collect::<GenerationResult<_>>()?,
            ),
            Expr::CaseWhen { branches, default } => Self::CaseWhen(
                branches
                    .iter()
                    .map(|(condition, value)| {
                        Ok((Self::bind(condition, columns)?, Self::bind(value, columns)?))
                    })
                    .collect::<GenerationResult<_>>()?,
                default
                    .as_ref()
                    .map(|expr| Self::bind(expr, columns).map(Box::new))
                    .transpose()?,
            ),
            Expr::NamedArg { name, value } => {
                Self::NamedArg(name.clone(), Box::new(Self::bind(value, columns)?))
            }
        })
    }

    fn to_expr(&self, columns: &[Column]) -> GenerationResult<Expr> {
        Ok(match self {
            Self::Column(id) => Expr::Identifier(column_by_id(columns, *id)?.schema.name.clone()),
            Self::Literal(value) => Expr::Literal(value.clone()),
            Self::Unary(operator, expr) => Expr::Unary {
                operator: operator.clone(),
                expr: Box::new(expr.to_expr(columns)?),
            },
            Self::In(expr, values) => Expr::In {
                expr: Box::new(expr.to_expr(columns)?),
                values: values.clone(),
            },
            Self::Binary(left, operator, right) => Expr::Binary {
                left: Box::new(left.to_expr(columns)?),
                operator: operator.clone(),
                right: Box::new(right.to_expr(columns)?),
            },
            Self::Function(name, args) => Expr::Function {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|arg| arg.to_expr(columns))
                    .collect::<GenerationResult<_>>()?,
            },
            Self::CaseWhen(branches, default) => Expr::CaseWhen {
                branches: branches
                    .iter()
                    .map(|(condition, value)| {
                        Ok((condition.to_expr(columns)?, value.to_expr(columns)?))
                    })
                    .collect::<GenerationResult<_>>()?,
                default: default
                    .as_ref()
                    .map(|expr| expr.to_expr(columns).map(Box::new))
                    .transpose()?,
            },
            Self::NamedArg(name, value) => Expr::NamedArg {
                name: name.clone(),
                value: Box::new(value.to_expr(columns)?),
            },
        })
    }
}

impl Planner<'_> {
    fn scan(&mut self, source: &str) -> GenerationResult<Relation> {
        let schema = self
            .schemas
            .iter()
            .find(|schema| schema.source == source)
            .ok_or_else(|| invalid(format!("schema for source '{source}' is missing")))?;
        let metadata = schema.columns.clone();
        let columns = metadata
            .into_iter()
            .map(|metadata| {
                let mut column = self.new_column(&metadata.name);
                column.schema = metadata;
                column
            })
            .collect();
        Ok(Relation {
            node: RelNode::Scan(source.to_string()),
            columns,
            groups: Vec::new(),
            order: Vec::new(),
            frame: None,
        })
    }

    fn new_column(&mut self, name: &str) -> Column {
        let column = Column {
            id: ColumnId(self.next_id),
            schema: SchemaColumn::new(name),
            hidden: false,
        };
        self.next_id += 1;
        column
    }

    fn stage(&mut self) -> GenerationResult<()> {
        self.stages += 1;
        // ponytail: conservative stage-per-operation lowering; merge only after equivalence tests.
        if self.stages > 64 {
            return Err(GenerationError::MaxNestingDepthExceeded {
                depth: self.stages,
                max_depth: 64,
            });
        }
        Ok(())
    }

    fn hidden_name(&mut self, columns: &[Column], purpose: &str) -> String {
        loop {
            let name = format!("__libdplyr_{purpose}_{}", self.next_id);
            self.next_id += 1;
            if !columns.iter().any(|column| column.schema.name == name) {
                return name;
            }
        }
    }

    fn project(
        &mut self,
        input: Relation,
        mut items: Vec<Projection>,
    ) -> GenerationResult<Relation> {
        if items.is_empty() {
            return Err(GenerationError::EmptyQuery);
        }
        let mut names = std::collections::HashSet::new();
        if items
            .iter()
            .any(|item| !names.insert(item.column.schema.name.clone()))
        {
            return Err(invalid("projection contains duplicate output names"));
        }
        // Keep an earlier sort value when select/rename/mutate changes its public name.
        for (id, _) in &input.order {
            if !items.iter().any(|item| item.column.id == *id) {
                let mut column = column_by_id(&input.columns, *id)?.clone();
                column.hidden = true;
                let output = items
                    .iter()
                    .map(|item| item.column.clone())
                    .collect::<Vec<_>>();
                column.schema.name = self.hidden_name(&output, "order");
                items.push(Projection {
                    column,
                    expression: BoundExpr::Column(*id),
                });
            }
        }
        let columns = items.iter().map(|item| item.column.clone()).collect();
        let groups = input.groups.clone();
        let order = input.order.clone();
        let partition = groups.clone();
        let frame = input.frame;
        self.stage()?;
        Ok(Relation {
            node: RelNode::Project {
                input: Box::new(input),
                items,
                partition,
                frame,
            },
            columns,
            groups,
            order,
            frame,
        })
    }

    fn identities(columns: &[Column]) -> Vec<Projection> {
        columns
            .iter()
            .map(|column| Projection {
                column: column.clone(),
                expression: BoundExpr::Column(column.id),
            })
            .collect()
    }

    fn is_window(&self, name: &str) -> bool {
        self.generator
            .dialect()
            .translate_aggregate_function(name)
            .is_some()
            || matches!(
                name.to_ascii_lowercase().as_str(),
                "n" | "n_distinct"
                    | "row_number"
                    | "rank"
                    | "dense_rank"
                    | "ntile"
                    | "lead"
                    | "lag"
                    | "first"
                    | "first_value"
                    | "last"
                    | "last_value"
                    | "nth_value"
                    | "min_rank"
                    | "percent_rank"
                    | "cume_dist"
                    | "cumsum"
                    | "cummean"
                    | "cummin"
                    | "cummax"
            )
    }

    fn has_window(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Function { name, args } => {
                self.is_window(name) || args.iter().any(|arg| self.has_window(arg))
            }
            Expr::Binary { left, right, .. } => self.has_window(left) || self.has_window(right),
            Expr::Unary { expr, .. } => self.has_window(expr),
            Expr::In { expr, .. } => self.has_window(expr),
            Expr::CaseWhen { branches, default } => {
                branches
                    .iter()
                    .any(|(a, b)| self.has_window(a) || self.has_window(b))
                    || default.as_ref().is_some_and(|expr| self.has_window(expr))
            }
            Expr::NamedArg { value, .. } => self.has_window(value),
            _ => false,
        }
    }

    fn validate_expression(&self, expr: &Expr) -> GenerationResult<()> {
        match expr {
            Expr::Function { name, args } => {
                if name.eq_ignore_ascii_case("n_distinct") {
                    return Err(self.unsupported("n_distinct() as a window expression"));
                }
                if self.is_window(name) && args.iter().any(|arg| self.has_window(arg)) {
                    return Err(invalid(
                        "window expressions cannot contain another window expression",
                    ));
                }
                for arg in args {
                    self.validate_expression(arg)?;
                }
            }
            Expr::Binary { left, right, .. } => {
                self.validate_expression(left)?;
                self.validate_expression(right)?;
            }
            Expr::Unary { expr, .. } => self.validate_expression(expr)?,
            Expr::In { expr, .. } => self.validate_expression(expr)?,
            Expr::CaseWhen { branches, default } => {
                for (a, b) in branches {
                    self.validate_expression(a)?;
                    self.validate_expression(b)?;
                }
                if let Some(expr) = default {
                    self.validate_expression(expr)?;
                }
            }
            Expr::NamedArg { value, .. } => self.validate_expression(value)?,
            _ => {}
        }
        Ok(())
    }

    fn unsupported(&self, operation: &str) -> GenerationError {
        GenerationError::UnsupportedOperation {
            operation: format!("schema-aware {operation}"),
            dialect: self.generator.dialect().dialect_name().to_string(),
        }
    }

    fn validate_summary(
        &self,
        expr: &Expr,
        input: &Relation,
        inside_aggregate: bool,
    ) -> GenerationResult<()> {
        match expr {
            Expr::Identifier(name) => {
                let column = visible_column(&input.columns, name)?;
                if !inside_aggregate && !input.groups.contains(&column.id) {
                    return Err(invalid(format!(
                        "summary column '{name}' must be grouped or aggregated"
                    )));
                }
            }
            Expr::Function { name, args } => {
                let aggregate = self
                    .generator
                    .dialect()
                    .translate_aggregate_function(name)
                    .is_some()
                    || name.eq_ignore_ascii_case("n")
                    || name.eq_ignore_ascii_case("n_distinct");
                if self.is_window(name) && !aggregate {
                    return Err(self.unsupported("window functions inside summarise()"));
                }
                if aggregate && inside_aggregate {
                    return Err(invalid(
                        "aggregate expressions cannot contain another aggregate",
                    ));
                }
                for arg in args {
                    self.validate_summary(arg, input, inside_aggregate || aggregate)?;
                }
            }
            Expr::Binary { left, right, .. } => {
                self.validate_summary(left, input, inside_aggregate)?;
                self.validate_summary(right, input, inside_aggregate)?;
            }
            Expr::Unary { expr, .. } => self.validate_summary(expr, input, inside_aggregate)?,
            Expr::In { expr, .. } => self.validate_summary(expr, input, inside_aggregate)?,
            Expr::CaseWhen { branches, default } => {
                for (condition, value) in branches {
                    self.validate_summary(condition, input, inside_aggregate)?;
                    self.validate_summary(value, input, inside_aggregate)?;
                }
                if let Some(value) = default {
                    self.validate_summary(value, input, inside_aggregate)?;
                }
            }
            Expr::NamedArg { value, .. } => {
                self.validate_summary(value, input, inside_aggregate)?
            }
            Expr::Literal(_) => {}
        }
        Ok(())
    }

    fn summarise(
        &mut self,
        mut input: Relation,
        assignments: Vec<(String, Expr)>,
    ) -> GenerationResult<Relation> {
        let keys = input.groups.clone();
        let mut columns = keys
            .iter()
            .map(|id| column_by_id(&input.columns, *id).cloned())
            .collect::<GenerationResult<Vec<_>>>()?;
        let mut measures = Vec::new();
        let mut has_aggregate = false;
        for (name, expr) in assignments {
            let expr = self.resolve_expression(&expr, &input)?;
            let (next, expr) = self.summary_statistics(input, &expr)?;
            input = next;
            self.validate_summary(&expr, &input, false)?;
            has_aggregate |= self.has_window(&expr);
            let expression = BoundExpr::bind(&expr, &input.columns)?;
            let column = self.new_column(&name);
            columns.push(column.clone());
            measures.push(Measure { column, expression });
        }
        if keys.is_empty() && !has_aggregate {
            // Force one global summary row, including for an empty input.
            let name = self.hidden_name(&columns, "summary");
            let mut column = self.new_column(&name);
            column.hidden = true;
            columns.push(column.clone());
            measures.push(Measure {
                column,
                expression: BoundExpr::Function("n".to_string(), Vec::new()),
            });
        }
        self.validate_output(&columns)?;
        self.stage()?;
        let groups = keys
            .iter()
            .take(keys.len().saturating_sub(1))
            .copied()
            .collect();
        let result = Relation {
            node: RelNode::Aggregate {
                input: Box::new(input),
                keys,
                measures,
            },
            columns,
            groups,
            order: Vec::new(),
            frame: None,
        };
        if result.columns.iter().any(|column| column.hidden) {
            let visible = result
                .columns
                .iter()
                .filter(|column| !column.hidden)
                .cloned()
                .collect::<Vec<_>>();
            self.project(result, Self::identities(&visible))
        } else {
            Ok(result)
        }
    }

    fn set(
        &mut self,
        left: Relation,
        operation: &SetOperation,
        right_table: &str,
    ) -> GenerationResult<Relation> {
        let right = self.scan(right_table)?;
        self.set_relations(left, right, operation)
    }

    fn across_schema(&self, input: &Relation) -> Vec<SchemaColumn> {
        input
            .columns
            .iter()
            .filter(|column| !column.hidden && !input.groups.contains(&column.id))
            .map(|column| column.schema.clone())
            .collect()
    }

    // R3-AC1: Every expression in one across() reads the same input stage.
    fn mutate_assignments(
        &mut self,
        mut input: Relation,
        assignments: &[crate::parser::Assignment],
    ) -> GenerationResult<Relation> {
        if assignments.is_empty() {
            return Ok(input);
        }
        let mut prepared = Vec::new();
        let mut hidden = Vec::new();
        for original in assignments {
            let bound = self.resolve_expression(&original.expr, &input)?;
            let expanded = self.expand_predicates(&bound, &input)?;
            let original_ids = input.columns.iter().map(|c| c.id).collect::<Vec<_>>();
            if input.frame.is_some()
                && statistics::needs_stage(&expanded, self.generator.dialect().dialect_name())
            {
                return Err(self.unsupported("portable statistics with a moving window frame"));
            }
            let (next, expanded) = self.summary_statistics(input, &expanded)?;
            input = next;
            hidden.extend(
                input
                    .columns
                    .iter()
                    .filter(|c| !original_ids.contains(&c.id))
                    .map(|c| c.id),
            );
            let (next, expr, ids) = self.distinct_windows(input, &expanded)?;
            input = next;
            hidden.extend(ids);
            prepared.push(crate::parser::Assignment {
                column: original.column.clone(),
                expr,
            });
        }
        let mut items = Self::identities(&input.columns);
        for assignment in prepared {
            if matches!(assignment.expr, Expr::Literal(LiteralValue::Null)) {
                if let Some(column) = input
                    .columns
                    .iter()
                    .find(|c| c.schema.name == assignment.column)
                {
                    if input.groups.contains(&column.id) {
                        return Err(invalid("cannot delete a grouping column"));
                    }
                }
                items.retain(|item| item.column.schema.name != assignment.column);
                continue;
            }
            self.validate_expression(&assignment.expr)?;
            let expression = BoundExpr::bind(&assignment.expr, &input.columns)?;
            let mut column = self.new_column(&assignment.column);
            if let Expr::Identifier(name) = &assignment.expr {
                let source = visible_column(&input.columns, name)?;
                column.schema.data_type.clone_from(&source.schema.data_type);
                column.schema.nullable = source.schema.nullable;
            }
            let projection = Projection { column, expression };
            if let Some(index) = items.iter().position(|item| {
                !item.column.hidden && item.column.schema.name == assignment.column
            }) {
                items[index] = projection;
            } else {
                items.push(projection);
            }
        }
        for item in &mut items {
            if hidden.contains(&item.column.id) {
                item.column.hidden = true;
            }
        }
        let group_names = names_for_ids(&input.columns, &input.groups)?;
        let mut result = self.project(input, items)?;
        result.groups = group_names
            .iter()
            .map(|name| visible_column(&result.columns, name).map(|column| column.id))
            .collect::<GenerationResult<_>>()?;
        Ok(result)
    }

    fn apply(
        &mut self,
        mut input: Relation,
        operation: &DplyrOperation,
    ) -> GenerationResult<Relation> {
        match operation {
            DplyrOperation::Extended { name, args, .. } => self.extended(input, name, args),
            DplyrOperation::Select { columns, .. } => {
                let columns = columns
                    .iter()
                    .map(|column| {
                        Ok(crate::parser::ColumnExpr {
                            expr: self.resolve_expression(&column.expr, &input)?,
                            alias: column.alias.clone(),
                        })
                    })
                    .collect::<GenerationResult<Vec<_>>>()?;
                let columns = &columns;
                let schema = input
                    .columns
                    .iter()
                    .filter(|column| !column.hidden)
                    .map(|column| column.schema.clone())
                    .collect::<Vec<_>>();
                let selected = selection::resolve(columns, &schema)?;
                let mut items = Vec::new();
                for selected in selected {
                    let Expr::Identifier(name) = &selected.expr else {
                        return Err(self.unsupported("computed select(); use mutate()"));
                    };
                    let mut column = visible_column(&input.columns, name)?.clone();
                    if let Some(alias) = &selected.alias {
                        column.schema.name.clone_from(alias);
                    }
                    items.push(Projection {
                        expression: BoundExpr::Column(column.id),
                        column,
                    });
                }
                let mut missing_groups = Vec::new();
                for id in &input.groups {
                    if !items.iter().any(|item| item.column.id == *id) {
                        missing_groups.push(Projection {
                            column: column_by_id(&input.columns, *id)?.clone(),
                            expression: BoundExpr::Column(*id),
                        });
                    }
                }
                missing_groups.extend(items);
                self.project(input, missing_groups)
            }
            DplyrOperation::Mutate { assignments, .. } => {
                for assignment in assignments {
                    let expanded = if assignment.column.is_empty() {
                        across::expand(&assignment.expr, &self.across_schema(&input))?
                    } else {
                        vec![assignment.clone()]
                    };
                    input = self.mutate_assignments(input, &expanded)?;
                }
                Ok(input)
            }
            DplyrOperation::Rename { renames, .. } => {
                let mut items = Self::identities(&input.columns);
                let mut seen = std::collections::HashSet::new();
                for rename in renames {
                    let source = visible_column(&input.columns, &rename.old_name)?;
                    if !seen.insert(source.id.0) {
                        return Err(invalid(
                            "rename() refers to the same input column more than once",
                        ));
                    }
                    let item = items
                        .iter_mut()
                        .find(|item| item.column.id == source.id)
                        .ok_or_else(|| invalid("missing rename input"))?;
                    item.column.schema.name.clone_from(&rename.new_name);
                }
                self.project(input, items)
            }
            DplyrOperation::Filter { condition, .. } => {
                let bound = self.resolve_expression(condition, &input)?;
                let condition = self.expand_predicates(&bound, &input)?;
                let original_visible = input
                    .columns
                    .iter()
                    .filter(|c| !c.hidden)
                    .cloned()
                    .collect::<Vec<_>>();
                let (next, condition, distinct_hidden) =
                    self.distinct_windows(input, &condition)?;
                input = next;
                let condition = &condition;
                self.validate_expression(condition)?;
                let mut predicate = BoundExpr::bind(condition, &input.columns)?;
                let visible = input
                    .columns
                    .iter()
                    .filter(|column| !column.hidden)
                    .cloned()
                    .collect::<Vec<_>>();
                if self.has_window(condition) {
                    let name = self.hidden_name(&input.columns, "filter");
                    let mut column = self.new_column(&name);
                    column.hidden = true;
                    let id = column.id;
                    let mut items = Self::identities(&input.columns);
                    items.push(Projection {
                        column,
                        expression: predicate,
                    });
                    input = self.project(input, items)?;
                    predicate = BoundExpr::Column(id);
                }
                self.stage()?;
                let mut result = Relation {
                    columns: input.columns.clone(),
                    groups: input.groups.clone(),
                    order: input.order.clone(),
                    frame: input.frame,
                    node: RelNode::Filter {
                        input: Box::new(input),
                        predicate,
                    },
                };
                if self.has_window(condition) {
                    let columns = if distinct_hidden.is_empty() {
                        &visible
                    } else {
                        &original_visible
                    };
                    result = self.project(result, Self::identities(columns))?;
                }
                Ok(result)
            }
            DplyrOperation::GroupBy { columns, .. } => {
                input.groups.clear();
                for name in columns {
                    let id = visible_column(&input.columns, name)?.id;
                    if !input.groups.contains(&id) {
                        input.groups.push(id);
                    }
                }
                Ok(input)
            }
            DplyrOperation::Ungroup { .. } => {
                // Clears the current grouping only. No SQL stage: an earlier
                // summarise already baked its GROUP BY into its own subquery.
                input.groups.clear();
                Ok(input)
            }
            DplyrOperation::Arrange { columns, .. } => {
                input.order = columns
                    .iter()
                    .map(|item| {
                        Ok((
                            visible_column(&input.columns, &item.column)?.id,
                            item.direction.clone(),
                        ))
                    })
                    .collect::<GenerationResult<_>>()?;
                Ok(input)
            }
            DplyrOperation::Summarise { aggregations, .. } => {
                let assignments = aggregations
                    .iter()
                    .map(|aggregation| {
                        let name = aggregation.alias.clone().unwrap_or_else(|| {
                            format!("{}({})", aggregation.function, aggregation.column)
                        });
                        let args = if aggregation.column.is_empty() {
                            Vec::new()
                        } else {
                            vec![Expr::Identifier(aggregation.column.clone())]
                        };
                        (
                            name,
                            Expr::Function {
                                name: aggregation.function.clone(),
                                args,
                            },
                        )
                    })
                    .collect();
                self.summarise(input, assignments)
            }
            DplyrOperation::SummariseExpressions { assignments, .. } => {
                let mut expanded = Vec::new();
                for assignment in assignments {
                    if assignment.column.is_empty() {
                        expanded.extend(across::expand(
                            &assignment.expr,
                            &self.across_schema(&input),
                        )?);
                    } else {
                        expanded.push(assignment.clone());
                    }
                }
                self.summarise(
                    input,
                    expanded
                        .into_iter()
                        .map(|assignment| (assignment.column, assignment.expr))
                        .collect(),
                )
            }
            DplyrOperation::Slice { spec, .. } => self.slice(input, spec),
            DplyrOperation::Count {
                columns: count_columns,
                ..
            } => {
                let original_groups = input.groups.clone();
                let mut keys = original_groups.clone();
                for name in count_columns {
                    let id = visible_column(&input.columns, name)?.id;
                    if !keys.contains(&id) {
                        keys.push(id);
                    }
                }
                let mut columns = keys
                    .iter()
                    .map(|id| column_by_id(&input.columns, *id).cloned())
                    .collect::<GenerationResult<Vec<_>>>()?;
                let mut name = "n".to_string();
                while columns.iter().any(|column| column.schema.name == name) {
                    name.push('n');
                }
                let mut column = self.new_column(&name);
                column.schema.nullable = Some(false);
                columns.push(column.clone());
                let measures = vec![Measure {
                    column,
                    expression: BoundExpr::Function("n".to_string(), Vec::new()),
                }];
                self.stage()?;
                Ok(Relation {
                    node: RelNode::Aggregate {
                        input: Box::new(input),
                        keys,
                        measures,
                    },
                    columns,
                    groups: original_groups,
                    order: Vec::new(),
                    frame: None,
                })
            }
            DplyrOperation::Distinct { columns, .. } => {
                if !columns.is_empty() {
                    let mut ids = input.groups.clone();
                    for name in columns {
                        let id = visible_column(&input.columns, name)?.id;
                        if !ids.contains(&id) {
                            ids.push(id);
                        }
                    }
                    let selected = ids
                        .iter()
                        .map(|id| column_by_id(&input.columns, *id).cloned())
                        .collect::<GenerationResult<Vec<_>>>()?;
                    input = self.project(input, Self::identities(&selected))?;
                }
                if input.columns.iter().any(|column| column.hidden) {
                    return Err(
                        self.unsupported("distinct() with a removed or overwritten sort key")
                    );
                }
                self.stage()?;
                Ok(Relation {
                    columns: input.columns.clone(),
                    groups: input.groups.clone(),
                    order: input.order.clone(),
                    frame: input.frame,
                    node: RelNode::Distinct(Box::new(input)),
                })
            }
            DplyrOperation::Join {
                join_type, spec, ..
            } => self.join(input, join_type, spec),
            DplyrOperation::SetOp {
                operation,
                right_table,
                ..
            } => self.set(input, operation, right_table),
        }
    }

    fn resolve_expression(&self, expr: &Expr, input: &Relation) -> GenerationResult<Expr> {
        let columns = input
            .columns
            .iter()
            .filter(|c| !c.hidden)
            .map(|c| c.schema.clone())
            .collect::<Vec<_>>();
        let expr = native::expand(expr)?;
        bindings::resolve(&expr, &columns, self.bindings)
    }

    fn validate_output(&self, columns: &[Column]) -> GenerationResult<()> {
        let mut names = std::collections::HashSet::new();
        if columns
            .iter()
            .any(|column| column.schema.name.is_empty() || !names.insert(&column.schema.name))
        {
            return Err(invalid("output column names must be nonempty and unique"));
        }
        Ok(())
    }
}

fn names_for_ids(columns: &[Column], ids: &[ColumnId]) -> GenerationResult<Vec<String>> {
    ids.iter()
        .map(|id| Ok(column_by_id(columns, *id)?.schema.name.clone()))
        .collect()
}

fn identity_select(columns: &[Column]) -> Vec<SelectItem> {
    columns
        .iter()
        .map(|column| SelectItem {
            alias: column.schema.name.clone(),
            expression: SelectExpression::Scalar {
                expr: Expr::Identifier(column.schema.name.clone()),
                partition_by: Vec::new(),
            },
        })
        .collect()
}

fn lower(relation: &Relation, sequence: &mut usize) -> GenerationResult<SqlQuery> {
    let (input, items, predicate, group_by, distinct) = match &relation.node {
        RelNode::Query(query) => return Ok(query.as_ref().clone()),
        RelNode::Scan(source) => {
            return Ok(SqlQuery {
                source: SqlSource::Table(source.clone()),
                projection: identity_select(&relation.columns),
                filter: None,
                group_by: Vec::new(),
                order_by: Vec::new(),
                distinct: false,
                limit: None,
            })
        }
        RelNode::Join {
            left,
            right,
            join_type,
            keys,
            predicates,
            closest,
            items,
            na_matches,
        } => {
            let projection = items
                .iter()
                .map(|item| {
                    let left_ref = item
                        .left
                        .map(|id| {
                            Ok((
                                "__libdplyr_left".to_string(),
                                column_by_id(&left.columns, id)?.schema.name.clone(),
                            ))
                        })
                        .transpose()?;
                    let right_ref = item
                        .right
                        .map(|id| {
                            Ok((
                                "__libdplyr_right".to_string(),
                                column_by_id(&right.columns, id)?.schema.name.clone(),
                            ))
                        })
                        .transpose()?;
                    let expression = match (left_ref, right_ref) {
                        (Some(left), Some(right)) => SelectExpression::Coalesce { left, right },
                        (Some((relation, column)), None) | (None, Some((relation, column))) => {
                            SelectExpression::Qualified { relation, column }
                        }
                        _ => return Err(invalid("join projection has no source column")),
                    };
                    Ok(SelectItem {
                        expression,
                        alias: item.column.schema.name.clone(),
                    })
                })
                .collect::<GenerationResult<Vec<_>>>()?;
            let keys = keys
                .iter()
                .map(|(a, b)| {
                    Ok((
                        column_by_id(&left.columns, *a)?.schema.name.clone(),
                        column_by_id(&right.columns, *b)?.schema.name.clone(),
                    ))
                })
                .collect::<GenerationResult<_>>()?;
            return Ok(SqlQuery {
                source: SqlSource::Join {
                    left: Box::new(lower(left, sequence)?),
                    right: Box::new(lower(right, sequence)?),
                    join_type: join_type.clone(),
                    keys,
                    predicates: predicates
                        .iter()
                        .map(|(a, op, b)| {
                            Ok((
                                column_by_id(&left.columns, *a)?.schema.name.clone(),
                                op.clone(),
                                column_by_id(&right.columns, *b)?.schema.name.clone(),
                            ))
                        })
                        .collect::<GenerationResult<_>>()?,
                    closest: closest
                        .as_ref()
                        .map(|(a, op, b)| {
                            Ok((
                                column_by_id(&left.columns, *a)?.schema.name.clone(),
                                op.clone(),
                                column_by_id(&right.columns, *b)?.schema.name.clone(),
                            ))
                        })
                        .transpose()?,
                    na_matches: *na_matches,
                },
                projection,
                filter: None,
                group_by: Vec::new(),
                order_by: Vec::new(),
                distinct: false,
                limit: None,
            });
        }
        RelNode::Slice { input, spec, by } => {
            return slice::lower(
                lower(input, sequence)?,
                spec,
                names_for_ids(&input.columns, by)?,
                input
                    .columns
                    .iter()
                    .map(|column| column.schema.name.clone())
                    .collect(),
            );
        }
        RelNode::Set {
            left,
            right,
            operation,
        } => {
            return Ok(SqlQuery {
                source: SqlSource::Set {
                    left: Box::new(lower(left, sequence)?),
                    right: Box::new(lower(right, sequence)?),
                    operation: operation.clone(),
                },
                projection: identity_select(&relation.columns),
                filter: None,
                group_by: Vec::new(),
                order_by: Vec::new(),
                distinct: false,
                limit: None,
            });
        }
        RelNode::Project {
            input,
            items,
            partition,
            frame,
        } => {
            let partition_by = names_for_ids(&input.columns, partition)?;
            let items = items
                .iter()
                .map(|item| {
                    Ok(SelectItem {
                        alias: item.column.schema.name.clone(),
                        expression: SelectExpression::WindowScalar {
                            expr: item.expression.to_expr(&input.columns)?,
                            partition_by: partition_by.clone(),
                            order_by: input
                                .order
                                .iter()
                                .map(|(id, direction)| {
                                    Ok(OrderExpr {
                                        column: column_by_id(&input.columns, *id)?
                                            .schema
                                            .name
                                            .clone(),
                                        direction: direction.clone(),
                                    })
                                })
                                .collect::<GenerationResult<_>>()?,
                            frame: *frame,
                        },
                    })
                })
                .collect::<GenerationResult<_>>()?;
            (input.as_ref(), items, None, Vec::new(), false)
        }
        RelNode::Filter { input, predicate } => (
            input.as_ref(),
            identity_select(&relation.columns),
            Some(predicate.to_expr(&input.columns)?),
            Vec::new(),
            false,
        ),
        RelNode::Aggregate {
            input,
            keys,
            measures,
        } => {
            let groups = names_for_ids(&input.columns, keys)?;
            let mut items = identity_select(&relation.columns[..keys.len()]);
            for measure in measures {
                items.push(SelectItem {
                    alias: measure.column.schema.name.clone(),
                    expression: SelectExpression::AggregateExpression(
                        measure.expression.to_expr(&input.columns)?,
                    ),
                });
            }
            (input.as_ref(), items, None, groups, false)
        }
        RelNode::Distinct(input) => (
            input.as_ref(),
            identity_select(&relation.columns),
            None,
            Vec::new(),
            true,
        ),
    };
    let query = lower(input, sequence)?;
    let alias = format!("q{}", *sequence);
    *sequence += 1;
    Ok(SqlQuery {
        source: SqlSource::Subquery(Box::new(query), alias),
        projection: items,
        filter: predicate,
        group_by,
        order_by: Vec::new(),
        distinct,
        limit: None,
    })
}

pub(crate) fn compile(
    ast: &DplyrNode,
    schema: &SourceSchema,
    generator: &SqlGenerator,
) -> GenerationResult<CompiledQuery> {
    compile_with_schemas(ast, std::slice::from_ref(schema), generator)
}

pub(crate) fn required_sources(ast: &DplyrNode) -> GenerationResult<Vec<String>> {
    let (source, operations) = match ast {
        DplyrNode::Pipeline {
            source,
            operations,
            target,
            ..
        } => {
            if target.is_some() {
                return Err(invalid(
                    "schema-aware compilation does not create target tables",
                ));
            }
            (
                source.as_deref().ok_or_else(|| {
                    invalid("automatic schema discovery requires an explicit source")
                })?,
                operations.as_slice(),
            )
        }
        DplyrNode::DataSource { name, .. } => (name.as_str(), &[][..]),
    };
    let mut sources = vec![source.to_string()];
    fn add(sources: &mut Vec<String>, name: &str) {
        if !sources.iter().any(|s| s == name) {
            sources.push(name.to_owned());
        }
    }
    fn operand(expr: &Expr, sources: &mut Vec<String>) {
        match expr {
            Expr::Identifier(name) | Expr::Literal(LiteralValue::String(name)) => {
                add(sources, name)
            }
            Expr::Function { name, args } if name == "__pipeline" => {
                if let Some(source) = args.first() {
                    operand(source, sources);
                }
                for step in args.iter().skip(1) {
                    if let Expr::Function { name, args } = step {
                        extended(name, args, sources);
                    }
                }
            }
            _ => {}
        }
    }
    fn extended(name: &str, args: &[Expr], sources: &mut Vec<String>) {
        if matches!(
            name,
            "union"
                | "union_all"
                | "intersect"
                | "setdiff"
                | "cross_join"
                | "rows_insert"
                | "rows_append"
                | "rows_update"
                | "rows_patch"
                | "rows_upsert"
                | "rows_delete"
        ) {
            if let Some(right) = args.first() {
                operand(right, sources);
            }
        } else if name == "bind_queries" {
            for arg in args {
                operand(arg, sources);
            }
        }
    }
    fn visit(operations: &[DplyrOperation], sources: &mut Vec<String>) {
        for operation in operations {
            match operation {
                DplyrOperation::Join { spec, .. } => {
                    add(sources, &spec.table);
                    visit(&spec.right_operations, sources);
                }
                DplyrOperation::SetOp { right_table, .. } => add(sources, right_table),
                DplyrOperation::Extended { name, args, .. } => extended(name, args, sources),
                _ => {}
            }
        }
    }
    visit(operations, &mut sources);
    Ok(sources)
}

pub(crate) fn compile_with_schemas(
    ast: &DplyrNode,
    schemas: &[SourceSchema],
    generator: &SqlGenerator,
) -> GenerationResult<CompiledQuery> {
    Ok(compile_plan(
        ast,
        schemas,
        generator,
        false,
        &std::collections::HashMap::new(),
    )?
    .query)
}

pub(crate) fn compile_for_execution(
    ast: &DplyrNode,
    schemas: &[SourceSchema],
    generator: &SqlGenerator,
) -> GenerationResult<crate::execution::ExecutionPlan> {
    compile_plan(
        ast,
        schemas,
        generator,
        true,
        &std::collections::HashMap::new(),
    )
}

pub(crate) fn compile_with_bindings(
    ast: &DplyrNode,
    schemas: &[SourceSchema],
    generator: &SqlGenerator,
    bindings: &std::collections::HashMap<String, serde_json::Value>,
) -> GenerationResult<CompiledQuery> {
    Ok(compile_plan(ast, schemas, generator, false, bindings)?.query)
}

fn compile_plan(
    ast: &DplyrNode,
    schemas: &[SourceSchema],
    generator: &SqlGenerator,
    collect_checks: bool,
    bindings: &std::collections::HashMap<String, serde_json::Value>,
) -> GenerationResult<crate::execution::ExecutionPlan> {
    schema::validate_schemas(schemas)?;
    let (source, operations) = match ast {
        DplyrNode::Pipeline {
            source,
            target,
            operations,
            ..
        } => {
            if target.is_some() {
                return Err(invalid(
                    "schema-aware compilation does not execute or create target tables",
                ));
            }
            (
                source.as_deref().unwrap_or(&schemas[0].source),
                operations.as_slice(),
            )
        }
        DplyrNode::DataSource { name, .. } => (name.as_str(), &[][..]),
    };
    let mut planner = Planner {
        generator,
        schemas,
        next_id: 0,
        stages: 1,
        checks: collect_checks.then(Vec::new),
        bindings,
    };
    let mut relation = planner.scan(source)?;
    for operation in operations {
        relation = planner.apply(relation, operation)?;
    }
    let mut sequence = 1;
    let mut query = lower(&relation, &mut sequence)?;
    if relation.columns.iter().any(|column| column.hidden) {
        let visible = relation
            .columns
            .iter()
            .filter(|column| !column.hidden)
            .cloned()
            .collect::<Vec<_>>();
        query = SqlQuery {
            source: SqlSource::Subquery(Box::new(query), format!("q{sequence}")),
            projection: identity_select(&visible),
            filter: None,
            group_by: Vec::new(),
            order_by: Vec::new(),
            distinct: false,
            limit: None,
        };
        planner.stage()?;
    }
    query.order_by = relation
        .order
        .iter()
        .map(|(id, direction)| {
            Ok(OrderExpr {
                column: column_by_id(&relation.columns, *id)?.schema.name.clone(),
                direction: direction.clone(),
            })
        })
        .collect::<GenerationResult<_>>()?;
    let sql = query.render(generator)?;
    let columns = relation
        .columns
        .into_iter()
        .filter(|column| !column.hidden)
        .map(|column| column.schema)
        .collect();
    Ok(crate::execution::ExecutionPlan {
        query: CompiledQuery {
            sql,
            columns,
            stages: planner.stages,
        },
        checks: planner.checks.unwrap_or_default(),
    })
}

pub(crate) use discovery::{pivot_keys, supply_pivot_keys};
