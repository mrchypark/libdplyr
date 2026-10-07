//! Schema-aware compilation with explicit relational stages.
//!
//! The legacy API remains available for compilation without source metadata.
//! This path binds every reference before rendering SQL and never substitutes
//! a computed expression for a reference to an earlier stage.

pub mod schema;
mod sql;

pub use schema::{SchemaColumn, SchemaInput, SourceSchema};

use crate::error::{GenerationError, GenerationResult};
use crate::parser::{
    BinaryOp, DplyrNode, DplyrOperation, Expr, JoinKey, JoinSpec, JoinType, LiteralValue,
    OrderDirection, OrderExpr, SetOperation,
};
use crate::sql_generator::SqlGenerator;
use sql::{SelectExpression, SelectItem, SqlQuery, SqlSource};

/// SQL and the ordered, visible output schema of a compiled pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    Binary(Box<BoundExpr>, BinaryOp, Box<BoundExpr>),
    Function(String, Vec<BoundExpr>),
    CaseWhen(Vec<(BoundExpr, BoundExpr)>, Option<Box<BoundExpr>>),
    NamedArg(String, Box<BoundExpr>),
}

struct Projection {
    column: Column,
    expression: BoundExpr,
}

struct Measure {
    column: Column,
    expression: BoundExpr,
}

struct JoinProjection {
    column: Column,
    left: Option<ColumnId>,
    right: Option<ColumnId>,
}

struct Relation {
    node: RelNode,
    columns: Vec<Column>,
    groups: Vec<ColumnId>,
    order: Vec<(ColumnId, OrderDirection)>,
}

