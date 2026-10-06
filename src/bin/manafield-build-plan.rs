use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

fn main() {
    if let Err(error) = run() {
        eprintln!("manafield-build-plan: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let input = args
        .next()
        .ok_or("usage: manafield-build-plan <instance.yaml> [output.json]")?;
    let output = args.next();

    if args.next().is_some() {
        return Err("too many arguments".into());
    }

    let definition = read_definition(Path::new(&input))?;
    validate_definition(&definition)?;

    let plan = BuildPlan::from(definition);
    let json = serde_json::to_string_pretty(&plan)?;

    if let Some(output) = output {
        fs::write(output, format!("{json}\n"))?;
    } else {
        println!("{json}");
    }

    Ok(())
}

fn read_definition(path: &Path) -> Result<InstanceDefinition, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(path)?;
    Ok(serde_yaml_ng::from_str(&contents)?)
}

fn validate_definition(definition: &InstanceDefinition) -> Result<(), String> {
    if definition.version != 0 {
        return Err(format!(
            "unsupported instance definition version {}",
            definition.version
        ));
    }

    require_non_empty("instance.id", &definition.instance.id)?;
    validate_source("core.source", &definition.core.source)?;

    let mut runtime_ids = HashSet::new();
    for runtime in &definition.runtime_providers {
        require_non_empty("runtimeProviders[].id", &runtime.id)?;

        if !runtime_ids.insert(runtime.id.as_str()) {
            return Err(format!("duplicate runtime provider id '{}'", runtime.id));
        }
    }

    let mut module_ids = HashSet::new();
    for module in &definition.modules {
        require_non_empty("modules[].id", &module.id)?;

        if !module_ids.insert(module.id.as_str()) {
            return Err(format!("duplicate module id '{}'", module.id));
        }

        validate_source(&format!("modules[{}].source", module.id), &module.source)?;

        require_non_empty(
            &format!("modules[{}].build.context", module.id),
            &module.build.context,
        )?;

        require_non_empty(
            &format!("modules[{}].build.dockerfile", module.id),
            &module.build.dockerfile,
        )?;
    }

    require_non_empty("deployment.network", &definition.deployment.network)?;

    Ok(())
}

fn validate_source(label: &str, source: &SourceDefinition) -> Result<(), String> {
    require_non_empty(&format!("{label}.repository"), &source.repository)?;
    require_non_empty(&format!("{label}.ref"), &source.git_ref)?;
    Ok(())
}

fn require_non_empty(label: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{label} must not be empty"))
    } else {
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceDefinition {
    version: u32,
    instance: InstanceMetadata,
    core: CoreDefinition,
    #[serde(default)]
    runtime_providers: Vec<RuntimeProviderDefinition>,
    #[serde(default)]
    modules: Vec<ModuleDefinition>,
    deployment: DeploymentDefinition,
}

#[derive(Debug, Deserialize)]
struct InstanceMetadata {
    id: String,
}

#[derive(Debug, Deserialize)]
struct CoreDefinition {
    source: SourceDefinition,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct SourceDefinition {
    repository: String,
    #[serde(rename = "ref")]
    git_ref: String,
}

#[derive(Debug, Deserialize)]
struct RuntimeProviderDefinition {
    id: String,
    #[serde(default = "default_true")]
    enabled: bool,
}

#[derive(Debug, Deserialize)]
struct ModuleDefinition {
    id: String,
    #[serde(default = "default_true")]
    enabled: bool,
    source: SourceDefinition,
    build: ModuleBuildDefinition,
}

#[derive(Debug, Deserialize)]
struct ModuleBuildDefinition {
    #[serde(rename = "type")]
    build_type: BuildType,
    context: String,
    dockerfile: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum BuildType {
    Docker,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DeploymentDefinition {
    network: String,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BuildPlan {
    version: u32,
    instance_id: String,
    core: CorePlan,
    runtime_providers: Vec<RuntimeProviderPlan>,
    modules: Vec<ModulePlan>,
    deployment: DeploymentDefinition,
}

impl From<InstanceDefinition> for BuildPlan {
    fn from(definition: InstanceDefinition) -> Self {
        Self {
            version: definition.version,
            instance_id: definition.instance.id,
            core: CorePlan {
                source: definition.core.source,
            },
            runtime_providers: definition
                .runtime_providers
                .into_iter()
                .filter(|runtime| runtime.enabled)
                .map(|runtime| RuntimeProviderPlan { id: runtime.id })
                .collect(),
            modules: definition
                .modules
                .into_iter()
                .filter(|module| module.enabled)
                .map(|module| ModulePlan {
                    id: module.id,
                    source: module.source,
                    build: ModuleBuildPlan {
                        build_type: module.build.build_type,
                        context: module.build.context,
                        dockerfile: module.build.dockerfile,
                    },
                })
                .collect(),
            deployment: definition.deployment,
        }
    }
}

#[derive(Debug, Serialize)]
struct CorePlan {
    source: SourceDefinition,
}

#[derive(Debug, Serialize)]
struct RuntimeProviderPlan {
    id: String,
}

#[derive(Debug, Serialize)]
struct ModulePlan {
    id: String,
    source: SourceDefinition,
    build: ModuleBuildPlan,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModuleBuildPlan {
    #[serde(rename = "type")]
    build_type: BuildType,
    context: String,
    dockerfile: String,
}
