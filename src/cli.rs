mod args;
mod bindings;
mod build;
mod client;
mod deploy;
mod extensions;
mod log;
mod release;
mod staging;
mod workspace;

use std::error::Error;
use std::fmt;
use std::path::Path;

use crate::core::{ModuleDescriptor, ResourceDescriptor};
use client::{CoreClient, DEFAULT_CORE_URL, HealthResponse};
#[cfg(test)]
use client::{HttpEndpoint, decode_chunked};
use extensions::run_extension;
#[cfg(test)]
use extensions::valid_extension_namespace;

pub use args::parse_args;

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Health,
    Log(log::Options),
    Extension {
        namespace: String,
        arguments: Vec<String>,
    },
    ExportBindings {
        plan: String,
        output: String,
    },
    ModuleBind {
        consumer: String,
        slot: String,
        target: String,
        instance_root: Option<String>,
    },
    Use {
        release_id: Option<String>,
        instance_root: Option<String>,
    },
    Ps,
    Resource(Option<String>),
    Plan {
        input: String,
        output: Option<String>,
        ci_output_dir: Option<String>,
    },
    BuildAll {
        source: String,
        output: Option<String>,
        no_docker: bool,
    },
    Build {
        workspace: String,
        revision: String,
        core_image: String,
    },
    DeployRelease {
        id: Option<String>,
        instance_root: Option<String>,
        staged: Option<String>,
        snapshot_only: bool,
        source: Option<String>,
        modules_root: Option<String>,
    },
    Deploy {
        release_dir: String,
    },
    Help,
    Version,
}

