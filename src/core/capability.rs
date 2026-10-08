use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct CapabilitySet {
    #[serde(default)]
    pub capabilities: Vec<CapabilityDescriptor>,
}

impl CapabilitySet {
    pub fn is_empty(&self) -> bool {
        self.capabilities.is_empty()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CapabilityDescriptor {
    pub id: String,
    pub version: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct CapabilityRequirementSet {
    #[serde(default)]
    pub capabilities: BTreeMap<String, CapabilityRequirement>,
}

impl CapabilityRequirementSet {
    pub fn is_empty(&self) -> bool {
        self.capabilities.is_empty()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CapabilityRequirement {
    pub id: String,
    pub version: String,
}
