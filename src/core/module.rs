use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Debug, Serialize)]
pub struct ModuleDescriptor {
    pub id: String,
    pub name: String,
    pub version: String,
    pub apis: Vec<ApiContract>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ApiContract {
    pub id: String,
    pub method: HttpMethod,
    pub path: String,
    pub input: Option<Value>,
    pub output: Option<Value>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
}
