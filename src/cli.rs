mod bindings;
mod build;
mod deploy;
mod log;
mod release;
mod staging;
mod workspace;

use std::env;
use std::error::Error;
use std::fmt;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::time::Duration;

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::core::{ModuleDescriptor, ResourceDescriptor};

const DEFAULT_CORE_URL: &str = "http://127.0.0.1:8080";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

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

pub fn parse_args<I>(args: I) -> Result<Command, CliError>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();

    let Some(command) = args.next() else {
        return Ok(Command::Help);
    };

    match command.as_str() {
        "serve" => Err(CliError::Usage(
            "Core is a separate executable; run manafield-core".to_owned(),
        )),
        "health" => ensure_no_more(args, Command::Health),
        "log" => Ok(Command::Log(log::parse(args)?)),
        "bindings" => parse_bindings_args(args),
        "use" => parse_use_args(args),
        "module" => parse_module_args(args),
        "ps" => ensure_no_more(args, Command::Ps),
        "resource" | "resources" => {
            let id = args.next();

            if let Some(extra) = args.next() {
                return Err(CliError::Usage(format!(
                    "unexpected argument '{extra}' after resource ID"
                )));
            }

            Ok(Command::Resource(id))
        }
        "plan" => parse_plan_args(args),
        "deploy" => parse_deploy_args(args),
        "build" => parse_build_args(args),
        "help" | "-h" | "--help" => ensure_no_more(args, Command::Help),
        "version" | "-V" | "--version" => ensure_no_more(args, Command::Version),
        other
            if valid_extension_namespace(other)
                && env::var_os("MANAFIELD_CLI_EXTENSIONS_DIR").is_some() =>
        {
            Ok(Command::Extension {
                namespace: other.to_owned(),
                arguments: args.collect(),
            })
        }
        other => Err(CliError::Usage(format!("unknown command '{other}'"))),
    }
}

fn parse_bindings_args<I>(mut args: I) -> Result<Command, CliError>
where
    I: Iterator<Item = String>,
{
    if args.next().as_deref() != Some("export") {
        return Err(CliError::Usage(
            "bindings requires: export --plan BUILD-PLAN.json --output FILE".into(),
        ));
    }
    let (mut plan, mut output) = (None, None);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--plan" if plan.is_none() => {
                plan = Some(args.next().ok_or_else(|| {
                    CliError::Usage("--plan requires a JSON Build Plan path".into())
                })?);
            }
            "--output" if output.is_none() => {
                output = Some(
                    args.next()
                        .ok_or_else(|| CliError::Usage("--output requires a path".into()))?,
                );
            }
            other => {
                return Err(CliError::Usage(format!(
                    "unknown or repeated bindings export option '{other}'"
                )));
            }
        }
    }
    Ok(Command::ExportBindings {
        plan: plan.ok_or_else(|| CliError::Usage("missing --plan".into()))?,
        output: output.ok_or_else(|| CliError::Usage("missing --output".into()))?,
    })
}

fn parse_module_args<I>(mut args: I) -> Result<Command, CliError>
where
    I: Iterator<Item = String>,
{
    let Some(operation) = args.next() else {
        return Err(CliError::Usage(
            "module requires a subcommand (bind)".to_owned(),
        ));
    };
    if operation != "bind" {
        return Err(CliError::Usage(format!(
            "unknown module command '{operation}'"
        )));
    }
    let mut positional = Vec::new();
    let mut instance_root = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--instance-root" if instance_root.is_none() => {
                instance_root = Some(args.next().ok_or_else(|| {
                    CliError::Usage("--instance-root requires a directory".to_owned())
                })?);
            }
            flag if flag.starts_with('-') => {
                return Err(CliError::Usage(format!(
                    "unknown/repeated module option '{flag}'"
                )));
            }
            value => positional.push(value.to_owned()),
        }
    }
    if positional.len() != 3 {
        return Err(CliError::Usage(
            "module bind requires CONSUMER SLOT TARGET [--instance-root DIR]".to_owned(),
        ));
    }
    Ok(Command::ModuleBind {
        consumer: positional.remove(0),
        slot: positional.remove(0),
        target: positional.remove(0),
        instance_root,
    })
}

