use std::collections::BTreeMap;
use std::fmt;

use super::ModuleDescriptor;

#[derive(Debug, Default)]
pub struct ModuleRegistry {
    modules: BTreeMap<String, ModuleDescriptor>,
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, module: ModuleDescriptor) -> Result<(), RegistryError> {
        if self.modules.contains_key(&module.id) {
            return Err(RegistryError::DuplicateModule(module.id));
        }

        self.modules.insert(module.id.clone(), module);
        Ok(())
    }

    pub fn list(&self) -> Vec<ModuleDescriptor> {
        self.modules.values().cloned().collect()
    }

    pub fn get(&self, id: &str) -> Option<ModuleDescriptor> {
        self.modules.get(id).cloned()
    }
}

#[derive(Debug)]
pub enum RegistryError {
    DuplicateModule(String),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateModule(id) => write!(f, "module '{id}' is already registered"),
        }
    }
}

impl std::error::Error for RegistryError {}
