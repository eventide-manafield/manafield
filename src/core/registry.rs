use std::collections::BTreeMap;
use std::fmt;

use super::{ModuleDescriptor, ValidationError, validate_module};

#[derive(Debug, Default)]
pub struct ModuleRegistry {
    modules: BTreeMap<String, ModuleDescriptor>,
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, module: ModuleDescriptor) -> Result<(), RegistryError> {
        validate_module(&module).map_err(RegistryError::InvalidModule)?;

        if self.modules.contains_key(&module.id) {
            return Err(RegistryError::DuplicateModule(module.id));
        }

        self.modules.insert(module.id.clone(), module);
        Ok(())
    }

    pub fn remove(&mut self, id: &str) -> Result<ModuleDescriptor, RegistryError> {
        self.modules
            .remove(id)
            .ok_or_else(|| RegistryError::ModuleNotFound(id.to_owned()))
    }

    pub fn list(&self) -> Vec<ModuleDescriptor> {
        self.modules.values().cloned().collect()
    }
}

#[derive(Debug)]
pub enum RegistryError {
    DuplicateModule(String),
    ModuleNotFound(String),
    InvalidModule(ValidationError),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateModule(id) => write!(f, "module '{id}' is already registered"),
            Self::ModuleNotFound(id) => write!(f, "module '{id}' is not registered"),
            Self::InvalidModule(error) => write!(f, "invalid module: {error}"),
        }
    }
}

impl std::error::Error for RegistryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DuplicateModule(_) | Self::ModuleNotFound(_) => None,
            Self::InvalidModule(error) => Some(error),
        }
    }
}
