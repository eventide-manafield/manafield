use serde::{Deserialize, Serialize};

use super::schema::DataSchema;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OperationContract {
    pub id: String,
    pub input: Option<DataSchema>,
    pub output: Option<DataSchema>,
    pub binding: OperationBinding,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum OperationBinding {
    Http {
        method: HttpMethod,
        path: String,
        codecs: Vec<PayloadCodec>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PayloadCodec {
    Json,
    MessagePack,
}
