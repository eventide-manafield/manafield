mod loader;
mod module;
mod operation;
mod registry;
mod registry_service;
mod schema;
mod snapshot;
mod validation;

pub use loader::discover_modules;
pub use module::ModuleDescriptor;
pub use operation::{OperationBinding, OperationContract};
pub use registry::{ModuleRegistry, RegistryError};
pub use registry_service::RegistryService;
pub use snapshot::RegistrySnapshot;
pub use validation::{ValidationError, validate_module};
