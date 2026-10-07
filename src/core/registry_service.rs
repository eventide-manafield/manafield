use std::sync::Arc;

use arc_swap::ArcSwap;
use tokio::sync::RwLock;

use super::{
    InstanceRegistry, ModuleDescriptor, RegistryError, RegistrySnapshot, ResourceDescriptor,
};

#[derive(Debug)]
pub struct RegistryService {
    registry: RwLock<InstanceRegistry>,
    snapshot: ArcSwap<RegistrySnapshot>,
}

impl RegistryService {
    pub fn new() -> Self {
        Self {
            registry: RwLock::new(InstanceRegistry::new()),
            snapshot: ArcSwap::from_pointee(RegistrySnapshot::default()),
        }
    }

    pub fn snapshot(&self) -> Arc<RegistrySnapshot> {
        self.snapshot.load_full()
    }

    pub async fn register(&self, module: ModuleDescriptor) -> Result<(), RegistryError> {
        self.register_module(module).await
    }

    pub async fn register_module(&self, module: ModuleDescriptor) -> Result<(), RegistryError> {
        let mut registry = self.registry.write().await;

        registry.register_module(module)?;
        self.publish_snapshot(&registry);

        Ok(())
    }

    pub async fn register_resource(
        &self,
        resource: ResourceDescriptor,
    ) -> Result<(), RegistryError> {
        let mut registry = self.registry.write().await;

        registry.register_resource(resource)?;
        self.publish_snapshot(&registry);

        Ok(())
    }

    pub async fn remove_module(&self, id: &str) -> Result<ModuleDescriptor, RegistryError> {
        let mut registry = self.registry.write().await;

        let removed = registry.remove_module(id)?;
        self.publish_snapshot(&registry);

        Ok(removed)
    }

    pub async fn remove_resource(&self, id: &str) -> Result<ResourceDescriptor, RegistryError> {
        let mut registry = self.registry.write().await;

        let removed = registry.remove_resource(id)?;
        self.publish_snapshot(&registry);

        Ok(removed)
    }

    fn publish_snapshot(&self, registry: &InstanceRegistry) {
        let snapshot = RegistrySnapshot::from_registry(registry);
        self.snapshot.store(Arc::new(snapshot));
    }
}

impl Default for RegistryService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::core::CapabilitySet;
    use crate::core::capability::CapabilityDescriptor;
    use crate::core::operation::{HttpMethod, OperationBinding, OperationContract, PayloadCodec};
    use crate::core::schema::DataSchema;

    fn module(id: &str) -> ModuleDescriptor {
        ModuleDescriptor {
            id: id.to_owned(),
            name: format!("{id} Module"),
            description: None,
            version: "0.0.1".to_owned(),
            health_operation: None,
            operations: vec![OperationContract {
                id: "hello".to_owned(),
                description: None,
                input: None,
                output: Some(DataSchema::Object {
                    properties: BTreeMap::new(),
                    required: Vec::new(),
                }),
                binding: OperationBinding::Http {
                    method: HttpMethod::Get,
                    path: "/hello".to_owned(),
                    codecs: vec![PayloadCodec::Json],
                },
            }],
        }
    }

    fn resource(id: &str) -> ResourceDescriptor {
        ResourceDescriptor {
            id: id.to_owned(),
            name: format!("{id} Resource"),
            resource_type: "sample".to_owned(),
            description: None,
            provides: CapabilitySet {
                capabilities: vec![CapabilityDescriptor {
                    id: "sample.capability".to_owned(),
                    version: "1.0.0".to_owned(),
                }],
            },
        }
    }

    #[tokio::test]
    async fn swaps_snapshot_after_registration() {
        let service = RegistryService::new();

        let before = service.snapshot();
        assert!(before.modules().is_empty());

        service
            .register(module("sample"))
            .await
            .expect("registration should succeed");

        let after = service.snapshot();

        assert!(before.modules().is_empty());
        assert_eq!(after.modules().len(), 1);
        assert_eq!(
            after.get_module("sample").map(|module| module.id.as_str()),
            Some("sample")
        );
    }

    #[tokio::test]
    async fn old_snapshot_remains_valid_after_swap() {
        let service = RegistryService::new();

        service
            .register(module("first"))
            .await
            .expect("first registration should succeed");

        let first_snapshot = service.snapshot();

        service
            .register(module("second"))
            .await
            .expect("second registration should succeed");

        let second_snapshot = service.snapshot();

        assert!(first_snapshot.get_module("first").is_some());
        assert!(first_snapshot.get_module("second").is_none());

        assert!(second_snapshot.get_module("first").is_some());
        assert!(second_snapshot.get_module("second").is_some());
    }

    #[tokio::test]
    async fn swaps_snapshot_after_removal() {
        let service = RegistryService::new();

        service
            .register(module("sample"))
            .await
            .expect("registration should succeed");

        let before_remove = service.snapshot();
        assert!(before_remove.get_module("sample").is_some());

        service
            .remove_module("sample")
            .await
            .expect("removal should succeed");

        let after_remove = service.snapshot();

        assert!(before_remove.get_module("sample").is_some());
        assert!(after_remove.get_module("sample").is_none());
    }

    #[tokio::test]
    async fn registers_resource_in_snapshot() {
        let service = RegistryService::new();

        service
            .register_resource(resource("sample-resource"))
            .await
            .expect("resource registration should succeed");

        let snapshot = service.snapshot();

        assert_eq!(snapshot.resources().len(), 1);
        assert_eq!(
            snapshot
                .get_resource("sample-resource")
                .map(|resource| resource.id.as_str()),
            Some("sample-resource")
        );
    }

    #[tokio::test]
    async fn rejects_instance_id_shared_by_module_and_resource() {
        let service = RegistryService::new();

        service
            .register(module("shared-id"))
            .await
            .expect("module registration should succeed");

        let error = service
            .register_resource(resource("shared-id"))
            .await
            .expect_err("resource registration should conflict");

        assert!(matches!(
            error,
            RegistryError::DuplicateInstance(id) if id == "shared-id"
        ));
    }
}
