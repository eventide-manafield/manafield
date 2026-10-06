use serde::{Deserialize, Serialize};

use super::OperationContract;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModuleDescriptor {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(
        rename = "healthOperation",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub health_operation: Option<String>,
    pub operations: Vec<OperationContract>,
}
