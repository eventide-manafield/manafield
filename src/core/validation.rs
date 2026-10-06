use std::collections::HashSet;
use std::fmt;

use super::{ModuleDescriptor, OperationBinding};

pub fn validate_module(module: &ModuleDescriptor) -> Result<(), ValidationError> {
    if module.id.trim().is_empty() {
        return Err(ValidationError::EmptyModuleId);
    }

    if module.name.trim().is_empty() {
        return Err(ValidationError::EmptyModuleName);
    }

    if module.version.trim().is_empty() {
        return Err(ValidationError::EmptyModuleVersion);
    }

    let mut operation_ids = HashSet::new();

    for operation in &module.operations {
        if operation.id.trim().is_empty() {
            return Err(ValidationError::EmptyOperationId);
        }

        if !operation_ids.insert(operation.id.as_str()) {
            return Err(ValidationError::DuplicateOperationId(operation.id.clone()));
        }

        match &operation.binding {
            OperationBinding::Http { path, codecs, .. } => {
                if !path.starts_with('/') {
                    return Err(ValidationError::InvalidHttpPath {
                        operation_id: operation.id.clone(),
                        path: path.clone(),
                    });
                }

                if codecs.is_empty() {
                    return Err(ValidationError::NoPayloadCodecs(operation.id.clone()));
                }
            }
        }
    }

    Ok(())
}

#[derive(Debug)]
pub enum ValidationError {
    EmptyModuleId,
    EmptyModuleName,
    EmptyModuleVersion,
    EmptyOperationId,
    DuplicateOperationId(String),
    InvalidHttpPath { operation_id: String, path: String },
    NoPayloadCodecs(String),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyModuleId => write!(f, "module id must not be empty"),
            Self::EmptyModuleName => write!(f, "module name must not be empty"),
            Self::EmptyModuleVersion => write!(f, "module version must not be empty"),
            Self::EmptyOperationId => write!(f, "operation id must not be empty"),
            Self::DuplicateOperationId(id) => {
                write!(f, "operation id '{id}' is duplicated")
            }
            Self::InvalidHttpPath { operation_id, path } => write!(
                f,
                "HTTP path '{path}' for operation '{operation_id}' must start with '/'"
            ),
            Self::NoPayloadCodecs(operation_id) => write!(
                f,
                "HTTP operation '{operation_id}' must declare at least one payload codec"
            ),
        }
    }
}

impl std::error::Error for ValidationError {}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::core::operation::{HttpMethod, OperationBinding, OperationContract, PayloadCodec};

    fn valid_module() -> ModuleDescriptor {
        ModuleDescriptor {
            id: "sample".to_owned(),
            name: "Sample Module".to_owned(),
            version: "0.0.1".to_owned(),
            operations: vec![OperationContract {
                id: "echo".to_owned(),
                input: Some(json!({ "type": "object" })),
                output: Some(json!({ "type": "object" })),
                binding: OperationBinding::Http {
                    method: HttpMethod::Post,
                    path: "/echo".to_owned(),
                    codecs: vec![PayloadCodec::Json],
                },
            }],
        }
    }

    #[test]
    fn accepts_valid_module() {
        assert!(validate_module(&valid_module()).is_ok());
    }

    #[test]
    fn rejects_duplicate_operation_ids() {
        let mut module = valid_module();
        module.operations.push(module.operations[0].clone());

        assert!(matches!(
            validate_module(&module),
            Err(ValidationError::DuplicateOperationId(_))
        ));
    }

    #[test]
    fn rejects_http_path_without_leading_slash() {
        let mut module = valid_module();
        let OperationBinding::Http { path, .. } = &mut module.operations[0].binding;
        *path = "echo".to_owned();

        assert!(matches!(
            validate_module(&module),
            Err(ValidationError::InvalidHttpPath { .. })
        ));
    }
}
