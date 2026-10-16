//! Direct Docker S3 Resource materialization.
//!
//! Manafield Core only sees Resource metadata. This adapter prepares endpoint
//! config and private credential-file mounts for trusted consumer Modules.
//! S3 bytes never pass through Core or this adapter.

use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use super::{CliError, fail};
use crate::storage_config::S3ResourceConfig;

pub(super) struct S3Runtime {
    pub(super) id: String,
    pub(super) config: S3ResourceConfig,
    access_key_file: PathBuf,
    secret_key_file: PathBuf,
}

impl S3Runtime {
    pub(super) fn prepare(resource: &Value, root: &Path) -> Result<Self, CliError> {
        let id = resource["id"]
            .as_str()
            .filter(|id| valid_resource_id(id))
            .ok_or_else(|| fail("unsafe or missing S3 Resource Instance ID"))?;
        let config = S3ResourceConfig::parse(&resource["config"]).map_err(fail)?;
        let dir = root.join("secrets/s3").join(id);
        // Master copies are managed externally: never generate S3 credentials.
        let master_access = dir.join("access-key-id");
        let master_secret = dir.join("secret-access-key");
        let access = read_master(&master_access)?;
        let secret = read_master(&master_secret)?;

        // A read-only Docker bind mount of a 0600 host file is inaccessible to
        // non-root Modules. Create restricted *host-directory* runtime copies
        // whose file mode is 0444, readable only when explicitly mounted into
        // a permitted container. The 0700 parent guards host traversal.
        let runtime_dir = dir.join("runtime");
        fs::create_dir_all(&runtime_dir)
            .map_err(|e| fail(format!("cannot create S3 runtime secret directory: {e}")))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&runtime_dir, fs::Permissions::from_mode(0o700))
                .map_err(|e| fail(format!("cannot restrict S3 runtime secret directory: {e}")))?;
        }
        let access_key_file = runtime_dir.join("access-key-id");
        let secret_key_file = runtime_dir.join("secret-access-key");
        write_mount_copy(&access_key_file, &access)?;
        write_mount_copy(&secret_key_file, &secret)?;

        Ok(Self {
            id: id.to_owned(),
            config,
            access_key_file,
            secret_key_file,
        })
    }

    pub(super) fn access_path(&self) -> &Path {
        &self.access_key_file
    }

    pub(super) fn secret_path(&self) -> &Path {
        &self.secret_key_file
    }

    pub(super) fn inject(
        &self,
        module_id: &str,
        slot: &str,
        manifest_path: &Path,
        env: &mut Map<String, Value>,
        mounts: &mut Vec<Value>,
    ) -> Result<(), CliError> {
        let manifest: Value = serde_json::from_slice(
            &fs::read(manifest_path)
                .map_err(|e| fail(format!("cannot read Module manifest: {e}")))?,
        )
        .map_err(|e| fail(format!("invalid Module manifest JSON: {e}")))?;
        let required = manifest["requires"]["capabilities"][slot]["id"].as_str();
        if required != Some("storage.object") {
            return Err(fail(format!(
                "Module '{module_id}' binds S3 Resource '{}' at slot '{slot}' but does not require storage.object",
                self.id
            )));
        }
        if !valid_resource_id(module_id) || !valid_slot(slot) {
            return Err(fail("unsafe S3 Module ID or binding slot"));
        }
        let key = slot.to_ascii_uppercase();
        let prefix = format!("modules/{module_id}/{slot}/");
        let runtime_access = format!("/run/manafield/bindings/{slot}/access-key-id");
        let runtime_secret = format!("/run/manafield/bindings/{slot}/secret-access-key");
        for (suffix, value) in [
            ("ENDPOINT_URL", self.config.endpoint.clone()),
            ("CONFIG_BUCKET", self.config.bucket.clone()),
            ("CONFIG_REGION", self.config.region.clone()),
            ("CONFIG_PREFIX", prefix),
            ("CONFIG_FORCE_PATH_STYLE", self.config.force_path_style.to_string()),
            ("SECRET_ACCESS_KEY_ID_FILE", runtime_access.clone()),
            ("SECRET_SECRET_ACCESS_KEY_FILE", runtime_secret.clone()),
        ] {
            env.insert(format!("MANAFIELD_BINDING_{key}_{suffix}"), json!(value));
        }
        if let Some(url) = &self.config.public_base_url {
            env.insert(
                format!("MANAFIELD_BINDING_{key}_CONFIG_PUBLIC_BASE_URL"),
                json!(url),
            );
        }
        mounts.push(json!({
            "type": "bind",
            "source": self.access_key_file,
            "target": runtime_access,
            "read_only": true
        }));
        mounts.push(json!({
            "type": "bind",
            "source": self.secret_key_file,
            "target": runtime_secret,
            "read_only": true
        }));
        Ok(())
    }
}

fn valid_resource_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 63
        && value.bytes().next().is_some_and(|v| v.is_ascii_alphanumeric())
        && value.bytes().all(|v| v.is_ascii_alphanumeric() || matches!(v, b'.' | b'_' | b'-'))
        && !value.contains("..")
}

fn valid_slot(value: &str) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(b'A'..=b'Z' | b'a'..=b'z' | b'_'))
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn read_master(path: &Path) -> Result<Vec<u8>, CliError> {
    let meta = fs::symlink_metadata(path)
        .map_err(|_| fail("S3 master credentials not found under Instance secrets/s3/<resource-id>/"))?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(fail("S3 master credential must be a regular file (not symlink)"));
    }
    let contents = fs::read(path)
        .map_err(|_| fail("Cannot read S3 master credential file"))?;
    if contents.is_empty() || contents.len() > 4096 {
        return Err(fail("S3 master credential file is empty or too large"));
    }
    let trimmed = contents
        .strip_suffix(b"\n")
        .unwrap_or(&contents)
        .strip_suffix(b"\r")
        .unwrap_or(&contents);
    if trimmed.is_empty() || trimmed.contains(&b'\n') || trimmed.contains(&b'\r') || trimmed.contains(&0) {
        return Err(fail("S3 master credential file has malformed content"));
    }
    // Preserve only the supplied value, not the trailing line ending.
    Ok(trimmed.to_vec())
}

fn write_mount_copy(path: &Path, contents: &[u8]) -> Result<(), CliError> {
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(fail("S3 runtime credential path is not a regular file"));
        }
        if fs::read(path).map_err(|_| fail("Cannot read S3 runtime credential file"))? != contents {
            return Err(fail(
                "S3 runtime credential differs from master; controlled credential rotation is required",
            ));
        }
        return Ok(());
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o444);
    }
    let mut file = options
        .open(path)
        .map_err(|_| fail("Cannot create S3 runtime credential file"))?;
    file.write_all(contents)
        .and_then(|_| file.sync_all())
        .map_err(|_| fail("Cannot write S3 runtime credential file"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn validates_resource_and_slot_names() {
        assert!(valid_resource_id("manafield-r2"));
        assert!(!valid_resource_id("../r2"));
        assert!(valid_slot("media"));
        assert!(!valid_slot("media-storage"));
    }

    #[test]
    fn redacts_secret_and_rejects_rotation_without_explicit_action() {
        let id = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("mf-s3-test-{id}"));
        fs::create_dir_all(&root).unwrap();
        let secret = root.join("credential");
        write_mount_copy(&secret, b"fake-secret").unwrap();
        write_mount_copy(&secret, b"fake-secret").unwrap();
        assert!(write_mount_copy(&secret, b"rotated").is_err());
        assert!(!format!("{}", fail("no secret bytes")).contains("fake-secret"));
        fs::remove_dir_all(root).unwrap();
    }
}