fn parse_use_args<I>(mut args: I) -> Result<Command, CliError>
where
    I: Iterator<Item = String>,
{
    let mut release_id = None;
    let mut instance_root = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--instance-root" if instance_root.is_none() => {
                instance_root = Some(args.next().ok_or_else(|| {
                    CliError::Usage("--instance-root requires a directory".to_owned())
                })?);
            }
            flag if flag.starts_with('-') => {
                return Err(CliError::Usage(format!(
                    "unknown/repeated use option '{flag}'"
                )));
            }
            id if release_id.is_none() => release_id = Some(id.to_owned()),
            other => {
                return Err(CliError::Usage(format!(
                    "unexpected argument '{other}' after Release ID"
                )));
            }
        }
    }
    Ok(Command::Use {
        release_id,
        instance_root,
    })
}

fn parse_plan_args<I>(mut args: I) -> Result<Command, CliError>
where
    I: Iterator<Item = String>,
{
    let mut input = None;
    let mut output = None;
    let mut ci_output_dir = None;

    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--output" => {
                if output.is_some() {
                    return Err(CliError::Usage(
                        "--output may only be specified once".to_owned(),
                    ));
                }

                output = Some(
                    args.next()
                        .ok_or_else(|| CliError::Usage("--output requires a path".to_owned()))?,
                );
            }
            "--ci-output" => {
                if ci_output_dir.is_some() {
                    return Err(CliError::Usage(
                        "--ci-output may only be specified once".to_owned(),
                    ));
                }

                ci_output_dir =
                    Some(args.next().ok_or_else(|| {
                        CliError::Usage("--ci-output requires a path".to_owned())
                    })?);
            }
            option if option.starts_with('-') => {
                return Err(CliError::Usage(format!("unknown plan option '{option}'")));
            }
            value => {
                if input.is_some() {
                    return Err(CliError::Usage(format!(
                        "unexpected argument '{value}' after Instance Definition"
                    )));
                }
                input = Some(value.to_owned());
            }
        }
    }

    Ok(Command::Plan {
        input: input.unwrap_or_else(|| "instance.yaml".to_owned()),
        output,
        ci_output_dir,
    })
}

fn parse_build_args<I>(mut args: I) -> Result<Command, CliError>
where
    I: Iterator<Item = String>,
{
    match args.next() {
        Some(first) if first == "all" => parse_build_all_args(args),
        Some(first) => parse_legacy_build_args(args, first),
        None => Err(CliError::Usage(
            "build requires a target (try 'manafield build all')".to_owned(),
        )),
    }
}

// Previous Jenkins invocation kept as a compatibility adapter until its
// Module image pipeline moves into Instance/module management.
fn parse_legacy_build_args<I>(mut args: I, first: String) -> Result<Command, CliError>
where
    I: Iterator<Item = String>,
{
    let workspace = first;
    let revision = args
        .next()
        .ok_or_else(|| CliError::Usage("build requires REVISION".to_owned()))?;
    let core_image = args
        .next()
        .ok_or_else(|| CliError::Usage("build requires CORE_IMAGE".to_owned()))?;
    ensure_no_more(
        args,
        Command::Build {
            workspace,
            revision,
            core_image,
        },
    )
}

fn parse_build_all_args<I>(mut args: I) -> Result<Command, CliError>
where
    I: Iterator<Item = String>,
{
    let mut source: Option<String> = None;
    let mut output: Option<String> = None;
    let mut no_docker = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--source" if source.is_none() => {
                source = Some(
                    args.next()
                        .ok_or_else(|| CliError::Usage("--source requires a path".to_owned()))?,
                );
            }
            "--output" if output.is_none() => {
                output = Some(
                    args.next()
                        .ok_or_else(|| CliError::Usage("--output requires a path".to_owned()))?,
                );
            }
            "--no-docker" if !no_docker => no_docker = true,
            _ => {
                return Err(CliError::Usage(format!(
                    "unknown or repeated build all option '{arg}'"
                )));
            }
        }
    }
    Ok(Command::BuildAll {
        source: source.unwrap_or_else(|| ".".to_owned()),
        output,
        no_docker,
    })
}

