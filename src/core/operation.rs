use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Debug, Serialize)]
pub struct OperationContract {
    pub id: String,
    pub input: Option<Value>,
    pub output: Option<Value>,
    pub binding: OperationBinding,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum OperationBinding {
    Http { method: HttpMethod, path: String },
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
}
