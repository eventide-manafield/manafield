//! Shared Resource binding materializer for CI and direct CLI Release preparation.
//! No Docker, Jenkins API, image build, ingress publication or service restart.
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use serde_json::{Map, Value, json};

use super::{fail, redis::RedisRuntime, s3::S3Runtime, secrets::create_secret};
use crate::cli::CliError;

pub(crate) fn run(definition: &str, root: &str, release: &str) -> Result<(), CliError> {
    let plan = crate::build_plan::plan_value(Path::new(definition))
        .map_err(|e| fail(format!("invalid Instance Definition: {e}")))?;
    let root = fs::canonicalize(root).map_err(|_| fail("Instance Root is inaccessible"))?;
    let release =
        fs::canonicalize(release).map_err(|_| fail("Release directory is inaccessible"))?;
    let staged = read_json(&release.join("build-plan.json"))?;
    if staged != plan {
        return Err(fail(
            "staged Build Plan differs from the Instance Definition",
        ));
    }
    augment(&plan, &root, &release)?;
    println!("Prepared shared Resource bindings (no Docker or ingress action).");
    Ok(())
}

fn read_json(path: &Path) -> Result<Value, CliError> {
    serde_json::from_slice(&fs::read(path).map_err(|_| fail("cannot read staged Build Plan"))?)
        .map_err(|_| fail("invalid staged Build Plan JSON"))
}

fn yaml_json(path: &Path) -> Result<Value, CliError> {
    serde_yaml_ng::from_str(
        &fs::read_to_string(path).map_err(|_| fail("cannot read staged Compose file"))?,
    )
    .map_err(|_| fail("invalid staged Compose YAML"))
}

fn env_values(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|line| {
            if line.starts_with('#') {
                return None;
            }
            line.split_once('=').map(|(k, v)| {
                let v = v
                    .strip_prefix("'")
                    .and_then(|v| v.strip_suffix("'"))
                    .unwrap_or(v);
                (k.to_owned(), v.to_owned())
            })
        })
        .collect()
}

fn image(
    values: &BTreeMap<String, String>,
    name: &str,
    fallback: &str,
) -> Result<String, CliError> {
    let value = values
        .get(name)
        .or_else(|| values.get(fallback))
        .ok_or_else(|| {
            fail(format!(
                "missing prebuilt Resource image metadata '{fallback}'"
            ))
        })?;
    if value.is_empty() || value.contains(['\n', '\r', '\t', ' ', '$', '"', '\'']) {
        return Err(fail("invalid Resource image reference"));
    }
    Ok(value.clone())
}

pub(super) fn validate_requirements(
    module: &Value,
    manifest: &Value,
    resources: &[Value],
) -> Result<(), CliError> {
    let id = module["id"]
        .as_str()
        .ok_or_else(|| fail("Module ID missing"))?;
    let Some(requirements) = manifest["requires"]["capabilities"].as_object() else {
        return Ok(());
    };
    for (slot, requirement) in requirements {
        let provider = match requirement["id"].as_str() {
            Some("database.postgresql") => "postgresql",
            Some("storage.object") => "s3",
            Some("cache.redis") => "redis",
            _ => continue,
        };
        let target = module["bindings"][slot]
            .as_str()
            .ok_or_else(|| fail(format!("Module '{id}' required slot '{slot}' is UNBOUND")))?;
        if !resources
            .iter()
            .any(|r| r["id"] == target && r["provider"] == provider)
        {
            return Err(fail(format!(
                "Module '{id}' slot '{slot}' must target a {provider} Resource"
            )));
        }
    }
    Ok(())
}

