mod api;
mod core;

use std::error::Error;
use std::net::SocketAddr;
use std::path::PathBuf;

use axum::Router;
use core::{RegistryService, discover_modules};
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::EnvFilter;

const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8080";
const DEFAULT_MODULES_DIR: &str = "modules";

#[tokio::main]
async fn main() {
    init_tracing();

    let bind_addr =
        std::env::var("MANAFIELD_BIND").unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_owned());

    let addr: SocketAddr = bind_addr
        .parse()
        .expect("MANAFIELD_BIND must be a valid socket address");

    let registry = load_registry()
        .await
        .unwrap_or_else(|error| panic!("failed to load Manafield modules: {error}"));
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

async fn load_registry() -> Result<RegistryService, Box<dyn Error>> {
    let modules_dir = PathBuf::from(
        std::env::var("MANAFIELD_MODULES_DIR").unwrap_or_else(|_| DEFAULT_MODULES_DIR.to_owned()),
    );

    let modules = discover_modules(&modules_dir)?;
    let module_count = modules.len();
    let registry = RegistryService::new();

    for module in modules {
        registry.register(module).await?;
    }

    info!(
        path = %modules_dir.display(),
        modules = module_count,
        "Manafield modules loaded"
    );

    Ok(registry)
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
