use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use serde::Serialize;

use crate::core::{ModuleDescriptor, ModuleRegistry};

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

async fn list_modules(State(state): State<AppState>) -> Json<Vec<ModuleDescriptor>> {
    Json(state.registry.list())
}

async fn get_module(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ModuleDescriptor>, StatusCode> {
    state
        .registry
        .get(&id)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}
