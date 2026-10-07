use std::collections::BTreeMap;
use std::fmt;

use super::{
    ModuleDescriptor, ResourceDescriptor, ResourceValidationError, ValidationError,
    validate_module, validate_resource,
};

#[derive(Debug, Default)]
pub struct InstanceRegistry {
    modules: BTreeMap<String, ModuleDescriptor>,
    resources: BTreeMap<String, ResourceDescriptor>,
}

impl InstanceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_module(&mut self, module: ModuleDescriptor) -> Result<(), RegistryError> {
        validate_module(&module).map_err(RegistryError::InvalidModule)?;
        self.ensure_available_id(&module.id)?;

        self.modules.insert(module.id.clone(), module);
        Ok(())
    }

    pub fn register_resource(&mut self, resource: ResourceDescriptor) -> Result<(), RegistryError> {
        validate_resource(&resource).map_err(RegistryError::InvalidResource)?;
        self.ensure_available_id(&resource.id)?;

        self.resources.insert(resource.id.clone(), resource);
        Ok(())
    }

    pub fn remove_module(&mut self, id: &str) -> Result<ModuleDescriptor, RegistryError> {
        self.modules
            .remove(id)
            .ok_or_else(|| RegistryError::ModuleNotFound(id.to_owned()))
    }

    pub fn remove_resource(&mut self, id: &str) -> Result<ResourceDescriptor, RegistryError> {
        self.resources
            .remove(id)
            .ok_or_else(|| RegistryError::ResourceNotFound(id.to_owned()))
    }

    pub fn list_modules(&self) -> Vec<ModuleDescriptor> {
        self.modules.values().cloned().collect()
    }

    pub fn list_resources(&self) -> Vec<ResourceDescriptor> {
        self.resources.values().cloned().collect()
    }

    fn ensure_available_id(&self, id: &str) -> Result<(), RegistryError> {
        if self.modules.contains_key(id) || self.resources.contains_key(id) {
            Err(RegistryError::DuplicateInstance(id.to_owned()))
        } else {
            Ok(())
        }
    }
}

#[derive(Debug)]
pub enum RegistryError {
    DuplicateInstance(String),
    ModuleNotFound(String),
    ResourceNotFound(String),
    InvalidModule(ValidationError),
    InvalidResource(ResourceValidationError),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateInstance(id) => write!(f, "instance '{id}' is already registered"),
            Self::ModuleNotFound(id) => write!(f, "module '{id}' is not registered"),
            Self::ResourceNotFound(id) => write!(f, "resource '{id}' is not registered"),
            Self::InvalidModule(error) => write!(f, "invalid module: {error}"),
            Self::InvalidResource(error) => write!(f, "invalid resource: {error}"),
        }
    }
}

impl std::error::Error for RegistryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DuplicateInstance(_) | Self::ModuleNotFound(_) | Self::ResourceNotFound(_) => {
                None
            }
            Self::InvalidModule(error) => Some(error),
            Self::InvalidResource(error) => Some(error),
        }
    }
}