fn parse_deploy_args<I>(args: I) -> Result<Command, CliError>
where
    I: Iterator<Item = String>,
{
    let mut args = args.peekable();
    // Existing Jenkins-compatible `deploy RELEASE_DIR` remains unmodified.
    if let Some(first) = args.peek()
        && (first.starts_with('/') || first.starts_with('.') || first.contains('/'))
    {
        let release_dir = args.next().unwrap();
        return ensure_no_more(args, Command::Deploy { release_dir });
    }

    let mut id = None;
    let mut instance_root = None;
    let mut staged = None;
    let mut snapshot_only = false;
    let mut source = None;
    let mut modules_root = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--instance-root" if instance_root.is_none() => {
                instance_root = Some(args.next().ok_or_else(|| {
                    CliError::Usage("--instance-root requires a directory".into())
                })?);
            }
            "--staged-dir" if staged.is_none() => {
                staged =
                    Some(args.next().ok_or_else(|| {
                        CliError::Usage("--staged-dir requires a directory".into())
                    })?);
            }
            "--snapshot-only" if !snapshot_only => snapshot_only = true,
            "--source" if source.is_none() => {
                source = Some(
                    args.next()
                        .ok_or_else(|| CliError::Usage("--source requires a directory".into()))?,
                );
            }
            "--modules-root" if modules_root.is_none() => {
                modules_root = Some(args.next().ok_or_else(|| {
                    CliError::Usage("--modules-root requires a directory".into())
                })?);
            }
            other if other.starts_with('-') => {
                return Err(CliError::Usage(format!(
                    "unknown or repeated deploy option '{other}'"
                )));
            }
            other if id.is_none() => id = Some(other.to_owned()),
            other => {
                return Err(CliError::Usage(format!(
                    "unexpected deploy argument '{other}'"
                )));
            }
        }
    }
    if snapshot_only && staged.is_some() {
        return Err(CliError::Usage(
            "--snapshot-only cannot be used with --staged-dir".into(),
        ));
    }
    // Preserve the historical one-argument Jenkins/Compose deployment grammar.
    if let Some(value) = id.as_deref()
        && !value.starts_with('v')
        && instance_root.is_none()
        && staged.is_none()
        && !snapshot_only
    {
        return Ok(Command::Deploy {
            release_dir: value.to_owned(),
        });
    }
    Ok(Command::DeployRelease {
        id,
        instance_root,
        staged,
        snapshot_only,
        source,
        modules_root,
    })
}

fn ensure_no_more<I>(mut args: I, command: Command) -> Result<Command, CliError>
where
    I: Iterator<Item = String>,
{
    if let Some(extra) = args.next() {
        Err(CliError::Usage(format!("unexpected argument '{extra}'")))
    } else {
        Ok(command)
    }
}

