//! Schema metadata for schema-aware relational compilation.

use serde::{Deserialize, Serialize};

use crate::error::{GenerationError, GenerationResult};

/// A single column of a source relation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaColumn {
    pub name: String,
    #[serde(default)]
    pub data_type: Option<String>,
    #[serde(default)]
    pub nullable: Option<bool>,
}

impl SchemaColumn {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            data_type: None,
            nullable: None,
        }
    }
}

/// An ordered list of columns for one source relation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSchema {
    pub source: String,
    pub columns: Vec<SchemaColumn>,
}

/// JSON metadata accepts one source object or an ordered array of source objects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SchemaInput {
    Single(SourceSchema),
    Multiple(Vec<SourceSchema>),
}

impl SchemaInput {
    pub fn as_slice(&self) -> &[SourceSchema] {
        match self {
            Self::Single(schema) => std::slice::from_ref(schema),
            Self::Multiple(schemas) => schemas,
        }
    }

    pub fn validate(&self) -> GenerationResult<()> {
        validate_schemas(self.as_slice())
    }
}

pub(super) fn validate_schemas(schemas: &[SourceSchema]) -> GenerationResult<()> {
    if schemas.is_empty() {
        return Err(GenerationError::InvalidAst {
            reason: "at least one source schema is required".to_string(),
        });
    }
    let mut seen = std::collections::HashSet::new();
    for schema in schemas {
        schema.validate()?;
        if !seen.insert(&schema.source) {
            return Err(GenerationError::InvalidAst {
                reason: format!("duplicate schema source '{}'", schema.source),
            });
        }
    }
    Ok(())
}

impl SourceSchema {
    /// Build a schema from plain column names, preserving their order.
    pub fn new(
        source: impl Into<String>,
        columns: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self::with_columns(source, columns.into_iter().map(SchemaColumn::new).collect())
    }

    pub fn with_columns(source: impl Into<String>, columns: Vec<SchemaColumn>) -> Self {
        Self {
            source: source.into(),
            columns,
        }
    }

    /// Rejects an empty source, no columns, empty column names, and duplicate names.
    pub fn validate(&self) -> GenerationResult<()> {
        if self.source.is_empty() || self.source.contains('\0') {
            return Err(GenerationError::UnsupportedOperation {
                operation: "schema source must be nonempty and contain no NUL".to_string(),
                dialect: "schema".to_string(),
            });
        }

        if self.columns.is_empty() {
            return Err(GenerationError::UnsupportedOperation {
                operation: format!("schema for '{}' has no columns", self.source),
                dialect: "schema".to_string(),
            });
        }

        let mut seen = std::collections::HashSet::with_capacity(self.columns.len());
        for column in &self.columns {
            if column.name.is_empty() || column.name.contains('\0') {
                return Err(GenerationError::InvalidColumnReference {
                    column: column.name.clone(),
                    table: Some(self.source.clone()),
                });
            }
            if !seen.insert(column.name.as_str()) {
                return Err(GenerationError::InvalidColumnReference {
                    column: column.name.clone(),
                    table: Some(self.source.clone()),
                });
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid() -> SourceSchema {
        SourceSchema::new("data", vec!["name", "age"])
    }

    #[test]
    fn valid_schema_passes() {
        assert!(valid().validate().is_ok());
    }

    #[test]
    fn column_order_is_preserved() {
        let schema = valid();
        let names: Vec<&str> = schema.columns.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["name", "age"]);
    }

    #[test]
    fn new_leaves_optional_fields_unset() {
        let column = SchemaColumn::new("age");
        assert_eq!(column.data_type, None);
        assert_eq!(column.nullable, None);
    }

    #[test]
    fn empty_source_is_rejected() {
        let schema = SourceSchema::new("", vec!["name"]);
        assert!(matches!(
            schema.validate(),
            Err(GenerationError::UnsupportedOperation { dialect, .. }) if dialect == "schema"
        ));
    }

    #[test]
    fn empty_column_list_is_rejected() {
        let schema = SourceSchema::new("data", Vec::<String>::new());
        assert!(matches!(
            schema.validate(),
            Err(GenerationError::UnsupportedOperation { dialect, .. }) if dialect == "schema"
        ));
    }

    #[test]
    fn empty_column_name_is_rejected() {
        let schema = SourceSchema::with_columns(
            "data",
            vec![SchemaColumn::new("name"), SchemaColumn::new("")],
        );
        assert!(matches!(
            schema.validate(),
            Err(GenerationError::InvalidColumnReference { table: Some(t), .. }) if t == "data"
        ));
    }

    #[test]
    fn duplicate_column_names_are_rejected() {
        let schema = SourceSchema::new("data", vec!["name", "name"]);
        assert!(matches!(
            schema.validate(),
            Err(GenerationError::InvalidColumnReference { column, .. }) if column == "name"
        ));
    }

    #[test]
    fn deserializes_from_json_with_optional_fields() {
        let json = r#"{
            "source": "data",
            "columns": [
                {"name": "id", "data_type": "integer", "nullable": false},
                {"name": "name"}
            ]
        }"#;

        let schema: SourceSchema = serde_json::from_str(json).expect("valid schema json");
        assert_eq!(schema.source, "data");
        assert_eq!(schema.columns.len(), 2);
        assert_eq!(schema.columns[0].data_type.as_deref(), Some("integer"));
        assert_eq!(schema.columns[0].nullable, Some(false));
        assert_eq!(schema.columns[1].data_type, None);
        assert!(schema.validate().is_ok());
    }
}
