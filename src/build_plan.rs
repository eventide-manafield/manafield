mod ci_output;
mod validation;

use ci_output::write_ci_plan;
use validation::{read_definition, validate_definition};

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub fn resolve(
    input: &Path,
    output: Option<&Path>,
    ci_output_dir: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let definition = read_definition(input)?;
    validate_definition(&definition)?;

    let plan = BuildPlan::from(definition);

    if let Some(ci_output_dir) = ci_output_dir {
        write_ci_plan(&plan, ci_output_dir)?;
    }

    let json = serde_json::to_string_pretty(&plan)?;

    if let Some(output) = output {
        fs::write(output, format!("{json}\n"))?;
    } else {
        println!("{json}");
    }

    Ok(())
}

/// Validate the complete v0 Instance Definition without writing build artifacts.
pub fn validate_yaml(contents: &str) -> Result<(), Box<dyn std::error::Error>> {
    let definition: InstanceDefinition = serde_yaml_ng::from_str(contents)?;
    validate_definition(&definition)?;
    Ok(())
}

/// Return the fully resolved (executor-neutral) Build Plan as JSON for safe
/// equality checks before applying pre-staged Compose files.
pub fn plan_value(input: &Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let definition = read_definition(input)?;
    validate_definition(&definition)?;
    Ok(serde_json::to_value(BuildPlan::from(definition))?)
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
    #[serde(default)]
    resources: Vec<ResourceDefinition>,
    deployment: DeploymentDefinition,
}

#[derive(Debug, Deserialize)]
struct InstanceMetadata {
    id: String,
}

#[derive(Debug, Deserialize)]
struct CoreDefinition {
    source: SourceDefinition,
    #[serde(default)]
    bindings: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct SourceDefinition {
    repository: String,
    #[serde(rename = "ref")]
    git_ref: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum ModuleSourceDefinition {
    Git {
        repository: String,
        #[serde(rename = "ref")]
        git_ref: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subdir: Option<String>,
    },
    Dir,
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
    source: ModuleSourceDefinition,
    build: ModuleBuildDefinition,
    #[serde(default)]
    exposure: Option<ExposureDefinition>,
    #[serde(default)]
    bindings: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct ResourceDefinition {
    id: String,
    #[serde(default = "default_true")]
    enabled: bool,
    provider: String,
}

#[derive(Debug, Deserialize)]
struct ModuleBuildDefinition {
    #[serde(rename = "type")]
    build_type: BuildType,
    context: String,
    dockerfile: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum ExposureDefinition {
    Host {
        host: String,
        #[serde(rename = "targetPort")]
        target_port: u16,
    },
    Prefix {
        host: String,
        prefix: String,
        #[serde(rename = "targetPort")]
        target_port: u16,
    },
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum BuildType {
    Docker,
}

impl BuildType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Docker => "docker",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeploymentDefinition {
    modules_network: String,
    edge_network: String,
    #[serde(default)]
    ingress: Option<IngressDefinition>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct IngressDefinition {
    provider: String,
    output: String,
    #[serde(default)]
    bootstrap: Option<IngressBootstrapDefinition>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct IngressBootstrapDefinition {
    compose_file: String,
    service: String,
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
    resources: Vec<ResourcePlan>,
    deployment: DeploymentDefinition,
}

impl From<InstanceDefinition> for BuildPlan {
    fn from(definition: InstanceDefinition) -> Self {
        Self {
            version: definition.version,
            instance_id: definition.instance.id,
            core: CorePlan {
                source: definition.core.source,
                bindings: definition.core.bindings,
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
                    exposure: module.exposure,
                    bindings: module.bindings,
                })
                .collect(),
            resources: definition
                .resources
                .into_iter()
                .filter(|resource| resource.enabled)
                .map(|resource| ResourcePlan {
                    id: resource.id,
                    provider: resource.provider,
                })
                .collect(),
            deployment: definition.deployment,
        }
    }
}

#[derive(Debug, Serialize)]
struct CorePlan {
    source: SourceDefinition,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    bindings: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
struct RuntimeProviderPlan {
    id: String,
}

#[derive(Debug, Serialize)]
struct ModulePlan {
    id: String,
    source: ModuleSourceDefinition,
    build: ModuleBuildPlan,
    #[serde(skip_serializing_if = "Option::is_none")]
    exposure: Option<ExposureDefinition>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    bindings: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
struct ResourcePlan {
    id: String,
    provider: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModuleBuildPlan {
    #[serde(rename = "type")]
    build_type: BuildType,
    context: String,
    dockerfile: String,
}

#[cfg(test)]
mod tests;
