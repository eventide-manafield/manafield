use serde::Serialize;

use super::OperationContract;

#[derive(Clone, Debug, Serialize)]
pub struct ModuleDescriptor {
    pub id: String,
    pub name: String,
    pub version: String,
    pub operations: Vec<OperationContract>,
}
