use std::collections::BTreeMap;

use super::{ModuleDescriptor, ModuleRegistry};

#[derive(Debug, Default)]
pub struct RegistrySnapshot {
    modules: Vec<ModuleDescriptor>,
    indexes: BTreeMap<String, usize>,
}

impl RegistrySnapshot {
    pub fn from_registry(registry: &ModuleRegistry) -> Self {
        let modules = registry.list();
        let indexes = modules
            .iter()
            .enumerate()
            .map(|(index, module)| (module.id.clone(), index))
            .collect();

        Self { modules, indexes }
    }

    pub fn modules(&self) -> &[ModuleDescriptor] {
        &self.modules
    }

    pub fn get(&self, id: &str) -> Option<&ModuleDescriptor> {
        self.indexes
            .get(id)
            .and_then(|index| self.modules.get(*index))
    }
}
