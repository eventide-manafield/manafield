use std::sync::Arc;

use axum::{
    Json, Router,
    body::Body,
    extract::{Path, State},
    http::{
        HeaderMap, StatusCode,
        header::{ACCEPT, CONTENT_TYPE},
    },
    response::Response,
    routing::get,
};
use serde::Serialize;

use crate::core::ModuleRegistry;

const JSON_MEDIA_TYPE: &str = "application/json";
const MESSAGEPACK_MEDIA_TYPE: &str = "application/msgpack";

#[derive(Clone)]
struct AppState {
    registry: Arc<ModuleRegistry>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    version: &'static str,
}

pub fn router(registry: ModuleRegistry) -> Router {
    let state = AppState {
        registry: Arc::new(registry),
    };

    Router::new()
        .route("/health", get(health))
        .route("/modules", get(list_modules))
        .route("/modules/{id}", get(get_module))
        .with_state(state)
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "manafield-core",
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn list_modules(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    encode_response(&headers, &state.registry.list())
}

async fn get_module(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let module = state.registry.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    encode_response(&headers, &module)
}

fn encode_response<T>(headers: &HeaderMap, value: &T) -> Result<Response, StatusCode>
where
    T: Serialize,
{
    let wants_messagepack = headers
        .get(ACCEPT)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|accept| accept.contains(MESSAGEPACK_MEDIA_TYPE));

    let (content_type, body) = if wants_messagepack {
        (
            MESSAGEPACK_MEDIA_TYPE,
            rmp_serde::to_vec_named(value).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        )
    } else {
        (
            JSON_MEDIA_TYPE,
            serde_json::to_vec(value).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        )
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, content_type)
        .body(Body::from(body))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
