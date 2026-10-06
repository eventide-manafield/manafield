use std::collections::{BTreeMap, HashSet};
use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DataSchema {
    String,
    Integer,
    Number,
    Boolean,
    Array {
        items: Box<DataSchema>,
    },
    Object {
        #[serde(default)]
        properties: BTreeMap<String, DataSchema>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        required: Vec<String>,
    },
}

impl DataSchema {
    pub(crate) fn validate(&self) -> Result<(), SchemaError> {
        self.validate_at("$")
    }

    fn validate_at(&self, path: &str) -> Result<(), SchemaError> {
        match self {
            Self::String | Self::Integer | Self::Number | Self::Boolean => Ok(()),
            Self::Array { items } => items.validate_at(&format!("{path}[]")),
            Self::Object {
                properties,
                required,
            } => {
                for (name, schema) in properties {
                    if name.trim().is_empty() {
                        return Err(SchemaError::EmptyPropertyName {
                            path: path.to_owned(),
                        });
                    }

                    schema.validate_at(&format!("{path}.{name}"))?;
                }

                let mut seen_required = HashSet::new();

                for name in required {
                    if !seen_required.insert(name.as_str()) {
                        return Err(SchemaError::DuplicateRequiredProperty {
                            path: path.to_owned(),
                            property: name.clone(),
                        });
                    }

                    if !properties.contains_key(name) {
                        return Err(SchemaError::UnknownRequiredProperty {
                            path: path.to_owned(),
                            property: name.clone(),
                        });
                    }
                }

                Ok(())
            }
        }
    }
}

#[derive(Debug)]
pub enum SchemaError {
    EmptyPropertyName { path: String },
    DuplicateRequiredProperty { path: String, property: String },
    UnknownRequiredProperty { path: String, property: String },
}

impl fmt::Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPropertyName { path } => {
                write!(
                    f,
                    "object schema at '{path}' contains an empty property name"
                )
            }
            Self::DuplicateRequiredProperty { path, property } => write!(
                f,
                "object schema at '{path}' declares required property '{property}' more than once"
            ),
            Self::UnknownRequiredProperty { path, property } => write!(
                f,
                "object schema at '{path}' requires unknown property '{property}'"
            ),
        }
    }
}

impl std::error::Error for SchemaError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_nested_schema() {
        let schema: DataSchema = serde_json::from_str(
            r#"{
                "type": "object",
                "required": ["name", "tags"],
                "properties": {
                    "name": { "type": "string" },
                    "tags": {
                        "type": "array",
                        "items": { "type": "string" }
                    }
                }
            }"#,
        )
        .expect("schema should deserialize");

        assert!(schema.validate().is_ok());
    }

    #[test]
    fn rejects_unknown_required_property() {
        let schema = DataSchema::Object {
            properties: BTreeMap::new(),
            required: vec!["missing".to_owned()],
        };

        assert!(matches!(
            schema.validate(),
            Err(SchemaError::UnknownRequiredProperty { .. })
        ));
    }
}
