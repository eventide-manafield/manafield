//! Encode the resolved Build Plan into the Jenkins compatibility artifacts.
use super::*;

pub(super) fn write_ci_plan(
    plan: &BuildPlan,
    directory: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
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
        let (source_type, source_value, source_ref, source_subdir) = match &module.source {
            ModuleSourceDefinition::Git {
                repository,
                git_ref,
                subdir,
            } => (
                "git",
                repository.as_str(),
                git_ref.as_str(),
                subdir.as_deref().unwrap_or("."),
            ),
            ModuleSourceDefinition::Dir => ("dir", "-", "-", "."),
        };

        modules.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            module.id,
            source_type,
            source_value,
            source_ref,
            source_subdir,
            module.build.build_type.as_str(),
            module.build.context,
            module.build.dockerfile,
        ));
    }
    fs::write(directory.join("modules.tsv"), modules)?;

    let mut module_exposures = String::new();
    for module in &plan.modules {
        let Some(exposure) = &module.exposure else {
            continue;
        };

        match exposure {
            ExposureDefinition::Host { host, target_port } => {
                module_exposures.push_str(&format!(
                    "{}\thost\t{}\t-\t{}\n",
                    module.id, host, target_port
                ));
            }
            ExposureDefinition::Prefix {
                host,
                prefix,
                target_port,
            } => {
                module_exposures.push_str(&format!(
                    "{}\tprefix\t{}\t{}\t{}\n",
                    module.id, host, prefix, target_port
                ));
            }
        }
    }
    fs::write(directory.join("module-exposures.tsv"), module_exposures)?;

    let mut module_bindings = String::new();
    for module in &plan.modules {
        for (slot, target) in &module.bindings {
            module_bindings.push_str(&format!("{}\t{}\t{}\n", module.id, slot, target));
        }
    }
    fs::write(directory.join("module-bindings.tsv"), module_bindings)?;

    // Keep Core's optional state binding separate from Module bindings: Core
    // has no Module manifest nor module-sources.tsv entry.
    let mut core_bindings = String::new();
    for (slot, target) in &plan.core.bindings {
        core_bindings.push_str(&format!("{slot}\t{target}\n"));
    }
    fs::write(directory.join("core-bindings.tsv"), core_bindings)?;

    let mut resources = String::new();
    for resource in &plan.resources {
        resources.push_str(&format!("{}\t{}\n", resource.id, resource.provider,));
    }
    fs::write(directory.join("resources.tsv"), resources)?;

    Ok(())
}
