use std::collections::BTreeMap;

use super::{InstanceRegistry, ModuleDescriptor, ResourceDescriptor};

#[derive(Debug, Default)]
pub struct RegistrySnapshot {
    modules: Vec<ModuleDescriptor>,
    module_indexes: BTreeMap<String, usize>,
    resources: Vec<ResourceDescriptor>,
    resource_indexes: BTreeMap<String, usize>,
}

impl RegistrySnapshot {
    pub fn from_registry(registry: &InstanceRegistry) -> Self {
        let modules = registry.list_modules();
        let module_indexes = modules
            .iter()
            .enumerate()
            .map(|(index, module)| (module.id.clone(), index))
            .collect();

        let resources = registry.list_resources();
        let resource_indexes = resources
            .iter()
            .enumerate()
            .map(|(index, resource)| (resource.id.clone(), index))
            .collect();

        Self {
            modules,
            module_indexes,
            resources,
            resource_indexes,
        }
    }

    pub fn modules(&self) -> &[ModuleDescriptor] {
        &self.modules
    }

    pub fn resources(&self) -> &[ResourceDescriptor] {
        &self.resources
    }

    pub fn get_module(&self, id: &str) -> Option<&ModuleDescriptor> {
        self.module_indexes
            .get(id)
            .and_then(|index| self.modules.get(*index))
    }

    pub fn get_resource(&self, id: &str) -> Option<&ResourceDescriptor> {
        self.resource_indexes
            .get(id)
            .and_then(|index| self.resources.get(*index))
    }
}
