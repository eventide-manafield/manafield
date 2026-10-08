use serde::{Deserialize, Serialize};

use super::{CapabilityRequirementSet, CapabilitySet, OperationContract};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModuleDescriptor {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub version: String,
    #[serde(
        rename = "healthOperation",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub health_operation: Option<String>,
    #[serde(default, skip_serializing_if = "CapabilitySet::is_empty")]
    pub provides: CapabilitySet,
    #[serde(default, skip_serializing_if = "CapabilityRequirementSet::is_empty")]
    pub requires: CapabilityRequirementSet,
    pub operations: Vec<OperationContract>,
}