pub fn run(command: Command) -> Result<(), CliError> {
    match command {
        Command::Health => print_health(),
        Command::Log(options) => log::run(options),
        Command::Extension {
            namespace,
            arguments,
        } => run_extension(&namespace, &arguments),
        Command::ExportBindings { plan, output } => bindings::export(&plan, &output),
        Command::ModuleBind {
            consumer,
            slot,
            target,
            instance_root,
        } => workspace::bind(instance_root.as_deref(), &consumer, &slot, &target),
        Command::Use {
            release_id,
            instance_root,
        } => workspace::run(instance_root.as_deref(), release_id.as_deref()),
        Command::Ps => print_ps(),
        Command::Resource(id) => print_resource(id.as_deref()),
        Command::Plan {
            input,
            output,
            ci_output_dir,
        } => crate::build_plan::resolve(
            Path::new(&input),
            output.as_deref().map(Path::new),
            ci_output_dir.as_deref().map(Path::new),
        )
        .map_err(|error| CliError::Execution(error.to_string())),
        Command::Deploy { release_dir } => deploy::run(&release_dir),
        Command::DeployRelease {
            id,
            instance_root,
            staged,
            snapshot_only,
            source,
            modules_root,
        } => release::run(release::Options {
            id,
            instance_root,
            staged,
            snapshot_only,
            source,
            modules_root,
        }),
        Command::BuildAll {
            source,
            output,
            no_docker,
        } => build::run_platform(build::PlatformOptions {
            source,
            output,
            no_docker,
        }),
        Command::Build {
            workspace,
            revision,
            core_image,
        } => build::run(&workspace, &revision, &core_image),
        Command::Help => {
            print_help();
            Ok(())
        }
        Command::Version => {
            println!("manafield {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}

fn print_health() -> Result<(), CliError> {
    let client = CoreClient::from_env()?;
    let health: HealthResponse = client.get("/health")?;

    println!(
        "Core {} ({} {})",
        health.status, health.service, health.version
    );

    Ok(())
}

fn print_ps() -> Result<(), CliError> {
    let client = CoreClient::from_env()?;
    let health: HealthResponse = client.get("/health")?;
    let modules: Vec<ModuleDescriptor> = client.get("/modules")?;
    let resources: Vec<ResourceDescriptor> = client.get("/resources")?;

    println!("Manafield");
    println!();
    println!("CORE");
    println!(
        "  {:<24} {} ({})",
        health.service, health.status, health.version
    );

    println!();
    println!("MODULES ({})", modules.len());
    if modules.is_empty() {
        println!("  (none)");
    } else {
        for module in modules {
            println!("  {:<24} registered  {}", module.id, module.version);
        }
    }

    println!();
    println!("RESOURCES ({})", resources.len());
    if resources.is_empty() {
        println!("  (none)");
    } else {
        for resource in resources {
            println!(
                "  {:<24} registered  {}",
                resource.id, resource.resource_type
            );

            for capability in resource.provides.capabilities {
                println!("    - {} @ {}", capability.id, capability.version);
            }
        }
    }

    Ok(())
}

fn print_resource(id: Option<&str>) -> Result<(), CliError> {
    let client = CoreClient::from_env()?;

    match id {
        Some(id) => {
            let resource: ResourceDescriptor = client.get(&format!("/resources/{id}"))?;

            println!("Resource {}", resource.id);
            println!("  Name: {}", resource.name);
            println!("  Type: {}", resource.resource_type);

            if let Some(description) = resource.description {
                println!("  Description: {description}");
            }

            println!("  Capabilities:");
            if resource.provides.capabilities.is_empty() {
                println!("    (none)");
            } else {
                for capability in resource.provides.capabilities {
                    println!("    - {} @ {}", capability.id, capability.version);
                }
            }
        }
        None => {
            let resources: Vec<ResourceDescriptor> = client.get("/resources")?;

            if resources.is_empty() {
                println!("No Resources registered.");
            } else {
                println!("{:<24} {:<18} CAPABILITIES", "ID", "TYPE");

                for resource in resources {
                    let capabilities = resource
                        .provides
                        .capabilities
                        .iter()
                        .map(|capability| format!("{}@{}", capability.id, capability.version))
                        .collect::<Vec<_>>()
                        .join(", ");

                    println!(
                        "{:<24} {:<18} {}",
                        resource.id, resource.resource_type, capabilities
                    );
                }
            }
        }
    }

    Ok(())
}

pub fn print_help() {
    println!(
        "Manafield {}\n\
         \n\
         Usage:\n\
           manafield [COMMAND]\n\
         \n\
         Commands:\n\
           bindings export   Export safe Module binding snapshot for Manage Web\n\
                             options: --plan FILE --output FILE\n\
           module bind A B C  Set consumer A requirement slot B to Instance C\n\
                             option: --instance-root DIR\n\
           use [RELEASE_ID]  Select a Release as editable YAML (or show selection)\n\
                             option: --instance-root DIR\n\
           health            Show Core health\n\
           log               Read private local Core logs (operator-only)\n\
                             options: --file PATH --level LEVEL --source NAME --since 1h --audit --limit N --json\n\
           ps                Show Core, Module, and Resource summary\n\
           resource [ID]     List Resources or show one Resource\n\
           plan [INSTANCE]   Resolve an Instance Definition into a Build Plan\n\
                             options: --output PATH --ci-output DIR\n\
           build all         Build the Manafield platform distribution\n\
                             options: --source DIR --output DIR --no-docker\n\
           build WORKSPACE REVISION CORE_IMAGE  Legacy Jenkins image builder\n\
           deploy [ID]       Snapshot a new Release YAML and deploy its matching artifacts\n\
                             options: --instance-root DIR --source DIR --modules-root DIR --staged-dir DIR --snapshot-only\n\
           deploy RELEASE_DIR Legacy Jenkins Compose deployment\n\
           help              Show this help\n\
           version           Show version\n\
         \n\
         Core server: run manafield-core separately\n\
         \n\
         Environment:\n\
           MANAFIELD_CORE_URL  Core API base URL for CLI commands\n\
                               default: {}",
        env!("CARGO_PKG_VERSION"),
        DEFAULT_CORE_URL
    );
}

#[derive(Debug)]
pub enum CliError {
    Usage(String),
    Execution(String),
    InvalidCoreUrl(String),
    Request { url: String, source: String },
    HttpStatus { url: String, status: u16 },
    Response { url: String, source: String },
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(message) | Self::Execution(message) | Self::InvalidCoreUrl(message) => {
                write!(f, "{message}")
            }
            Self::Request { url, source } => {
                write!(f, "failed to request Core API '{url}': {source}")
            }
            Self::HttpStatus { url, status } => {
                write!(f, "Core API '{url}' returned HTTP {status}")
            }
            Self::Response { url, source } => {
                write!(f, "failed to decode Core response from '{url}': {source}")
            }
        }
    }
}

impl Error for CliError {}

#[cfg(test)]
mod tests;
