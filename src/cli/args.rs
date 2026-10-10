use super::{CliError, Command, extensions::valid_extension_namespace, log};
use std::env;

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