// Plugins are opt-in executable adapters from a *trusted directory* and are
// executed without a shell. A Module may offer CLI features without extending
// the Rust Core with its domain-specific operations.
fn valid_extension_namespace(name: &str) -> bool {
    let bytes = name.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 64
        && bytes[0].is_ascii_lowercase()
        && bytes
            .iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

fn run_extension(namespace: &str, arguments: &[String]) -> Result<(), CliError> {
    let directory = env::var_os("MANAFIELD_CLI_EXTENSIONS_DIR").ok_or_else(|| {
        CliError::Usage("set MANAFIELD_CLI_EXTENSIONS_DIR to a trusted plugin directory".into())
    })?;
    if !valid_extension_namespace(namespace) {
        return Err(CliError::Usage("invalid CLI extension namespace".into()));
    }
    let executable = PathBuf::from(directory).join(format!("manafield-{namespace}"));
    let info = std::fs::symlink_metadata(&executable)
        .map_err(|_| CliError::Usage(format!("CLI extension '{namespace}' is not installed")))?;
    if !info.file_type().is_file() {
        return Err(CliError::Usage(
            "CLI extension must be a regular file (not a symlink)".into(),
        ));
    }
    let status = ProcessCommand::new(&executable)
        .args(arguments)
        .status()
        .map_err(|e| CliError::Execution(format!("cannot run CLI extension '{namespace}': {e}")))?;
    if !status.success() {
        return Err(CliError::Execution(format!(
            "CLI extension '{namespace}' exited with {status}"
        )));
    }
    Ok(())
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

#[derive(Debug, Deserialize)]
struct HealthResponse {
    status: String,
    service: String,
    version: String,
}

struct CoreClient {
    endpoint: HttpEndpoint,
}

impl CoreClient {
    fn from_env() -> Result<Self, CliError> {
        let base_url =
            env::var("MANAFIELD_CORE_URL").unwrap_or_else(|_| DEFAULT_CORE_URL.to_owned());

        Ok(Self {
            endpoint: HttpEndpoint::parse(&base_url)?,
        })
    }

    fn get<T>(&self, path: &str) -> Result<T, CliError>
    where
        T: DeserializeOwned,
    {
        let target = self.endpoint.target(path);
        let url = format!("http://{}{}", self.endpoint.authority, target);

        let mut stream =
            TcpStream::connect(&self.endpoint.address).map_err(|error| CliError::Request {
                url: url.clone(),
                source: error.to_string(),
            })?;

        stream
            .set_read_timeout(Some(REQUEST_TIMEOUT))
            .map_err(|error| CliError::Request {
                url: url.clone(),
                source: error.to_string(),
            })?;
        stream
            .set_write_timeout(Some(REQUEST_TIMEOUT))
            .map_err(|error| CliError::Request {
                url: url.clone(),
                source: error.to_string(),
            })?;

        let request = format!(
            "GET {target} HTTP/1.1\r\n\
             Host: {}\r\n\
             Accept: application/json\r\n\
             Connection: close\r\n\
             User-Agent: manafield/{}\r\n\
             \r\n",
            self.endpoint.authority,
            env!("CARGO_PKG_VERSION")
        );

        stream
            .write_all(request.as_bytes())
            .map_err(|error| CliError::Request {
                url: url.clone(),
                source: error.to_string(),
            })?;

        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .map_err(|error| CliError::Request {
                url: url.clone(),
                source: error.to_string(),
            })?;

        let header_end = find_bytes(&response, b"\r\n\r\n").ok_or_else(|| CliError::Response {
            url: url.clone(),
            source: "invalid HTTP response".to_owned(),
        })?;

        let headers =
            std::str::from_utf8(&response[..header_end]).map_err(|error| CliError::Response {
                url: url.clone(),
                source: error.to_string(),
            })?;

        let status = headers
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|status| status.parse::<u16>().ok())
            .ok_or_else(|| CliError::Response {
                url: url.clone(),
                source: "invalid HTTP status line".to_owned(),
            })?;

        if !(200..300).contains(&status) {
            return Err(CliError::HttpStatus { url, status });
        }

        let raw_body = &response[header_end + 4..];
        let body = if headers
            .lines()
            .any(|line| line.eq_ignore_ascii_case("transfer-encoding: chunked"))
        {
            decode_chunked(raw_body).map_err(|source| CliError::Response {
                url: url.clone(),
                source,
            })?
        } else {
            raw_body.to_vec()
        };

        serde_json::from_slice(&body).map_err(|error| CliError::Response {
            url,
            source: error.to_string(),
        })
    }
}

struct HttpEndpoint {
    authority: String,
    address: String,
    base_path: String,
}

impl HttpEndpoint {
    fn parse(base_url: &str) -> Result<Self, CliError> {
        let raw = base_url.strip_prefix("http://").ok_or_else(|| {
            CliError::InvalidCoreUrl("MANAFIELD_CORE_URL must use http:// in CLI v0".to_owned())
        })?;

        let (authority, path) = match raw.split_once('/') {
            Some((authority, path)) => (authority, path),
            None => (raw, ""),
        };

        if authority.is_empty() {
            return Err(CliError::InvalidCoreUrl(
                "MANAFIELD_CORE_URL is missing a host".to_owned(),
            ));
        }

        if authority.starts_with('[') {
            return Err(CliError::InvalidCoreUrl(
                "IPv6 Core URLs are not supported by CLI v0".to_owned(),
            ));
        }

        let address = match authority.rsplit_once(':') {
            Some((host, port)) if !host.is_empty() && port.parse::<u16>().is_ok() => {
                authority.to_owned()
            }
            Some(_) => {
                return Err(CliError::InvalidCoreUrl(
                    "MANAFIELD_CORE_URL contains an invalid port".to_owned(),
                ));
            }
            None => format!("{authority}:80"),
        };

        let base_path = if path.trim_matches('/').is_empty() {
            String::new()
        } else {
            format!("/{}", path.trim_matches('/'))
        };

        Ok(Self {
            authority: authority.to_owned(),
            address,
            base_path,
        })
    }

    fn target(&self, path: &str) -> String {
        format!("{}{}", self.base_path, path)
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn decode_chunked(input: &[u8]) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    let mut cursor = 0;

    loop {
        let line_end = find_bytes(&input[cursor..], b"\r\n")
            .map(|offset| cursor + offset)
            .ok_or_else(|| "invalid chunked response".to_owned())?;

        let size_line =
            std::str::from_utf8(&input[cursor..line_end]).map_err(|error| error.to_string())?;
        let size_hex = size_line.split(';').next().unwrap_or(size_line);
        let size = usize::from_str_radix(size_hex.trim(), 16).map_err(|error| error.to_string())?;

        cursor = line_end + 2;

        if size == 0 {
            break;
        }

        let chunk_end = cursor
            .checked_add(size)
            .filter(|end| *end <= input.len())
            .ok_or_else(|| "truncated chunked response".to_owned())?;

        output.extend_from_slice(&input[cursor..chunk_end]);
        cursor = chunk_end;

        if input.get(cursor..cursor + 2) != Some(b"\r\n") {
            return Err("invalid chunk terminator".to_owned());
        }

        cursor += 2;
    }

    Ok(output)
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
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn no_arguments_prints_help() {
        assert_eq!(parse_args(args(&[])).unwrap(), Command::Help);
    }

    #[test]
    fn serve_command_points_to_separate_binary() {
        assert!(matches!(
            parse_args(args(&["serve"])),
            Err(CliError::Usage(_))
        ));
    }

    #[test]
    fn parses_bindings_export() {
        assert_eq!(
            parse_args(args(&[
                "bindings",
                "export",
                "--plan",
                "build-plan.json",
                "--output",
                "bindings.json"
            ]))
            .unwrap(),
            Command::ExportBindings {
                plan: "build-plan.json".to_owned(),
                output: "bindings.json".to_owned(),
            }
        );
        for invalid in [
            vec!["bindings", "export", "--plan", "plan.json"],
            vec!["bindings", "export", "--output", "bindings.json"],
            vec![
                "bindings", "export", "--plan", "a", "--output", "b", "--plan", "c",
            ],
        ] {
            assert!(matches!(
                parse_args(args(&invalid)),
                Err(CliError::Usage(_))
            ));
        }
    }

    #[test]
    fn parses_ps() {
        assert_eq!(parse_args(args(&["ps"])).unwrap(), Command::Ps);
    }

    #[test]
    fn parses_resource_with_id() {
        assert_eq!(
            parse_args(args(&["resource", "main-db"])).unwrap(),
            Command::Resource(Some("main-db".to_owned()))
        );
    }

    #[test]
    fn parses_plan_with_outputs() {
        assert_eq!(
            parse_args(args(&[
                "plan",
                "/tmp/instance.yaml",
                "--output",
                "/tmp/build-plan.json",
                "--ci-output",
                "/tmp/ci-plan",
            ]))
            .unwrap(),
            Command::Plan {
                input: "/tmp/instance.yaml".to_owned(),
                output: Some("/tmp/build-plan.json".to_owned()),
                ci_output_dir: Some("/tmp/ci-plan".to_owned()),
            }
        );
    }

    #[test]
    fn plan_defaults_to_instance_yaml() {
        assert_eq!(
            parse_args(args(&["plan"])).unwrap(),
            Command::Plan {
                input: "instance.yaml".to_owned(),
                output: None,
                ci_output_dir: None,
            }
        );
    }

    #[test]
    fn parses_direct_deploy_options() {
        assert_eq!(
            parse_args(args(&[
                "deploy",
                "--instance-root",
                "/instance",
                "--source",
                "/repo",
                "--modules-root",
                "/local"
            ]))
            .unwrap(),
            Command::DeployRelease {
                id: None,
                instance_root: Some("/instance".into()),
                staged: None,
                snapshot_only: false,
                source: Some("/repo".into()),
                modules_root: Some("/local".into()),
            }
        );
        assert!(parse_args(args(&["deploy", "--source"])).is_err());
        assert!(parse_args(args(&["deploy", "--modules-root"])).is_err());
        assert!(parse_args(args(&["deploy", "--source", "a", "--source", "b"])).is_err());
    }

    #[test]
    fn parses_release_deploy_and_legacy_deploy_separately() {
        assert_eq!(
            parse_args(args(&[
                "deploy",
                "v3_20261008T070000Z",
                "--instance-root",
                "/tmp/instance",
                "--snapshot-only"
            ]))
            .unwrap(),
            Command::DeployRelease {
                id: Some("v3_20261008T070000Z".to_owned()),
                instance_root: Some("/tmp/instance".to_owned()),
                staged: None,
                snapshot_only: true,
                source: None,
                modules_root: None,
            }
        );
        assert_eq!(
            parse_args(args(&["deploy", "/tmp/staged-release"])).unwrap(),
            Command::Deploy {
                release_dir: "/tmp/staged-release".to_owned()
            }
        );
        assert!(
            parse_args(args(&[
                "deploy",
                "v3_20261008T070000Z",
                "--snapshot-only",
                "--staged-dir",
                "/tmp/staged"
            ]))
            .is_err()
        );
        assert!(parse_args(args(&["deploy", "v3_20261008T070000Z", "unexpected"])).is_err());
    }

    #[test]
    fn parses_module_bind() {
        assert_eq!(
            parse_args(args(&[
                "module",
                "bind",
                "echo",
                "state",
                "main-postgres",
                "--instance-root",
                "/tmp/i"
            ]))
            .unwrap(),
            Command::ModuleBind {
                consumer: "echo".to_owned(),
                slot: "state".to_owned(),
                target: "main-postgres".to_owned(),
                instance_root: Some("/tmp/i".to_owned()),
            }
        );
        assert!(parse_args(args(&["module", "bind", "echo", "state"])).is_err());
        assert!(parse_args(args(&["module", "unknown"])).is_err());
        assert!(parse_args(args(&["module", "bind", "echo", "state", "db", "extra"])).is_err());
    }

    #[test]
    fn parses_use_selection_and_status() {
        assert_eq!(
            parse_args(args(&[
                "use",
                "v1_20261008T070000Z",
                "--instance-root",
                "/tmp/i"
            ]))
            .unwrap(),
            Command::Use {
                release_id: Some("v1_20261008T070000Z".to_owned()),
                instance_root: Some("/tmp/i".to_owned())
            }
        );
        assert_eq!(
            parse_args(args(&["use"])).unwrap(),
            Command::Use {
                release_id: None,
                instance_root: None
            }
        );
        assert!(parse_args(args(&["use", "--instance-root"])).is_err());
    }

    #[test]
    fn parses_platform_build_all() {
        assert_eq!(
            parse_args(args(&[
                "build",
                "all",
                "--source",
                "/src",
                "--output",
                "/tmp/dist",
                "--no-docker"
            ]))
            .unwrap(),
            Command::BuildAll {
                source: "/src".to_owned(),
                output: Some("/tmp/dist".to_owned()),
                no_docker: true
            }
        );
        assert_eq!(
            parse_args(args(&["build", "all"])).unwrap(),
            Command::BuildAll {
                source: ".".to_owned(),
                output: None,
                no_docker: false
            }
        );
    }

    #[test]
    fn rejects_unknown_or_duplicate_platform_options() {
        assert!(parse_args(args(&["build", "all", "--no-cache"])).is_err());
        assert!(parse_args(args(&["build", "all", "--output"])).is_err());
        assert!(parse_args(args(&["build", "all", "--no-docker", "--no-docker"])).is_err());
    }

    #[test]
    fn parses_build_arguments() {
        assert_eq!(
            parse_args(args(&[
                "build",
                "/tmp/workspace",
                "abcdef",
                "manafield-core:abcdef"
            ]))
            .unwrap(),
            Command::Build {
                workspace: "/tmp/workspace".to_owned(),
                revision: "abcdef".to_owned(),
                core_image: "manafield-core:abcdef".to_owned(),
            }
        );
    }

    #[test]
    fn parses_deploy_release_directory() {
        assert_eq!(
            parse_args(args(&["deploy", "/tmp/release 123"])).unwrap(),
            Command::Deploy {
                release_dir: "/tmp/release 123".to_owned(),
            }
        );
    }

    #[test]
    fn deploy_supports_generated_ids_but_rejects_invalid_legacy_args() {
        assert_eq!(
            parse_args(args(&["deploy"])).unwrap(),
            Command::DeployRelease {
                id: None,
                instance_root: None,
                staged: None,
                snapshot_only: false,
                source: None,
                modules_root: None,
            }
        );
        assert!(matches!(
            parse_args(args(&["deploy", "/tmp/release", "extra"])),
            Err(CliError::Usage(_))
        ));
        assert!(matches!(
            parse_args(args(&["deploy", "--unexpected"])),
            Err(CliError::Usage(_))
        ));
    }

    #[test]
    fn validates_extension_namespace() {
        for accepted in ["account", "echo", "my-module", "v2"] {
            assert!(valid_extension_namespace(accepted), "{accepted}");
        }
        for rejected in [
            "",
            "-bad",
            "Account",
            "../bad",
            "echo/../../bin/sh",
            "abc.def",
            "📦",
        ] {
            assert!(!valid_extension_namespace(rejected), "{rejected}");
        }
    }

    #[test]
    fn parses_log_command_without_accessing_core_http() {
        let parsed = parse_args(args(&["log", "--level", "warn", "--source", "account"])).unwrap();
        match parsed {
            Command::Log(opts) => {
                assert_eq!(opts.level.as_deref(), Some("warn"));
                assert_eq!(opts.source.as_deref(), Some("account"));
            }
            _ => panic!("log command not registered"),
        }
    }

    #[test]
    fn rejects_unknown_command() {
        assert!(matches!(
            parse_args(args(&["wat"])),
            Err(CliError::Usage(_))
        ));
    }

    #[test]
    fn parses_core_url() {
        let endpoint = HttpEndpoint::parse("http://127.0.0.1:18080/base").unwrap();

        assert_eq!(endpoint.address, "127.0.0.1:18080");
        assert_eq!(endpoint.authority, "127.0.0.1:18080");
        assert_eq!(endpoint.target("/health"), "/base/health");
    }

    #[test]
    fn decodes_chunked_body() {
        let decoded = decode_chunked(b"4\r\ntest\r\n0\r\n\r\n").unwrap();

        assert_eq!(decoded, b"test");
    }
}
