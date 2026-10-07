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
        .ok_or("usage: manafield-build-plan <instance.yaml> [output.json] [ci-output-dir]")?;
    let output = args.next();
    let ci_output_dir = args.next();

    if args.next().is_some() {
        return Err(
            "usage: manafield-build-plan <instance.yaml> [output.json] [ci-output-dir]".into(),
        );
    }

    let definition = read_definition(Path::new(&input))?;
    validate_definition(&definition)?;

    let plan = BuildPlan::from(definition);

    if let Some(ci_output_dir) = ci_output_dir {
        write_ci_plan(&plan, Path::new(&ci_output_dir))?;
    }

    let json = serde_json::to_string_pretty(&plan)?;

    if let Some(output) = output {
        fs::write(output, format!("{json}\n"))?;
    } else {
        println!("{json}");
    }

    Ok(())
}

fn write_ci_plan(plan: &BuildPlan, directory: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(directory)?;

    fs::write(
        directory.join("instance-id.txt"),
        format!("{}\n", plan.instance_id),
    )?;
    fs::write(
        directory.join("modules-network.txt"),
        format!("{}\n", plan.deployment.modules_network),
    )?;
    fs::write(
        directory.join("edge-network.txt"),
        format!("{}\n", plan.deployment.edge_network),
    )?;

    if let Some(ingress) = &plan.deployment.ingress {
        fs::write(
            directory.join("ingress-provider.txt"),
            format!("{}\n", ingress.provider),
        )?;
        fs::write(
            directory.join("ingress-output.txt"),
            format!("{}\n", ingress.output),
        )?;

        if let Some(bootstrap) = &ingress.bootstrap {
            fs::write(
                directory.join("ingress-bootstrap-compose.txt"),
                format!("{}\n", bootstrap.compose_file),
            )?;
            fs::write(
                directory.join("ingress-bootstrap-service.txt"),
                format!("{}\n", bootstrap.service),
            )?;
        }
    }

    let runtime_providers = plan
        .runtime_providers
        .iter()
        .map(|provider| provider.id.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        directory.join("runtime-providers.txt"),
        if runtime_providers.is_empty() {
            String::new()
        } else {
            format!("{runtime_providers}\n")
        },
    )?;

    let mut modules = String::new();
    for module in &plan.modules {
        let (source_type, source_value, source_ref) = match &module.source {
            ModuleSourceDefinition::Git {
                repository,
                git_ref,
            } => ("git", repository.as_str(), git_ref.as_str()),
            ModuleSourceDefinition::Dir => ("dir", "-", "-"),
        };

        modules.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            module.id,
            source_type,
            source_value,
            source_ref,
            module.build.build_type.as_str(),
            module.build.context,
            module.build.dockerfile,
        ));
    }
    fs::write(directory.join("modules.tsv"), modules)?;

    let mut resources = String::new();
    for resource in &plan.resources {
        resources.push_str(&format!("{}\t{}\n", resource.id, resource.provider,));
    }
    fs::write(directory.join("resources.tsv"), resources)?;

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

    let mut instance_ids = HashSet::new();
    let mut exposure_hosts = HashSet::new();
    let mut has_exposure = false;

    for module in &definition.modules {
        require_non_empty("modules[].id", &module.id)?;

        if !instance_ids.insert(module.id.as_str()) {
            return Err(format!("duplicate instance id '{}'", module.id));
        }

        validate_module_source(&format!("modules[{}].source", module.id), &module.source)?;

        require_non_empty(
            &format!("modules[{}].build.context", module.id),
            &module.build.context,
        )?;

        require_non_empty(
            &format!("modules[{}].build.dockerfile", module.id),
            &module.build.dockerfile,
        )?;

        if module.enabled
            && let Some(exposure) = &module.exposure
        {
            has_exposure = true;

            match exposure {
                ExposureDefinition::Host { host, .. } => {
                    require_non_empty(&format!("modules[{}].exposure.host", module.id), host)?;

                    if !exposure_hosts.insert(host.as_str()) {
                        return Err(format!(
                            "duplicate host exposure '{}' in enabled Modules",
                            host
                        ));
                    }
                }
            }
        }
    }

    for resource in &definition.resources {
        require_non_empty("resources[].id", &resource.id)?;
        require_non_empty(
            &format!("resources[{}].provider", resource.id),
            &resource.provider,
        )?;

        if !instance_ids.insert(resource.id.as_str()) {
            return Err(format!("duplicate instance id '{}'", resource.id));
        }
    }

    require_non_empty(
        "deployment.modulesNetwork",
        &definition.deployment.modules_network,
    )?;
    require_non_empty(
        "deployment.edgeNetwork",
        &definition.deployment.edge_network,
    )?;

    if let Some(ingress) = &definition.deployment.ingress {
        require_non_empty("deployment.ingress.provider", &ingress.provider)?;
        require_non_empty("deployment.ingress.output", &ingress.output)?;

        if let Some(bootstrap) = &ingress.bootstrap {
            require_non_empty(
                "deployment.ingress.bootstrap.composeFile",
                &bootstrap.compose_file,
            )?;
            require_non_empty("deployment.ingress.bootstrap.service", &bootstrap.service)?;
        }
    } else if has_exposure {
        return Err("enabled Module exposure requires deployment.ingress".into());
    }

    Ok(())
}

fn validate_source(label: &str, source: &SourceDefinition) -> Result<(), String> {
    require_non_empty(&format!("{label}.repository"), &source.repository)?;
    require_non_empty(&format!("{label}.ref"), &source.git_ref)?;
    Ok(())
}

fn validate_module_source(label: &str, source: &ModuleSourceDefinition) -> Result<(), String> {
    match source {
        ModuleSourceDefinition::Git {
            repository,
            git_ref,
        } => {
            require_non_empty(&format!("{label}.repository"), repository)?;
            require_non_empty(&format!("{label}.ref"), git_ref)?;
        }
        ModuleSourceDefinition::Dir => {}
    }

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
