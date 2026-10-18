//! Exclusive Redis Resource binding for direct Release staging.
//!
//! The Redis Resource owns a password; consumers get mounted read-only
//! credentials. The stable per-module prefix is for namespacing only, NOT
//! Redis ACL isolation. Only the declared Module and slot receive credentials.
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use super::secrets::create_secret;
use super::{CliError, fail};
use crate::build_plan::ExclusiveConsumer;

pub(super) struct RedisRuntime {
    pub(super) id: String,
    password_path: PathBuf,
    owner: ExclusiveConsumer,
}

impl RedisRuntime {
    pub(super) fn prepare(resource: &Value, root: &Path) -> Result<Self, CliError> {
        let id = resource["id"]
            .as_str()
            .filter(|id| {
                !id.is_empty()
                    && id.len() <= 63
                    && id.bytes().next().is_some_and(|b| b.is_ascii_alphanumeric())
                    && id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
                    && !id.contains("..")
            })
            .ok_or_else(|| fail("invalid Redis Resource ID"))?;
        let owner: ExclusiveConsumer = serde_json::from_value(resource["exclusiveTo"].clone())
            .map_err(|_| fail("Redis Resource requires exclusiveTo with module and slot"))?;
        owner.validate().map_err(fail)?;
        let password_path = root.join("secrets/redis").join(id).join("password");
        create_secret(&password_path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let parent = password_path
                .parent()
                .ok_or_else(|| fail("invalid Redis secret directory"))?;
            let directory_mode = fs::metadata(parent)
                .map_err(|_| fail("cannot inspect Redis secret directory"))?
                .permissions()
                .mode();
            let file_mode = fs::metadata(&password_path)
                .map_err(|_| fail("cannot inspect Redis secret file"))?
                .permissions()
                .mode();
            if directory_mode & 0o077 != 0 || file_mode & 0o777 != 0o444 {
                return Err(fail(
                    "Redis secret requires a private host directory and a 0444 read-only consumer file; review existing ownership and permissions explicitly",
                ));
            }
        }
        let content = fs::read_to_string(&password_path)
            .map_err(|_| fail("cannot read Redis Resource secret file"))?;
        if content.trim_end_matches(['\r', '\n']).len() != 48
            || !content
                .trim_end_matches(['\r', '\n'])
                .bytes()
                .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
        {
            return Err(fail(
                "Redis Resource secret must be a 48-byte lowercase hex password",
            ));
        }
        Ok(Self {
            id: id.to_owned(),
            password_path,
            owner,
        })
    }

    pub(super) fn password_file(&self) -> &Path {
        &self.password_path
    }

