mod api;
mod core;

use std::net::SocketAddr;

use axum::Router;
use core::{
    HttpMethod, ModuleDescriptor, ModuleRegistry, OperationBinding, OperationContract, PayloadCodec,
};
use serde_json::json;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::EnvFilter;

const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8080";

#[tokio::main]
async fn main() {
    init_tracing();

    let bind_addr =
        std::env::var("MANAFIELD_BIND").unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_owned());

    let addr: SocketAddr = bind_addr
        .parse()
        .expect("MANAFIELD_BIND must be a valid socket address");

    let registry = initial_registry();
    let app = Router::new().merge(api::router(registry));

    let listener = TcpListener::bind(addr)
        .await
        .expect("failed to bind Manafield Core");

    info!(%addr, "Manafield Core is listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Manafield Core server failed");
}

fn initial_registry() -> ModuleRegistry {
    let mut registry = ModuleRegistry::new();

    registry
        .register(ModuleDescriptor {
            id: "sample".to_owned(),
            name: "Sample Module".to_owned(),
            version: "0.0.1".to_owned(),
            operations: vec![
                OperationContract {
                    id: "hello".to_owned(),
                    input: None,
                    output: Some(json!({
                        "type": "object",
                        "required": ["message"],
                        "properties": {
                            "message": { "type": "string" }
                        }
                    })),
                    binding: OperationBinding::Http {
                        method: HttpMethod::Get,
                        path: "/hello".to_owned(),
                        codecs: vec![PayloadCodec::Json, PayloadCodec::MessagePack],
                    },
                },
                OperationContract {
                    id: "echo".to_owned(),
                    input: Some(json!({
                        "type": "object",
                        "required": ["message"],
                        "properties": {
                            "message": { "type": "string" }
                        }
                    })),
                    output: Some(json!({
                        "type": "object",
                        "required": ["message"],
                        "properties": {
                            "message": { "type": "string" }
                        }
                    })),
                    binding: OperationBinding::Http {
                        method: HttpMethod::Post,
                        path: "/echo".to_owned(),
                        codecs: vec![PayloadCodec::Json, PayloadCodec::MessagePack],
                    },
                },
            ],
        })
        .expect("initial module registry must be valid");

    registry
}

fn init_tracing() {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("manafield=info"));

    tracing_subscriber::fmt().with_env_filter(filter).init();
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to listen for shutdown signal");

    info!("shutdown signal received");
}
