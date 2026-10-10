//! Direct Docker deployment preparation from an immutable Instance Release.
//! Supports Git/dir Modules and the v0 PostgreSQL Resource Provider.
//! No cleanup or volume removal is performed here.

mod secrets;
use secrets::{create_secret, write_private};

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use serde_json::{Map, Value, json};

use super::CliError;

pub(super) struct PrepareOptions<'a> {
    pub instance_root: &'a Path,
    pub release: &'a Path,
    pub release_id: &'a str,
    pub source: &'a Path,
    pub modules_root: Option<&'a Path>,
}

fn fail(message: impl Into<String>) -> CliError {
    CliError::Execution(message.into())
}

pub(super) fn prepare(options: PrepareOptions<'_>) -> Result<PathBuf, CliError> {
    let source = fs::canonicalize(options.source)
        .map_err(|e| fail(format!("cannot locate Manafield source: {e}")))?;
    if !source.join("deploy/compose.yml").is_file() || !source.join("Dockerfile").is_file() {
        return Err(fail(format!(
            "'{}' is not a Manafield source checkout (Dockerfile and deploy/compose.yml required)",
            source.display()
        )));
    }
    let root = fs::canonicalize(options.instance_root)
        .map_err(|e| fail(format!("Instance root is inaccessible: {e}")))?;
    let plan = crate::build_plan::plan_value(options.release)
        .map_err(|e| fail(format!("cannot resolve Release: {e}")))?;
    let modules = plan["modules"]
        .as_array()
        .ok_or_else(|| fail("invalid Build Plan Modules"))?;
    let resources = plan["resources"]
        .as_array()
        .ok_or_else(|| fail("invalid Build Plan Resources"))?;
    // Exposed services require the dedicated Traefik ingress renderer and its
    // publication step, which direct-mode deliberately does not silently skip.
    if modules.iter().any(|m| m.get("exposure").is_some()) {
        return Err(fail(
            "direct deploy of exposed Modules is not supported yet; an Ingress provider stage is required",
        ));
    }
    if !plan["deployment"]["ingress"].is_null() {
        return Err(fail(
            "direct deploy does not yet publish ingress; omit deployment.ingress or use Jenkins",
        ));
    }
    if !plan["runtimeProviders"]
        .as_array()
        .is_some_and(|a| a.iter().any(|p| p["id"] == "docker"))
    {
        return Err(fail(
            "direct deploy requires enabled Docker Runtime Provider",
        ));
    }
    if resources.len() > 1 || resources.iter().any(|r| r["provider"] != "postgresql") {
        return Err(fail(
            "direct deploy v0 supports at most one PostgreSQL Resource",
        ));
    }
    let postgres_id = resources.first().map(|v| field(v, "id")).transpose()?;
    let network = field(&plan["deployment"], "modulesNetwork")?;
    let edge = field(&plan["deployment"], "edgeNetwork")?;
    let project = compose_project_name(field(&plan, "instanceId")?)?;
    // Docker Compose project name and managed internal network are stable per
    // Instance, not per Release. Neither collides with the old Jenkins stack.
    let scoped_network = format!(
        "{network}-{}",
        field(&plan, "instanceId")?.to_ascii_lowercase()
    );
    if !scoped_network
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(fail(format!("unsafe network name '{scoped_network}'")));
    }

    let staging = root.join("manafield/staged").join(options.release_id);
    if staging.exists() {
        super::release::validate_staged(options.release, &staging)?;
        println!("Reusing prepared Release artifacts: {}", staging.display());
        return Ok(staging);
    }
    fs::create_dir_all(&staging)
        .map_err(|e| fail(format!("cannot create staging directory: {e}")))?;
    // Incomplete staging is removed; database secrets and live resources remain.
    match prepare_inside(
        &root,
        &source,
        &staging,
        &plan,
        modules,
        postgres_id,
        &scoped_network,
        edge,
        &project,
        options.modules_root,
    ) {
        Ok(()) => {
            println!("Prepared Release artifacts: {}", staging.display());
            Ok(staging)
        }
        Err(e) => {
            let _ = fs::remove_dir_all(&staging);
            Err(e)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn prepare_inside(
    root: &Path,
    source: &Path,
    stage: &Path,
    plan: &Value,
    modules: &[Value],
    postgres_id: Option<&str>,
    modules_network: &str,
    edge_network: &str,
    project: &str,
    modules_root: Option<&Path>,
) -> Result<(), CliError> {
    // Core and Resource Provider are built from the selected local Manafield
    // checkout; external Modules are built independently.
    let core_revision = output("git", &["rev-parse", "--short=12", "HEAD"], source)?;
    let core_image = format!("manafield-core:{core_revision}");
    run(
        "docker",
        &[
            "build",
            "--target",
            "core-runtime",
            "--tag",
            &core_image,
            ".",
        ],
        source,
    )?;
    let provider_image = if postgres_id.is_some() {
        let name = format!("manafield-resource-postgresql:{core_revision}");
        run(
            "docker",
            &[
                "build",
                "--file",
                "providers/postgresql/Dockerfile",
                "--tag",
                &name,
                "providers/postgresql",
            ],
            source,
        )?;
        name
    } else {
        String::new()
    };

    let mut compose_modules = Map::new();
    let modules_dir = stage.join("modules");
    let module_sources_dir = stage.join("sources");
    fs::create_dir_all(&modules_dir).map_err(|e| fail(e.to_string()))?;
    fs::create_dir_all(&module_sources_dir).map_err(|e| fail(e.to_string()))?;
    let mut allocations = Vec::<String>::new();
    let mut used_schema = BTreeSet::new();

    for module in modules {
        let id = field(module, "id")?;
        if !valid_part(id) {
            return Err(fail(format!("unsafe Module Instance ID '{id}'")));
        }
        let source_config = &module["source"];
        let source_dir = match field(source_config, "type")? {
            "git" => {
                let repo = field(source_config, "repository")?;
                let reference = field(source_config, "ref")?;
                let checkout = module_sources_dir.join(id);
                run(
                    "git",
                    &[
                        "clone",
                        "--no-checkout",
                        repo,
                        checkout
                            .to_str()
                            .ok_or_else(|| fail("non-UTF-8 checkout path"))?,
                    ],
                    stage,
                )?;
                run(
                    "git",
                    &["fetch", "--depth=1", "origin", reference],
                    &checkout,
                )?;
                run("git", &["checkout", "--detach", "FETCH_HEAD"], &checkout)?;
                let subdir = source_config["subdir"].as_str().unwrap_or(".");
                let module_source = checkout.join(subdir);
                if !module_source.is_dir() {
                    return Err(fail(format!("Module '{id}' source subdir missing")));
                }
                module_source
            }
            "dir" => {
                let location = modules_root
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| root.join("modules"));
                let found = location.join(id);
                if !found.is_dir() {
                    return Err(fail(format!(
                        "local Module '{}' not found in '{}'; supply --modules-root DIR",
                        id,
                        location.display()
                    )));
                }
                found
            }
            other => return Err(fail(format!("unsupported Module source '{other}'"))),
        };
        let manifest = source_dir.join("manafield.module.json");
        if !manifest.is_file() {
            return Err(fail(format!(
                "Module '{id}' is missing manafield.module.json"
            )));
        }
        let output_dir = modules_dir.join(id);
        fs::create_dir_all(&output_dir).map_err(|e| fail(e.to_string()))?;
        fs::copy(&manifest, output_dir.join("manafield.module.json"))
            .map_err(|e| fail(e.to_string()))?;
        let context = module_source_dir(&source_dir, field(&module["build"], "context")?)?;
        let dockerfile = field(&module["build"], "dockerfile")?;
        if dockerfile.starts_with('/') || dockerfile.split('/').any(|p| p == "..") {
            return Err(fail(format!("unsafe Dockerfile path '{dockerfile}'")));
        }
        if field(&module["build"], "type")? != "docker" {
            return Err(fail("only Docker Module builds supported"));
        }
        let digest = if source_config["type"] == "git" {
            output("git", &["rev-parse", "--short=12", "HEAD"], &source_dir)?
        } else {
            "local".to_owned()
        };
        let image = format!("manafield-module-{}:{digest}", id.to_ascii_lowercase());
        run(
            "docker",
            &["build", "--file", dockerfile, "--tag", &image, "."],
            &context,
        )?;

        let mut env = Map::new();
        env.insert("PORT".into(), json!("8080"));
        env.insert("MANAFIELD_CORE_URL".into(), json!("http://core:8080"));
        env.insert("MANAFIELD_MODULE_ID".into(), json!(id));
        env.insert("MANAFIELD_WEB_BASE_PATH".into(), json!("/"));
        let mut mounts = Vec::new();
        if let Some(bindings) = module.get("bindings").and_then(Value::as_object) {
            for (slot, target) in bindings {
                if !valid_env_slot(slot) {
                    return Err(fail(format!("unsafe Requirement slot '{slot}'")));
                }
                let instance = target
                    .as_str()
                    .ok_or_else(|| fail("Binding target must be string"))?;
                let key = slot.to_ascii_uppercase();
                env.insert(format!("MANAFIELD_BINDING_{key}_TARGET"), json!(instance));
                if postgres_id == Some(instance) {
                    let schema = format!(
                        "mf_{}_{}",
                        id.replace(['-', '.'], "_"),
                        slot.to_ascii_lowercase()
                    );
                    if schema.len() > 55
                        || !schema
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
                    {
                        return Err(fail(format!(
                            "PostgreSQL schema name for {id}.{slot} is unsafe or too long"
                        )));
                    }
                    if !used_schema.insert(schema.clone()) {
                        return Err(fail(format!("PostgreSQL schema collision '{schema}'")));
                    }
                    let secret_dir = root.join("secrets/postgresql").join(instance);
                    let secret_path = secret_dir.join(format!("{schema}.password"));
                    create_secret(&secret_path)?;
                    let container_path = format!("/run/manafield/bindings/{slot}/password");
                    let provider_path =
                        format!("/run/manafield/postgresql/secrets/{instance}/{schema}.password");
                    allocations.push(format!(
                        "{instance}\tmanafield\t{schema}\t{schema}\t{provider_path}"
                    ));
                    env.insert(
                        format!("MANAFIELD_BINDING_{key}_ENDPOINT_HOST"),
                        json!("manafield-postgres"),
                    );
                    env.insert(
                        format!("MANAFIELD_BINDING_{key}_ENDPOINT_PORT"),
                        json!("5432"),
                    );
                    env.insert(
                        format!("MANAFIELD_BINDING_{key}_CONFIG_DATABASE"),
                        json!("manafield"),
                    );
                    env.insert(
                        format!("MANAFIELD_BINDING_{key}_CONFIG_SCHEMA"),
                        json!(schema),
                    );
                    env.insert(
                        format!("MANAFIELD_BINDING_{key}_CONFIG_USERNAME"),
                        json!(schema),
                    );
                    env.insert(
                        format!("MANAFIELD_BINDING_{key}_SECRET_PASSWORD_FILE"),
                        json!(container_path),
                    );
                    mounts.push(json!({"type":"bind","source":secret_path,"target":container_path,"read_only":true}));
                }
            }
        }
        let mut service = json!({
            "image":image,"restart":"unless-stopped","init":true,
            "environment":env,
            "depends_on":{"core":{"condition":"service_healthy"}},
            "read_only":true,
            "tmpfs":["/tmp"],
            "cap_drop":["ALL"],
            "security_opt":["no-new-privileges:true"],
            "networks":{"modules":{}}
        });
        if !mounts.is_empty() {
            service["volumes"] = json!(mounts);
        }
        compose_modules.insert(format!("module-{id}"), service);
    }

    let mut compose: serde_yaml_ng::Value = serde_yaml_ng::from_str(
        &fs::read_to_string(source.join("deploy/compose.yml")).map_err(|e| fail(e.to_string()))?,
    )
    .map_err(|e| fail(format!("invalid Core Compose template: {e}")))?;
    // Avoid switching/removing the Jenkins project's active containers or
    // named PostgreSQL volume. The Compose project name is stable across
    // Releases of the same Instance but distinct across Instance IDs.
    compose["name"] = serde_yaml_ng::Value::String(project.to_owned());
    // Compose providers and modules rely on Core health; ensure it exists.
    compose["services"]["core"]["healthcheck"] = serde_yaml_ng::to_value(json!({
        "test":["CMD","curl","--fail","--silent","http://127.0.0.1:8080/health"],
        "interval":"5s","timeout":"3s","retries":15,"start_period":"5s"
    }))
    .map_err(|e| fail(e.to_string()))?;
    // Unused external edge network shouldn't be required for non-exposed Modules.
    compose["networks"]
        .as_mapping_mut()
        .ok_or_else(|| fail("Compose network map missing"))?
        .remove(serde_yaml_ng::Value::String("edge".into()));
    let compose_yaml = serde_yaml_ng::to_string(&compose).map_err(|e| fail(e.to_string()))?;
    fs::write(stage.join("compose.yml"), compose_yaml).map_err(|e| fail(e.to_string()))?;
    let module_yaml = serde_yaml_ng::to_string(&json!({"services":compose_modules}))
        .map_err(|e| fail(e.to_string()))?;
    fs::write(stage.join("modules.compose.yml"), module_yaml).map_err(|e| fail(e.to_string()))?;
    fs::write(
        stage.join("build-plan.json"),
        serde_json::to_vec_pretty(plan).map_err(|e| fail(e.to_string()))?,
    )
    .map_err(|e| fail(e.to_string()))?;
    let allocation_path = stage.join("postgresql-allocations.tsv");
    fs::write(
        &allocation_path,
        if allocations.is_empty() {
            String::new()
        } else {
            format!("{}\n", allocations.join("\n"))
        },
    )
    .map_err(|e| fail(e.to_string()))?;
    fs::write(
        stage.join("compose-profiles.txt"),
        if postgres_id.is_some() {
            "postgresql\n"
        } else {
            ""
        },
    )
    .map_err(|e| fail(e.to_string()))?;
    let core_port = core_host_port(project)?;
    let mut vars = vec![
        format!("MANAFIELD_CORE_PORT={core_port}"),
        format!("MANAFIELD_CORE_IMAGE={core_image}"),
        format!("MANAFIELD_MODULES_PATH={}", modules_dir.display()),
        format!("MANAFIELD_MODULES_NETWORK={modules_network}"),
        format!("MANAFIELD_EDGE_NETWORK={edge_network}"),
        format!("MANAFIELD_POSTGRES_PROVIDER_IMAGE={provider_image}"),
        format!(
            "MANAFIELD_POSTGRES_RESOURCE_ID={}",
            postgres_id.unwrap_or("")
        ),
        "MANAFIELD_POSTGRES_RESOURCE_NAME=Manafield PostgreSQL".to_owned(),
        "MANAFIELD_POSTGRES_DB=manafield".to_owned(),
        format!(
            "MANAFIELD_POSTGRES_ALLOCATIONS_FILE_HOST={}",
            allocation_path.display()
        ),
        format!(
            "MANAFIELD_POSTGRES_SECRETS_PATH={}",
            root.join("secrets/postgresql").display()
        ),
    ];
    if postgres_id.is_some() {
        let password_path = root.join("secrets/postgresql/admin.password");
        create_secret(&password_path)?;
        let password = fs::read_to_string(&password_path).map_err(|e| fail(e.to_string()))?;
        vars.push(format!("MANAFIELD_POSTGRES_PASSWORD={}", password.trim()));
    }
    // Environment files can contain credentials. Keep private.
    write_private(
        &stage.join("release.env"),
        format!("{}\n", vars.join("\n")).as_bytes(),
    )?;
    Ok(())
}

fn field<'a>(value: &'a Value, key: &str) -> Result<&'a str, CliError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| fail(format!("missing {key} in Build Plan")))
}
fn core_host_port(project: &str) -> Result<u16, CliError> {
    if let Ok(configured) = std::env::var("MANAFIELD_CORE_PORT") {
        let value = configured
            .parse::<u16>()
            .map_err(|e| fail(format!("MANAFIELD_CORE_PORT must be a valid TCP port: {e}")))?;
        if value < 1024 {
            return Err(fail("MANAFIELD_CORE_PORT must be at least 1024"));
        }
        return Ok(value);
    }
    // Stable per Instance, distinct from the Jenkins default of 18080.
    let hash = project.bytes().fold(2166136261u32, |acc, b| {
        (acc ^ u32::from(b)).wrapping_mul(16777619)
    });
    Ok((20_000 + hash % 20_000) as u16)
}

