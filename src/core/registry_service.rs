use std::sync::Arc;

use arc_swap::ArcSwap;
use tokio::sync::RwLock;

use super::{ModuleDescriptor, ModuleRegistry, RegistryError, RegistrySnapshot};

#[derive(Debug)]
pub struct RegistryService {
    registry: RwLock<ModuleRegistry>,
    snapshot: ArcSwap<RegistrySnapshot>,
}

impl RegistryService {
    pub fn new() -> Self {
        Self {
            registry: RwLock::new(ModuleRegistry::new()),
            snapshot: ArcSwap::from_pointee(RegistrySnapshot::default()),
        }
    }

    pub fn snapshot(&self) -> Arc<RegistrySnapshot> {
        self.snapshot.load_full()
    }

    pub async fn register(&self, module: ModuleDescriptor) -> Result<(), RegistryError> {
        let mut registry = self.registry.write().await;

        registry.register(module)?;

        let snapshot = RegistrySnapshot::from_registry(&registry);
        self.snapshot.store(Arc::new(snapshot));

        Ok(())
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
    use crate::core::operation::{HttpMethod, OperationBinding, OperationContract, PayloadCodec};
    use crate::core::schema::DataSchema;

    fn module(id: &str) -> ModuleDescriptor {
        ModuleDescriptor {
            id: id.to_owned(),
            name: format!("{id} Module"),
            version: "0.0.1".to_owned(),
            operations: vec![OperationContract {
                id: "hello".to_owned(),
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
            after.get("sample").map(|module| module.id.as_str()),
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

        assert!(first_snapshot.get("first").is_some());
        assert!(first_snapshot.get("second").is_none());

        assert!(second_snapshot.get("first").is_some());
        assert!(second_snapshot.get("second").is_some());
    }
}
