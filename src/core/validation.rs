use std::collections::HashSet;
use std::fmt;

use super::schema::SchemaError;
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

    let mut provided_capability_ids = HashSet::new();

    for capability in &module.provides.capabilities {
        if capability.id.trim().is_empty() {
            return Err(ValidationError::EmptyProvidedCapabilityId);
        }

        if capability.version.trim().is_empty() {
            return Err(ValidationError::EmptyProvidedCapabilityVersion(
                capability.id.clone(),
            ));
        }

        if !provided_capability_ids.insert(capability.id.as_str()) {
            return Err(ValidationError::DuplicateProvidedCapability(
                capability.id.clone(),
            ));
        }
    }

    for (slot, requirement) in &module.requires.capabilities {
        if slot.trim().is_empty() {
            return Err(ValidationError::EmptyRequirementSlot);
        }

        if requirement.id.trim().is_empty() {
            return Err(ValidationError::EmptyRequiredCapabilityId(slot.clone()));
        }

        if requirement.version.trim().is_empty() {
            return Err(ValidationError::EmptyRequiredCapabilityVersion(
                slot.clone(),
                requirement.id.clone(),
            ));
        }
    }

    let mut operation_ids = HashSet::new();

    for operation in &module.operations {
        if operation.id.trim().is_empty() {
            return Err(ValidationError::EmptyOperationId);
        }

        if !operation_ids.insert(operation.id.as_str()) {
            return Err(ValidationError::DuplicateOperationId(operation.id.clone()));
        }

        if let Some(input) = &operation.input {
            input
                .validate()
                .map_err(|source| ValidationError::InvalidInputSchema {
                    operation_id: operation.id.clone(),
                    source,
                })?;
        }

        if let Some(output) = &operation.output {
            output
                .validate()
                .map_err(|source| ValidationError::InvalidOutputSchema {
                    operation_id: operation.id.clone(),
                    source,
                })?;
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

    if let Some(health_operation_id) = &module.health_operation {
        if health_operation_id.trim().is_empty() {
            return Err(ValidationError::EmptyHealthOperationId);
        }

        let health_operation = module
            .operations
            .iter()
            .find(|operation| operation.id == *health_operation_id)
            .ok_or_else(|| ValidationError::UnknownHealthOperation(health_operation_id.clone()))?;

        if health_operation.input.is_some() {
            return Err(ValidationError::HealthOperationHasInput(
                health_operation_id.clone(),
            ));
        }
    }

    Ok(())
}

#[derive(Debug)]
pub enum ValidationError {
    EmptyModuleId,
    EmptyModuleName,
    EmptyModuleVersion,
    EmptyProvidedCapabilityId,
    EmptyProvidedCapabilityVersion(String),
    DuplicateProvidedCapability(String),
    EmptyRequirementSlot,
    EmptyRequiredCapabilityId(String),
    EmptyRequiredCapabilityVersion(String, String),
    EmptyOperationId,
    EmptyHealthOperationId,
    DuplicateOperationId(String),
    UnknownHealthOperation(String),
    HealthOperationHasInput(String),
    InvalidInputSchema {
        operation_id: String,
        source: SchemaError,
    },
    InvalidOutputSchema {
        operation_id: String,
        source: SchemaError,
    },
    InvalidHttpPath {
        operation_id: String,
        path: String,
    },
    NoPayloadCodecs(String),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyModuleId => write!(f, "module id must not be empty"),
            Self::EmptyModuleName => write!(f, "module name must not be empty"),
            Self::EmptyModuleVersion => write!(f, "module version must not be empty"),
            Self::EmptyProvidedCapabilityId => {
                write!(f, "provided capability id must not be empty")
            }
            Self::EmptyProvidedCapabilityVersion(id) => {
                write!(f, "provided capability '{id}' version must not be empty")
            }
            Self::DuplicateProvidedCapability(id) => {
                write!(f, "provided capability '{id}' is declared more than once")
            }
            Self::EmptyRequirementSlot => {
                write!(f, "capability requirement slot must not be empty")
            }
            Self::EmptyRequiredCapabilityId(slot) => {
                write!(
                    f,
                    "required capability id for slot '{slot}' must not be empty"
                )
            }
            Self::EmptyRequiredCapabilityVersion(slot, id) => write!(
                f,
                "required capability '{id}' version for slot '{slot}' must not be empty"
            ),
            Self::EmptyOperationId => write!(f, "operation id must not be empty"),
            Self::EmptyHealthOperationId => write!(f, "health operation id must not be empty"),
            Self::DuplicateOperationId(id) => {
                write!(f, "operation id '{id}' is duplicated")
            }
            Self::UnknownHealthOperation(id) => {
                write!(f, "health operation '{id}' does not exist")
            }
            Self::HealthOperationHasInput(id) => {
                write!(f, "health operation '{id}' must not require input")
            }
            Self::InvalidInputSchema {
                operation_id,
                source,
            } => write!(
                f,
                "invalid input schema for operation '{operation_id}': {source}"
            ),
            Self::InvalidOutputSchema {
                operation_id,
                source,
            } => write!(
                f,
                "invalid output schema for operation '{operation_id}': {source}"
            ),
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

impl std::error::Error for ValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidInputSchema { source, .. } | Self::InvalidOutputSchema { source, .. } => {
                Some(source)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::core::capability::{
        CapabilityDescriptor, CapabilityRequirement, CapabilityRequirementSet, CapabilitySet,
    };
    use crate::core::operation::{HttpMethod, OperationBinding, OperationContract, PayloadCodec};
    use crate::core::schema::DataSchema;

    fn valid_module() -> ModuleDescriptor {
        ModuleDescriptor {
            id: "sample".to_owned(),
            name: "Sample Module".to_owned(),
            description: None,
            version: "0.0.1".to_owned(),
            health_operation: None,
            provides: CapabilitySet::default(),
            requires: CapabilityRequirementSet::default(),
            operations: vec![OperationContract {
                id: "echo".to_owned(),
                description: None,
                input: Some(DataSchema::Object {
                    properties: BTreeMap::new(),
                    required: Vec::new(),
                }),
                output: Some(DataSchema::Object {
                    properties: BTreeMap::new(),
                    required: Vec::new(),
                }),
                binding: OperationBinding::Http {
                    method: HttpMethod::Post,
                    path: "/echo".to_owned(),
                    codecs: vec![PayloadCodec::Json],
                },
            }],
        }
    }

    fn health_operation() -> OperationContract {
        OperationContract {
            id: "health".to_owned(),
            description: None,
            input: None,
            output: Some(DataSchema::Object {
                properties: BTreeMap::new(),
                required: Vec::new(),
            }),
            binding: OperationBinding::Http {
                method: HttpMethod::Get,
                path: "/manafield/health".to_owned(),
                codecs: vec![PayloadCodec::Json],
            },
        }
    }

    #[test]
    fn accepts_valid_module() {
        assert!(validate_module(&valid_module()).is_ok());
    }

    #[test]
    fn accepts_valid_health_operation_reference() {
        let mut module = valid_module();
        module.health_operation = Some("health".to_owned());
        module.operations.push(health_operation());

        assert!(validate_module(&module).is_ok());
    }

    #[test]
    fn rejects_missing_health_operation_reference() {
        let mut module = valid_module();
        module.health_operation = Some("health".to_owned());

        assert!(matches!(
            validate_module(&module),
            Err(ValidationError::UnknownHealthOperation(_))
        ));
    }

    #[test]
    fn rejects_health_operation_with_input() {
        let mut module = valid_module();
        let mut health = health_operation();
        health.input = Some(DataSchema::String);
        module.health_operation = Some("health".to_owned());
        module.operations.push(health);

        assert!(matches!(
            validate_module(&module),
            Err(ValidationError::HealthOperationHasInput(_))
        ));
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

    #[test]
    fn rejects_invalid_input_schema() {
        let mut module = valid_module();
        module.operations[0].input = Some(DataSchema::Object {
            properties: BTreeMap::new(),
            required: vec!["missing".to_owned()],
        });

        assert!(matches!(
            validate_module(&module),
            Err(ValidationError::InvalidInputSchema { .. })
        ));
    }

    #[test]
    fn accepts_capability_provides_and_requires() {
        let mut module = valid_module();
        module.provides.capabilities.push(CapabilityDescriptor {
            id: "manafield.identity".to_owned(),
            version: "1.0.0".to_owned(),
        });
        module.requires.capabilities.insert(
            "state".to_owned(),
            CapabilityRequirement {
                id: "database.postgresql".to_owned(),
                version: "^1.0.0".to_owned(),
            },
        );

        assert!(validate_module(&module).is_ok());
    }

    #[test]
    fn rejects_duplicate_provided_capability_ids() {
        let mut module = valid_module();
        module.provides.capabilities = vec![
            CapabilityDescriptor {
                id: "manafield.identity".to_owned(),
                version: "1.0.0".to_owned(),
            },
            CapabilityDescriptor {
                id: "manafield.identity".to_owned(),
                version: "1.1.0".to_owned(),
            },
        ];

        assert!(matches!(
            validate_module(&module),
            Err(ValidationError::DuplicateProvidedCapability(id))
                if id == "manafield.identity"
        ));
    }
}