fn compose_project_name(id: &str) -> Result<String, CliError> {
    if id.is_empty()
        || id.len() > 40
        || !id.bytes().next().is_some_and(|b| b.is_ascii_alphanumeric())
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(fail(format!(
            "unsupported Instance ID '{id}' for Docker Compose project"
        )));
    }
    Ok(format!("manafield-{}", id.to_ascii_lowercase()))
}

fn valid_part(s: &str) -> bool {
    !s.is_empty()
        && s != "."
        && s != ".."
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
}
fn valid_env_slot(s: &str) -> bool {
    let mut chars = s.bytes();
    matches!(chars.next(), Some(b'A'..=b'Z' | b'a'..=b'z' | b'_'))
        && chars.all(|c| c.is_ascii_alphanumeric() || c == b'_')
}
fn module_source_dir(base: &Path, relative: &str) -> Result<PathBuf, CliError> {
    if relative.starts_with('/') || relative.split('/').any(|p| p == ".." || p.is_empty()) {
        return Err(fail(format!("unsafe Module build context '{relative}'")));
    }
    let path = base.join(relative);
    if !path.is_dir() {
        return Err(fail(format!(
            "Module build context '{}' missing",
            path.display()
        )));
    }
    Ok(path)
}
fn run(program: &str, args: &[&str], dir: &Path) -> Result<(), CliError> {
    let status = ProcessCommand::new(program)
        .current_dir(dir)
        .args(args)
        .status()
        .map_err(|e| fail(format!("cannot run {program} {}: {e}", args.join(" "))))?;
    if !status.success() {
        return Err(fail(format!(
            "{program} {} failed with {status}",
            args.join(" ")
        )));
    }
    Ok(())
}
fn output(program: &str, args: &[&str], dir: &Path) -> Result<String, CliError> {
    let out = ProcessCommand::new(program)
        .current_dir(dir)
        .args(args)
        .output()
        .map_err(|e| fail(format!("cannot run {program}: {e}")))?;
    if !out.status.success() {
        return Err(fail(format!(
            "{program} {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn safe_identifiers_and_context() {
        assert!(valid_part("echo-app"));
        assert!(!valid_part("../../etc"));
        assert!(!valid_part(".."));
        assert!(valid_env_slot("main_state"));
        assert!(!valid_env_slot("a-b"));
        assert_eq!(
            compose_project_name("manafield-bootstrap").unwrap(),
            "manafield-manafield-bootstrap"
        );
        assert!(compose_project_name("../unsafe").is_err());
        let port = core_host_port("manafield-demo").unwrap();
        assert!((20_000..40_000).contains(&port));
        assert_ne!(core_host_port("manafield-demo").unwrap(), 18080);
        assert_ne!(
            core_host_port("manafield-demo").unwrap(),
            core_host_port("manafield-other").unwrap()
        );
        assert!(module_source_dir(Path::new("/tmp"), "../etc").is_err());
    }
}