pub(super) fn augment(plan: &Value, root: &Path, release: &Path) -> Result<(), CliError> {
    let resources = plan["resources"]
        .as_array()
        .ok_or_else(|| fail("missing plan Resources"))?;
    let modules = plan["modules"]
        .as_array()
        .ok_or_else(|| fail("missing plan Modules"))?;
    let mut selected = BTreeMap::new();
    for resource in resources {
        let provider = resource["provider"]
            .as_str()
            .ok_or_else(|| fail("Resource provider missing"))?;
        if !["postgresql", "s3", "redis"].contains(&provider) {
            return Err(fail(format!("unsupported Resource provider '{provider}'")));
        }
        if selected.insert(provider, resource).is_some() {
            return Err(fail(
                "v0 Release preparation supports one Resource of each provider type",
            ));
        }
    }
    let mut compose = yaml_json(&release.join("compose.yml"))?;
    let mut module_compose = yaml_json(&release.join("modules.compose.yml"))?;
    let env_text =
        fs::read_to_string(release.join("release.env")).map_err(|_| fail("missing release.env"))?;
    let mut values = if release.join("resolved-images.env").is_file() {
        env_values(
            &fs::read_to_string(release.join("resolved-images.env"))
                .map_err(|_| fail("cannot read image metadata"))?,
        )
    } else {
        BTreeMap::new()
    };
    values.extend(env_values(&env_text));
    let mut manifests = BTreeMap::new();

    // Validate the complete staged consumer graph BEFORE allocating secrets.
    for module in modules {
        let id = module["id"]
            .as_str()
            .ok_or_else(|| fail("Module ID missing"))?;
        if id.is_empty()
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
        {
            return Err(fail("unsafe Module ID in staged Release"));
        }
        let service = format!("module-{id}");
        if !module_compose["services"][&service].is_object() {
            return Err(fail(format!("missing staged Module service '{service}'")));
        }
        let manifest: Value = serde_json::from_slice(
            &fs::read(
                release
                    .join("modules")
                    .join(id)
                    .join("manafield.module.json"),
            )
            .map_err(|_| fail(format!("missing staged manifest for Module '{id}'")))?,
        )
        .map_err(|_| fail("invalid Module manifest JSON"))?;
        if manifest["id"] != id {
            return Err(fail("staged manifest ID does not match its Module"));
        }
        validate_requirements(module, &manifest, resources)?;
        manifests.insert(id, manifest);
    }
    let mut additions = BTreeMap::new();
    if let Some(resource) = selected.get("s3") {
        let id = resource["id"]
            .as_str()
            .ok_or_else(|| fail("S3 ID missing"))?;
        additions.insert(
            "MANAFIELD_S3_PROVIDER_IMAGE".to_owned(),
            image(
                &values,
                "MANAFIELD_S3_PROVIDER_IMAGE",
                &format!("resource.{id}.provider.image"),
            )?,
        );
    }
    if let Some(resource) = selected.get("redis") {
        let id = resource["id"]
            .as_str()
            .ok_or_else(|| fail("Redis ID missing"))?;
        for (key, suffix) in [
            ("MANAFIELD_REDIS_PROVIDER_IMAGE", "provider"),
            ("MANAFIELD_REDIS_SERVER_IMAGE", "server"),
        ] {
            additions.insert(
                key.to_owned(),
                image(&values, key, &format!("resource.{id}.{suffix}.image"))?,
            );
        }
    }
    let storage = selected
        .get("s3")
        .map(|r| S3Runtime::prepare(r, root))
        .transpose()?;
    let cache = selected
        .get("redis")
        .map(|r| RedisRuntime::prepare(r, root))
        .transpose()?;
    if let Some(storage) = &storage {
        for (key, value) in [
            ("MANAFIELD_S3_RESOURCE_ID", storage.id.clone()),
            (
                "MANAFIELD_S3_RESOURCE_NAME",
                "Manafield S3 Object Storage".into(),
            ),
            ("MANAFIELD_S3_ENDPOINT", storage.config.endpoint.clone()),
            ("MANAFIELD_S3_BUCKET", storage.config.bucket.clone()),
            ("MANAFIELD_S3_REGION", storage.config.region.clone()),
            (
                "MANAFIELD_S3_FORCE_PATH_STYLE",
                storage.config.force_path_style.to_string(),
            ),
            (
                "MANAFIELD_S3_ACCESS_KEY_ID_HOST",
                storage.access_path().to_string_lossy().into_owned(),
            ),
            (
                "MANAFIELD_S3_SECRET_ACCESS_KEY_HOST",
                storage.secret_path().to_string_lossy().into_owned(),
            ),
        ] {
            additions.insert(key.into(), value);
        }
    }
    if let Some(cache) = &cache {
        additions.insert("MANAFIELD_REDIS_RESOURCE_ID".into(), cache.id.clone());
        additions.insert(
            "MANAFIELD_REDIS_PASSWORD_HOST".into(),
            cache.password_file().to_string_lossy().into_owned(),
        );
        let project = compose["name"]
            .as_str()
            .ok_or_else(|| fail("staged Compose project name missing"))?;
        if project.is_empty()
            || !project
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
        {
            return Err(fail("unsafe Compose project name"));
        }
        compose["volumes"]["manafield-redis-data"] =
            json!({"name":format!("{project}-redis-{}", cache.id)});
    }

    let allocation_path = release.join("postgresql-allocations.tsv");
    let mut allocations = if allocation_path.is_file() {
        fs::read_to_string(&allocation_path)
            .map_err(|_| fail("cannot read PostgreSQL allocations"))?
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    for module in modules {
        let id = module["id"].as_str().unwrap();
        let manifest = &manifests[id];
        let service = &mut module_compose["services"][format!("module-{id}")];
        let mut injected = Map::new();
        let mut mounts = Vec::new();
        if let Some(bindings) = module["bindings"].as_object() {
            for (slot, target) in bindings {
                if let Some(storage) = storage.as_ref().filter(|r| target == &r.id) {
                    let path = release
                        .join("modules")
                        .join(id)
                        .join("manafield.module.json");
                    storage.inject(id, slot, &path, &mut injected, &mut mounts)?;
                } else if let Some(cache) = cache.as_ref().filter(|r| target == &r.id) {
                    cache.inject(id, slot, manifest, &mut injected, &mut mounts)?;
                    service["depends_on"]["manafield-redis"] =
                        json!({"condition":"service_healthy"});
                } else if let Some(pg) = selected.get("postgresql").filter(|r| target == &r["id"]) {
                    let resource = pg["id"]
                        .as_str()
                        .ok_or_else(|| fail("PostgreSQL ID missing"))?;
                    let schema =
                        format!("mf_{}_{}", id.replace('-', "_"), slot.to_ascii_lowercase());
                    if schema.len() > 55
                        || !schema
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
                    {
                        return Err(fail("unsafe PostgreSQL Module allocation"));
                    }
                    let password = root
                        .join("secrets/postgresql")
                        .join(resource)
                        .join(format!("{schema}.password"));
                    create_secret(&password)?;
                    let destination = format!("/run/manafield/bindings/{slot}/password");
                    for (suffix, value) in [
                        ("ENDPOINT_HOST", "manafield-postgres".to_owned()),
                        ("ENDPOINT_PORT", "5432".to_owned()),
                        ("CONFIG_DATABASE", "manafield".to_owned()),
                        ("CONFIG_SCHEMA", schema.clone()),
                        ("CONFIG_USERNAME", schema.clone()),
                        ("SECRET_PASSWORD_FILE", destination.clone()),
                    ] {
                        injected.insert(
                            format!("MANAFIELD_BINDING_{}_{}", slot.to_ascii_uppercase(), suffix),
                            json!(value),
                        );
                    }
                    mounts.push(json!({"type":"bind","source":password,"target":destination,"read_only":true}));
                    let row = format!(
                        "{resource}\tmanafield\t{schema}\t{schema}\t/run/manafield/postgresql/secrets/{resource}/{schema}.password"
                    );
                    if !allocations.contains(&row) {
                        allocations.push(row);
                    }
                } else {
                    continue;
                }
                injected.insert(
                    format!("MANAFIELD_BINDING_{}_TARGET", slot.to_ascii_uppercase()),
                    target.clone(),
                );
            }
        }
        if let Some(host) = module["exposure"]["host"].as_str() {
            injected.insert(
                "MANAFIELD_MODULE_PUBLIC_URL".into(),
                json!(format!("https://{host}")),
            );
        }
        if service.get("environment").is_none() {
            service["environment"] = json!({});
        }
        service["environment"]
            .as_object_mut()
            .ok_or_else(|| fail("Module environment must be a mapping"))?
            .extend(injected);
        if !mounts.is_empty() {
            if service.get("volumes").is_none() {
                service["volumes"] = json!([]);
            }
            let existing = service["volumes"]
                .as_array_mut()
                .ok_or_else(|| fail("Module volumes must be a list"))?;
            for mount in mounts {
                if let Some(previous) = existing.iter().find(|v| v["target"] == mount["target"]) {
                    if previous != &mount {
                        return Err(fail("conflicting Resource credential mount"));
                    }
                } else {
                    existing.push(mount);
                }
            }
        }
    }

    let mut profiles: Vec<String> = fs::read_to_string(release.join("compose-profiles.txt"))
        .map_err(|_| fail("missing staged Compose profiles"))?
        .trim()
        .split(',')
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
        .collect();
    for provider in ["postgresql", "s3", "redis"] {
        if selected.contains_key(provider) && !profiles.iter().any(|p| p == provider) {
            profiles.push(provider.into());
        }
    }
    // Preserve unrelated entries and values; replace only this materializer's keys.
    let mut env_lines = env_text
        .lines()
        .filter(|line| {
            !line
                .split_once('=')
                .is_some_and(|(key, _)| additions.contains_key(key))
        })
        .map(str::to_owned)
        .collect::<Vec<_>>();
    for (key, value) in additions {
        // Single-quoted dotenv values are literal (including '$') in Compose.
        if value.contains(['\n', '\r', '\'']) {
            return Err(fail("Resource metadata cannot be encoded in release.env"));
        }
        env_lines.push(format!("{key}='{value}'"));
    }
    let compose_yaml = serde_yaml_ng::to_string(&compose).map_err(|e| fail(e.to_string()))?;
    let modules_yaml =
        serde_yaml_ng::to_string(&module_compose).map_err(|e| fail(e.to_string()))?;
    // Receipt is written last. Apply refuses incomplete/mismatched Resource staging.
    let receipt = release.join("resource-materialization.json");
    if receipt.exists() {
        fs::remove_file(&receipt)
            .map_err(|_| fail("cannot invalidate previous Resource receipt"))?;
    }
    if selected.contains_key("postgresql") {
        replace(
            &allocation_path,
            format!("{}\n", allocations.join("\n")).as_bytes(),
            false,
        )?;
    }
    replace(&release.join("compose.yml"), compose_yaml.as_bytes(), false)?;
    replace(
        &release.join("modules.compose.yml"),
        modules_yaml.as_bytes(),
        false,
    )?;
    replace(
        &release.join("release.env"),
        format!("{}\n", env_lines.join("\n")).as_bytes(),
        true,
    )?;
    replace(
        &release.join("compose-profiles.txt"),
        format!("{}\n", profiles.join(",")).as_bytes(),
        false,
    )?;
    replace(
        &release.join("resource-materialization.json"),
        &serde_json::to_vec(plan).map_err(|e| fail(e.to_string()))?,
        false,
    )?;
    Ok(())
}

pub(crate) fn verify(plan: &Value, release: &Path) -> Result<(), CliError> {
    let required = plan["resources"].as_array().is_some_and(|resources| {
        resources.iter().any(|r| {
            matches!(r["provider"].as_str(), Some("s3" | "redis")) || r.get("exclusiveTo").is_some()
        })
    });
    if required && read_json(&release.join("resource-materialization.json"))? != *plan {
        return Err(fail(
            "Resource materialization is missing or differs from the Build Plan",
        ));
    }
    Ok(())
}

fn replace(path: &Path, bytes: &[u8], private: bool) -> Result<(), CliError> {
    let temporary = path.with_extension(format!("stage-{}", std::process::id()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .map_err(|_| fail("cannot stage Resource artifacts"))?;
    let result = (|| {
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| fail("cannot write Resource artifacts"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(if private {
                0o600
            } else {
                0o644
            }))
            .map_err(|_| fail("cannot set artifact permissions"))?;
        }
        fs::rename(&temporary, path).map_err(|_| fail("cannot publish Resource artifacts"))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests;