enum RelNode {
    Scan(String),
    Project {
        input: Box<Relation>,
        items: Vec<Projection>,
        partition: Vec<ColumnId>,
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
    Join {
        left: Box<Relation>,
        right: Box<Relation>,
        join_type: JoinType,
        keys: Vec<(ColumnId, ColumnId)>,
        items: Vec<JoinProjection>,
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

fn equality_keys(expr: &Expr, keys: &mut Vec<JoinKey>) -> GenerationResult<()> {
    match expr {
        Expr::Identifier(name) => keys.push(JoinKey {
            left: name.clone(),
            right: name.clone(),
        }),
        Expr::Binary {
            left,
            operator: BinaryOp::And,
            right,
        } => {
            equality_keys(left, keys)?;
            equality_keys(right, keys)?;
        }
        Expr::Binary {
            left,
            operator: BinaryOp::Equal,
            right,
        } => {
            let (Expr::Identifier(left), Expr::Identifier(right)) = (left.as_ref(), right.as_ref())
            else {
                return Err(invalid("join comparisons require column identifiers"));
            };
            keys.push(JoinKey {
                left: left.clone(),
                right: right.clone(),
            });
        }
        _ => return Err(invalid("join predicates currently require equality keys")),
    }
    Ok(())
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
        self.stage()?;
        Ok(Relation {
            node: RelNode::Project {
                input: Box::new(input),
                items,
                partition,
            },
            columns,
            groups,
            order,
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
            )
    }

    fn has_window(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Function { name, args } => {
                self.is_window(name) || args.iter().any(|arg| self.has_window(arg))
            }
            Expr::Binary { left, right, .. } => self.has_window(left) || self.has_window(right),
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
        input: Relation,
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

    fn join(
        &mut self,
        left: Relation,
        join_type: &JoinType,
        spec: &JoinSpec,
    ) -> GenerationResult<Relation> {
        let mut by = spec.by.clone();
        if let Some(expr) = &spec.on_expr {
            equality_keys(expr, &mut by)?;
        }
        if by.is_empty() {
            return Err(invalid("join requires at least one equality key"));
        }
        self.stage()?;
        let right = self.scan(&spec.table)?;
        let mut left_keys = std::collections::HashSet::new();
        let mut right_keys = std::collections::HashSet::new();
        let keys = by
            .iter()
            .map(|key| {
                let left_id = visible_column(&left.columns, &key.left)?.id;
                let right_id = visible_column(&right.columns, &key.right)?.id;
                if !left_keys.insert(left_id.0) || !right_keys.insert(right_id.0) {
                    return Err(invalid("join keys must be unique on each side"));
                }
                Ok((left_id, right_id))
            })
            .collect::<GenerationResult<Vec<_>>>()?;
        let filtering = matches!(join_type, JoinType::Semi | JoinType::Anti);
        let right_visible = right
            .columns
            .iter()
            .filter(|column| !column.hidden && !right_keys.contains(&column.id.0))
            .collect::<Vec<_>>();
        // Equality keys retain left names. Suffix all other names together,
        // so a generated suffix and an existing suffix are repaired in input order.
        let left_names = left
            .columns
            .iter()
            .map(|column| column.schema.name.clone())
            .collect::<Vec<_>>();
        let left_aux = left
            .columns
            .iter()
            .filter(|column| !column.hidden && !left_keys.contains(&column.id.0))
            .collect::<Vec<_>>();
        let key_names = by.iter().map(|key| key.left.clone()).collect::<Vec<_>>();
        let mut reserved = key_names.clone();
        reserved.extend(
            left.columns
                .iter()
                .filter(|column| column.hidden)
                .map(|column| column.schema.name.clone()),
        );
        reserved.extend(
            right_visible
                .iter()
                .filter(|column| !key_names.contains(&column.schema.name))
                .map(|column| column.schema.name.clone()),
        );
        let left_output = add_suffixes(
            left_aux
                .iter()
                .map(|column| column.schema.name.clone())
                .collect(),
            &reserved,
            ".x",
        );
        let right_output = add_suffixes(
            right_visible
                .iter()
                .map(|column| column.schema.name.clone())
                .collect(),
            &left_names,
            ".y",
        );
        let mut items = Vec::new();
        for source in &left.columns {
            let mut column = source.clone();
            if !filtering {
                if let Some(index) = left_aux.iter().position(|item| item.id == source.id) {
                    column.schema.name.clone_from(&left_output[index]);
                }
            }
            let coalesced = if matches!(join_type, JoinType::Right | JoinType::Full) {
                keys.iter()
                    .find(|(id, _)| *id == source.id)
                    .map(|(_, id)| *id)
            } else {
                None
            };
            if let Some(id) = coalesced {
                let metadata = &column_by_id(&right.columns, id)?.schema;
                if column.schema.data_type != metadata.data_type {
                    column.schema.data_type = None;
                }
                column.schema.nullable = match (column.schema.nullable, metadata.nullable) {
                    (Some(false), Some(false)) => Some(false),
                    (Some(true), _) | (_, Some(true)) => Some(true),
                    _ => None,
                };
            } else if matches!(join_type, JoinType::Right | JoinType::Full) {
                column.schema.nullable = Some(true);
            }
            items.push(JoinProjection {
                column,
                left: Some(source.id),
                right: coalesced,
            });
        }
        if !filtering {
            for (index, source) in right_visible.iter().enumerate() {
                let mut column = (*source).clone();
                column.schema.name.clone_from(&right_output[index]);
                if matches!(join_type, JoinType::Left | JoinType::Full) {
                    column.schema.nullable = Some(true);
                }
                items.push(JoinProjection {
                    column,
                    left: None,
                    right: Some(source.id),
                });
            }
        }
        let columns = items
            .iter()
            .map(|item| item.column.clone())
            .collect::<Vec<_>>();
        self.validate_output(&columns)?;
        self.stage()?;
        Ok(Relation {
            columns,
            groups: left.groups.clone(),
            order: left.order.clone(),
            node: RelNode::Join {
                left: Box::new(left),
                right: Box::new(right),
                join_type: join_type.clone(),
                keys,
                items,
            },
        })
    }

    fn set(
        &mut self,
        mut left: Relation,
        operation: &SetOperation,
        right_table: &str,
    ) -> GenerationResult<Relation> {
        // Set equality uses visible values only; a prior sort cannot change membership.
        left.order.clear();
        if left.columns.iter().any(|column| column.hidden) {
            let visible = left
                .columns
                .iter()
                .filter(|column| !column.hidden)
                .cloned()
                .collect::<Vec<_>>();
            left = self.project(left, Self::identities(&visible))?;
        }
        self.stage()?;
        let right = self.scan(right_table)?;
        if left.columns.len() != right.columns.len() {
            return Err(invalid("set inputs must have the same column names"));
        }
        let mut columns = left.columns.clone();
        let mut aligned = Vec::new();
        for column in &mut columns {
            let counterpart = visible_column(&right.columns, &column.schema.name)?;
            aligned.push(Projection {
                column: counterpart.clone(),
                expression: BoundExpr::Column(counterpart.id),
            });
            if column.schema.data_type != counterpart.schema.data_type {
                column.schema.data_type = None;
            }
            column.schema.nullable = match operation {
                SetOperation::SetDiff => column.schema.nullable,
                SetOperation::Union => {
                    match (column.schema.nullable, counterpart.schema.nullable) {
                        (Some(false), Some(false)) => Some(false),
                        (Some(true), _) | (_, Some(true)) => Some(true),
                        _ => None,
                    }
                }
                SetOperation::Intersect => {
                    match (column.schema.nullable, counterpart.schema.nullable) {
                        (Some(false), _) | (_, Some(false)) => Some(false),
                        _ => None,
                    }
                }
            };
        }
        let right = self.project(right, aligned)?;
        self.stage()?;
        Ok(Relation {
            columns,
            groups: left.groups.clone(),
            order: Vec::new(),
            node: RelNode::Set {
                left: Box::new(left),
                right: Box::new(right),
                operation: operation.clone(),
            },
        })
    }

    fn apply(
        &mut self,
        mut input: Relation,
        operation: &DplyrOperation,
    ) -> GenerationResult<Relation> {
        match operation {
            DplyrOperation::Select { columns, .. } => {
                let mut items = Vec::new();
                for selected in columns {
                    let Expr::Identifier(name) = &selected.expr else {
                        return Err(self.unsupported("computed select(); use mutate()"));
                    };
                    if name == "*" {
                        if selected.alias.is_some() {
                            return Err(invalid("a wildcard cannot have an alias"));
                        }
                        items.extend(Self::identities(
                            &input
                                .columns
                                .iter()
                                .filter(|column| !column.hidden)
                                .cloned()
                                .collect::<Vec<_>>(),
                        ));
                    } else {
                        let mut column = visible_column(&input.columns, name)?.clone();
                        if let Some(alias) = &selected.alias {
                            column.schema.name.clone_from(alias);
                        }
                        items.push(Projection {
                            expression: BoundExpr::Column(column.id),
                            column,
                        });
                    }
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
                    self.validate_expression(&assignment.expr)?;
                    if input.columns.iter().any(|column| {
                        !column.hidden
                            && column.schema.name == assignment.column
                            && input.groups.contains(&column.id)
                    }) {
                        return Err(self.unsupported("mutate() of a grouping column"));
                    }
                    let expression = BoundExpr::bind(&assignment.expr, &input.columns)?;
                    let mut column = self.new_column(&assignment.column);
                    if let Expr::Identifier(name) = &assignment.expr {
                        let source = visible_column(&input.columns, name)?;
                        column.schema.data_type.clone_from(&source.schema.data_type);
                        column.schema.nullable = source.schema.nullable;
                    }
                    let mut items = Self::identities(&input.columns);
                    let projection = Projection { column, expression };
                    if let Some(index) = items.iter().position(|item| {
                        !item.column.hidden && item.column.schema.name == assignment.column
                    }) {
                        items[index] = projection;
                    } else {
                        items.push(projection);
                    }
                    input = self.project(input, items)?;
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
                    node: RelNode::Filter {
                        input: Box::new(input),
                        predicate,
                    },
                };
                if self.has_window(condition) {
                    result = self.project(result, Self::identities(&visible))?;
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
            DplyrOperation::SummariseExpressions { assignments, .. } => self.summarise(
                input,
                assignments
                    .iter()
                    .map(|assignment| (assignment.column.clone(), assignment.expr.clone()))
                    .collect(),
            ),
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
        RelNode::Scan(source) => {
            return Ok(SqlQuery {
                source: SqlSource::Table(source.clone()),
                projection: identity_select(&relation.columns),
                filter: None,
                group_by: Vec::new(),
                order_by: Vec::new(),
                distinct: false,
            })
        }
        RelNode::Join {
            left,
            right,
            join_type,
            keys,
            items,
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
                },
                projection,
                filter: None,
                group_by: Vec::new(),
                order_by: Vec::new(),
                distinct: false,
            });
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
            });
        }
        RelNode::Project {
            input,
            items,
            partition,
        } => {
            let partition_by = names_for_ids(&input.columns, partition)?;
            let items = items
                .iter()
                .map(|item| {
                    Ok(SelectItem {
                        alias: item.column.schema.name.clone(),
                        expression: SelectExpression::Scalar {
                            expr: item.expression.to_expr(&input.columns)?,
                            partition_by: partition_by.clone(),
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
    for operation in operations {
        let source = match operation {
            DplyrOperation::Join { spec, .. } => &spec.table,
            DplyrOperation::SetOp { right_table, .. } => right_table,
            _ => continue,
        };
        if !sources.contains(source) {
            sources.push(source.clone());
        }
    }
    Ok(sources)
}

pub(crate) fn compile_with_schemas(
    ast: &DplyrNode,
    schemas: &[SourceSchema],
    generator: &SqlGenerator,
) -> GenerationResult<CompiledQuery> {
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
    Ok(CompiledQuery {
        sql,
        columns,
        stages: planner.stages,
    })
}
