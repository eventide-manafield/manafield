use std::error::Error;
use std::net::SocketAddr;
use std::path::PathBuf;

use axum::Router;
use manafield::api;
use manafield::core::{RegistryService, discover_modules};
use tokio::net::TcpListener;
use tracing::info;

const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8080";
const DEFAULT_MODULES_DIR: &str = "modules";

#[tokio::main]
async fn main() {
    if let Err(error) = manafield::logging::init() {
        eprintln!("manafield-core: cannot initialize logging: {error}");
        std::process::exit(1);
    }

    if let Err(error) = serve().await {
        eprintln!("manafield-core: {error}");
        std::process::exit(1);
    }
}

async fn serve() -> Result<(), Box<dyn Error>> {
    let bind_addr =
        std::env::var("MANAFIELD_BIND").unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_owned());

    let addr: SocketAddr = bind_addr.parse()?;

    let registry = load_registry().await?;
    let app = Router::new().merge(api::router(registry));

    let listener = TcpListener::bind(addr).await?;

    info!(%addr, "Manafield Core is listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
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

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to listen for shutdown signal");

    info!("shutdown signal received");
}
