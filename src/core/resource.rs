use std::collections::HashSet;
use std::fmt;

use serde::{Deserialize, Serialize};

use super::CapabilitySet;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ResourceDescriptor {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub provides: CapabilitySet,
}

pub fn validate_resource(resource: &ResourceDescriptor) -> Result<(), ResourceValidationError> {
    if resource.id.trim().is_empty() {
        return Err(ResourceValidationError::EmptyResourceId);
    }

    if resource.name.trim().is_empty() {
        return Err(ResourceValidationError::EmptyResourceName);
    }

    if resource.resource_type.trim().is_empty() {
        return Err(ResourceValidationError::EmptyResourceType);
    }

    let mut capability_ids = HashSet::new();

    for capability in &resource.provides.capabilities {
        if capability.id.trim().is_empty() {
            return Err(ResourceValidationError::EmptyCapabilityId);
        }

        if capability.version.trim().is_empty() {
            return Err(ResourceValidationError::EmptyCapabilityVersion(
                capability.id.clone(),
            ));
        }

        if !capability_ids.insert(capability.id.as_str()) {
            return Err(ResourceValidationError::DuplicateCapability(
                capability.id.clone(),
            ));
        }
    }

    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResourceValidationError {
    EmptyResourceId,
    EmptyResourceName,
    EmptyResourceType,
    EmptyCapabilityId,
    EmptyCapabilityVersion(String),
    DuplicateCapability(String),
}

impl fmt::Display for ResourceValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyResourceId => write!(f, "resource id must not be empty"),
            Self::EmptyResourceName => write!(f, "resource name must not be empty"),
            Self::EmptyResourceType => write!(f, "resource type must not be empty"),
            Self::EmptyCapabilityId => write!(f, "capability id must not be empty"),
            Self::EmptyCapabilityVersion(id) => {
                write!(f, "capability '{id}' version must not be empty")
            }
            Self::DuplicateCapability(id) => {
                write!(f, "capability '{id}' is declared more than once")
            }
        }
    }
}

impl std::error::Error for ResourceValidationError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::CapabilitySet;
    use crate::core::capability::CapabilityDescriptor;

    fn resource() -> ResourceDescriptor {
        ResourceDescriptor {
            id: "sample-resource".to_owned(),
            name: "Sample Resource".to_owned(),
            resource_type: "sample".to_owned(),
            description: None,
            provides: CapabilitySet {
                capabilities: vec![CapabilityDescriptor {
                    id: "sample.capability".to_owned(),
                    version: "1.0.0".to_owned(),
                }],
            },
        }
    }

    #[test]
    fn accepts_valid_resource() {
        assert_eq!(validate_resource(&resource()), Ok(()));
    }

    #[test]
    fn rejects_duplicate_capability_ids() {
        let mut resource = resource();
        resource
            .provides
            .capabilities
            .push(resource.provides.capabilities[0].clone());

        assert_eq!(
            validate_resource(&resource),
            Err(ResourceValidationError::DuplicateCapability(
                "sample.capability".to_owned()
            ))
        );
    }
}
