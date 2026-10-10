//! Validate the input Instance Definition before any plan artifact is written.
use super::*;
use std::collections::HashSet;

pub(super) fn read_definition(
    path: &Path,
) -> Result<InstanceDefinition, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(path)?;
    Ok(serde_yaml_ng::from_str(&contents)?)
}

pub(super) fn validate_definition(definition: &InstanceDefinition) -> Result<(), String> {
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
    let mut enabled_instance_ids = HashSet::new();
    let mut host_exposures = HashSet::new();
    let mut prefix_exposures = HashSet::new();
    let mut has_exposure = false;

    for module in &definition.modules {
        require_non_empty("modules[].id", &module.id)?;

        if !instance_ids.insert(module.id.as_str()) {
            return Err(format!("duplicate instance id '{}'", module.id));
        }

        if module.enabled {
            enabled_instance_ids.insert(module.id.as_str());
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
                ExposureDefinition::Host { host, target_port } => {
                    require_non_empty(&format!("modules[{}].exposure.host", module.id), host)?;
                    validate_target_port(&module.id, *target_port)?;

                    if !host_exposures.insert(host.as_str()) {
                        return Err(format!(
                            "duplicate host exposure '{}' in enabled Modules",
                            host
                        ));
                    }
                }
                ExposureDefinition::Prefix {
                    host,
                    prefix,
                    target_port,
                } => {
                    require_non_empty(&format!("modules[{}].exposure.host", module.id), host)?;
                    validate_prefix(&module.id, prefix)?;
                    validate_target_port(&module.id, *target_port)?;

                    if !prefix_exposures.insert((host.as_str(), prefix.as_str())) {
                        return Err(format!(
                            "duplicate prefix exposure '{}{}' in enabled Modules",
                            host, prefix
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

        if resource.enabled {
            enabled_instance_ids.insert(resource.id.as_str());
        }
    }

    // v0 Core logger accepts only a PostgreSQL Resource for its optional
    // loggingState slot. Other Core settings remain independent of Modules.
    for (slot, target) in &definition.core.bindings {
        if slot != "loggingState" {
            return Err(format!("unknown Core binding slot '{slot}'"));
        }
        if !definition
            .resources
            .iter()
            .any(|r| r.enabled && r.id == *target && r.provider == "postgresql")
        {
            return Err(format!(
                "core.bindings.loggingState must target an enabled PostgreSQL Resource, got '{target}'"
            ));
        }
    }

    for module in definition.modules.iter().filter(|module| module.enabled) {
        for (slot, target) in &module.bindings {
            require_non_empty(&format!("modules[{}].bindings slot", module.id), slot)?;
            validate_binding_slot(&module.id, slot)?;
            require_non_empty(&format!("modules[{}].bindings.{slot}", module.id), target)?;

            if !enabled_instance_ids.contains(target.as_str()) {
                return Err(format!(
                    "module '{}' binding slot '{}' targets unknown or disabled instance '{}'",
                    module.id, slot, target
                ));
            }
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
            subdir,
        } => {
            require_non_empty(&format!("{label}.repository"), repository)?;
            require_non_empty(&format!("{label}.ref"), git_ref)?;

            if let Some(subdir) = subdir {
                validate_source_subdir(label, subdir)?;
            }
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

fn validate_source_subdir(label: &str, subdir: &str) -> Result<(), String> {
    require_non_empty(&format!("{label}.subdir"), subdir)?;

    if subdir.starts_with('/') || subdir.starts_with('\\') {
        return Err(format!("{label}.subdir must be a relative path"));
    }

    if subdir.contains('\\') {
        return Err(format!("{label}.subdir must use '/' path separators"));
    }

    if subdir
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(format!(
            "{label}.subdir must be a canonical relative path without '.', '..', or empty segments"
        ));
    }

    Ok(())
}

fn validate_binding_slot(module_id: &str, slot: &str) -> Result<(), String> {
    let mut characters = slot.chars();

    let Some(first) = characters.next() else {
        return Err(format!(
            "modules[{module_id}].bindings slot must not be empty"
        ));
    };

    if !(first.is_ascii_alphabetic() || first == '_')
        || !characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Err(format!(
            "modules[{module_id}].bindings slot '{slot}' must match [A-Za-z_][A-Za-z0-9_]*"
        ));
    }

    Ok(())
}

fn validate_target_port(module_id: &str, target_port: u16) -> Result<(), String> {
    if target_port == 0 {
        Err(format!(
            "modules[{module_id}].exposure.targetPort must be greater than 0"
        ))
    } else {
        Ok(())
    }
}

fn validate_prefix(module_id: &str, prefix: &str) -> Result<(), String> {
    if !prefix.starts_with('/') {
        return Err(format!(
            "modules[{module_id}].exposure.prefix must start with '/'"
        ));
    }

    if prefix.len() > 1 && prefix.ends_with('/') {
        return Err(format!(
            "modules[{module_id}].exposure.prefix must not end with '/'"
        ));
    }

    if prefix.contains('?') || prefix.contains('#') || prefix.contains("//") {
        return Err(format!(
            "modules[{module_id}].exposure.prefix must be a canonical URL path prefix"
        ));
    }

    Ok(())
}