    pub(super) fn inject(
        &self,
        module_id: &str,
        slot: &str,
        manifest: &Value,
        env: &mut Map<String, Value>,
        mounts: &mut Vec<Value>,
    ) -> Result<(), CliError> {
        self.owner
            .authorize(&self.id, module_id, slot)
            .map_err(fail)?;
        if manifest["requires"]["capabilities"][slot]["id"] != "cache.redis" {
            return Err(fail(format!(
                "Module '{module_id}' slot '{slot}' must require cache.redis to bind Redis Resource",
            )));
        }
        if module_id.is_empty()
            || !module_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
            || !slot.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            || slot.is_empty()
        {
            return Err(fail("unsafe Module ID or Redis Requirement slot"));
        }
        let key = slot.to_ascii_uppercase();
        let prefix = format!(
            "mf_{}_{}",
            module_id.replace('-', "_").to_ascii_lowercase(),
            slot.to_ascii_lowercase(),
        );
        if prefix.len() > 63 {
            return Err(fail(
                "Redis key prefix exceeds the consumer contract (63 characters)",
            ));
        }
        let target = format!("/run/manafield/bindings/{slot}/password");
        for (suffix, value) in [
            ("ENDPOINT_HOST", "manafield-redis".to_owned()),
            ("ENDPOINT_PORT", "6379".to_owned()),
            ("CONFIG_PREFIX", prefix),
            ("SECRET_PASSWORD_FILE", target.clone()),
        ] {
            env.insert(format!("MANAFIELD_BINDING_{key}_{suffix}"), json!(value));
        }
        mounts.push(json!({
            "type": "bind",
            "source": self.password_path,
            "target": target,
            "read_only": true,
        }));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn sandbox() -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "manafield-redis-staging-{}-{suffix}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn creates_stable_secret_and_injects_cache_binding() {
        let root = sandbox();
        let redis = RedisRuntime::prepare(&json!({"id":"noctemblith-redis","exclusiveTo":{"module":"noctemblith","slot":"cache"}}), &root).unwrap();
        let first = fs::read(redis.password_file()).unwrap();
        assert_eq!(first.len(), 49);
        assert_eq!(
            RedisRuntime::prepare(&json!({"id":"noctemblith-redis","exclusiveTo":{"module":"noctemblith","slot":"cache"}}), &root)
                .unwrap()
                .password_file(),
            redis.password_file()
        );
        assert_eq!(first, fs::read(redis.password_file()).unwrap());
        let manifest =
            json!({"requires":{"capabilities":{"cache":{"id":"cache.redis","version":"^1.0.0"}}}});
        let mut env = Map::new();
        let mut mounts = Vec::new();
        redis
            .inject("noctemblith", "cache", &manifest, &mut env, &mut mounts)
            .unwrap();
        assert_eq!(
            env["MANAFIELD_BINDING_CACHE_CONFIG_PREFIX"],
            "mf_noctemblith_cache"
        );
        assert_eq!(
            env["MANAFIELD_BINDING_CACHE_ENDPOINT_HOST"],
            "manafield-redis"
        );
        assert_eq!(mounts.len(), 1);
        assert!(
            !serde_json::to_string(&env)
                .unwrap()
                .contains(&String::from_utf8(first).unwrap())
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn wrong_capability_never_receives_redis_password() {
        let root = sandbox();
        let redis = RedisRuntime::prepare(&json!({"id":"noctemblith-redis","exclusiveTo":{"module":"noctemblith","slot":"cache"}}), &root).unwrap();
        let manifest = json!({"requires":{"capabilities":{"cache":{"id":"database.postgresql","version":"^1.0.0"}}}});
        let mut env = Map::new();
        let mut mounts = Vec::new();
        assert!(
            redis
                .inject("noctemblith", "cache", &manifest, &mut env, &mut mounts)
                .is_err()
        );
        assert!(mounts.is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn forbidden_consumer_never_receives_environment_or_secret_mount() {
        let root = sandbox();
        let redis = RedisRuntime::prepare(
            &json!({
                "id":"noctemblith-redis",
                "exclusiveTo":{"module":"noctemblith","slot":"cache"}
            }),
            &root,
        )
        .unwrap();
        for (module, slot) in [("other", "cache"), ("noctemblith", "other_cache")] {
            let mut env = Map::new();
            let mut mounts = Vec::new();
            let manifest = json!({"requires":{"capabilities":{
                slot: {"id":"cache.redis","version":"^1.0.0"}
            }}});
            assert!(
                redis
                    .inject(module, slot, &manifest, &mut env, &mut mounts)
                    .is_err()
            );
            assert!(env.is_empty());
            assert!(mounts.is_empty());
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_exclusivity_fails_before_secret_creation() {
        let root = sandbox();
        assert!(RedisRuntime::prepare(&json!({"id":"noctemblith-redis"}), &root).is_err());
        assert!(!root.join("secrets").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn separate_resource_ids_get_separate_stable_secrets() {
        let root = sandbox();
        let first = RedisRuntime::prepare(
            &json!({
                "id":"first-redis","exclusiveTo":{"module":"first","slot":"cache"}
            }),
            &root,
        )
        .unwrap();
        let second = RedisRuntime::prepare(
            &json!({
                "id":"second-redis","exclusiveTo":{"module":"second","slot":"cache"}
            }),
            &root,
        )
        .unwrap();
        assert_ne!(first.password_file(), second.password_file());
        assert_ne!(
            fs::read(first.password_file()).unwrap(),
            fs::read(second.password_file()).unwrap()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn incompatible_existing_secret_permissions_are_rejected_without_repair() {
        use std::os::unix::fs::PermissionsExt;
        let root = sandbox();
        let resource =
            json!({"id":"dedicated-redis","exclusiveTo":{"module":"app","slot":"cache"}});
        let redis = RedisRuntime::prepare(&resource, &root).unwrap();
        fs::set_permissions(redis.password_file(), fs::Permissions::from_mode(0o600)).unwrap();
        assert!(RedisRuntime::prepare(&resource, &root).is_err());
        assert_eq!(
            fs::metadata(redis.password_file())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        fs::set_permissions(redis.password_file(), fs::Permissions::from_mode(0o444)).unwrap();
        fs::set_permissions(
            redis.password_file().parent().unwrap(),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        assert!(RedisRuntime::prepare(&resource, &root).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
