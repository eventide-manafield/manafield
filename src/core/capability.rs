use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct CapabilitySet {
    #[serde(default)]
    pub capabilities: Vec<CapabilityDescriptor>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CapabilityDescriptor {
    pub id: String,
    pub version: String,
}
