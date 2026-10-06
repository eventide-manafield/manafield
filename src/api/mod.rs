use std::sync::Arc;

use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::{Path, State},
    http::{
        HeaderMap, StatusCode,
        header::{ACCEPT, CONTENT_TYPE},
    },
    response::Response,
    routing::get,
};
use serde::{Serialize, de::DeserializeOwned};

use crate::core::{ModuleDescriptor, RegistryError, RegistryService};

const JSON_MEDIA_TYPE: &str = "application/json";
const MESSAGEPACK_MEDIA_TYPE: &str = "application/msgpack";

#[derive(Clone)]
struct AppState {
    registry: Arc<RegistryService>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    version: &'static str,
}

pub fn router(registry: RegistryService) -> Router {
    let state = AppState {
        registry: Arc::new(registry),
    };

    Router::new()
        .route("/health", get(health))
        .route("/modules", get(list_modules).post(register_module))
        .route("/modules/{id}", get(get_module).delete(remove_module))
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
    let snapshot = state.registry.snapshot();
    encode_response(&headers, snapshot.modules(), StatusCode::OK)
}

async fn get_module(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let snapshot = state.registry.snapshot();
    let module = snapshot.get(&id).ok_or(StatusCode::NOT_FOUND)?;

    encode_response(&headers, module, StatusCode::OK)
}

async fn register_module(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, StatusCode> {
    let module: ModuleDescriptor = decode_request(&headers, &body)?;

    state
        .registry
        .register(module.clone())
        .await
        .map_err(registry_error_status)?;

    encode_response(&headers, &module, StatusCode::CREATED)
}

async fn remove_module(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let removed = state
        .registry
        .remove(&id)
        .await
        .map_err(registry_error_status)?;

    encode_response(&headers, &removed, StatusCode::OK)
}

fn decode_request<T>(headers: &HeaderMap, body: &[u8]) -> Result<T, StatusCode>
where
    T: DeserializeOwned,
{
    let content_type = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or(JSON_MEDIA_TYPE);

    if content_type.starts_with(MESSAGEPACK_MEDIA_TYPE) {
        rmp_serde::from_slice(body).map_err(|_| StatusCode::BAD_REQUEST)
    } else if content_type.starts_with(JSON_MEDIA_TYPE) {
        serde_json::from_slice(body).map_err(|_| StatusCode::BAD_REQUEST)
    } else {
        Err(StatusCode::UNSUPPORTED_MEDIA_TYPE)
    }
}

fn registry_error_status(error: RegistryError) -> StatusCode {
    match error {
        RegistryError::DuplicateModule(_) => StatusCode::CONFLICT,
        RegistryError::ModuleNotFound(_) => StatusCode::NOT_FOUND,
        RegistryError::InvalidModule(_) => StatusCode::BAD_REQUEST,
    }
}

fn encode_response<T>(
    headers: &HeaderMap,
    value: &T,
    status: StatusCode,
) -> Result<Response, StatusCode>
where
    T: Serialize + ?Sized,
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
        .status(status)
        .header(CONTENT_TYPE, content_type)
        .body(Body::from(body))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
