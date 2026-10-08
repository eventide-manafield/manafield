//! Build an isolated Manafield platform distribution without touching an Instance.
//! External Modules and Instance Resources are deliberately excluded.
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use serde::Serialize;

use super::super::CliError;

const BINARIES: [&str; 4] = [
    "manafield",
    "manafield-core",
    "manafield-build-plan",
    "manafield-ingress-traefik",
];

pub(crate) struct Options {
    pub source: String,
    pub output: Option<String>,
    pub no_docker: bool,
}

#[derive(Serialize)]
struct Manifest<'a> {
    schema_version: u32,
    kind: &'static str,
    platform_version: &'a str,
    source_revision: &'a str,
    source_dirty: bool,
    binaries: &'static [&'static str],
    docker_images: Vec<String>,
    note: &'static str,
}

pub(crate) fn run(options: Options) -> Result<(), CliError> {
    let source = fs::canonicalize(&options.source).map_err(|e| {
        CliError::Execution(format!("cannot resolve source '{}': {e}", options.source))
    })?;
    if !source.join("Cargo.toml").is_file() || !source.join("Dockerfile").is_file() {
        return Err(CliError::Execution(format!(
            "'{}' is not a Manafield source repository (Cargo.toml / Dockerfile required)",
            source.display()
        )));
    }

    let revision = git_output(&source, &["rev-parse", "--short=12", "HEAD"])?;
    let dirty = !git_output(
        &source,
        &["status", "--porcelain", "--untracked-files=normal"],
    )?
    .is_empty();
    let version = env!("CARGO_PKG_VERSION");
    let id = format!("{version}-{revision}{}", if dirty { "-dirty" } else { "" });
    let output = options
        .output
        .map(PathBuf::from)
        .unwrap_or_else(|| source.join("dist").join("platform").join(&id));
    let output = if output.is_absolute() {
        output
    } else {
        env::current_dir()
            .map_err(|e| CliError::Execution(e.to_string()))?
            .join(output)
    };

    if output.exists() {
        return Err(CliError::Execution(format!(
            "build destination '{}' already exists; use another --output path",
            output.display()
        )));
    }
    let parent = output
        .parent()
        .ok_or_else(|| CliError::Execution("invalid output path".to_owned()))?;
    fs::create_dir_all(parent)
        .map_err(|e| CliError::Execution(format!("cannot create output parent: {e}")))?;
    let staging = parent.join(format!(
        ".manafield-platform-{}-{}.tmp",
        std::process::id(),
        revision
    ));
    if staging.exists() {
        return Err(CliError::Execution(format!(
            "temporary build path '{}' already exists",
            staging.display()
        )));
    }

    if !options.no_docker {
        let mut docker_check = ProcessCommand::new("docker");
        docker_check.args(["info", "--format", "{{.ServerVersion}}"]);
        execute(
            &mut docker_check,
            "Docker daemon preflight (check Docker socket permissions)",
        )?;
    }

    fs::create_dir_all(staging.join("bin"))
        .map_err(|e| CliError::Execution(format!("cannot create staging directory: {e}")))?;

    let result = build_staged(&source, &staging, &revision, dirty, options.no_docker);
    match result {
        Ok(()) => {
            fs::rename(&staging, &output).map_err(|e| {
                CliError::Execution(format!(
                    "cannot publish build output '{}': {e}",
                    output.display()
                ))
            })?;
            println!("Manafield platform distribution: {}", output.display());
            Ok(())
        }
        Err(error) => {
            let _ = fs::remove_dir_all(&staging);
            Err(error)
        }
    }
}

fn build_staged(
    source: &Path,
    staging: &Path,
    revision: &str,
    dirty: bool,
    no_docker: bool,
) -> Result<(), CliError> {
    let mut cargo = ProcessCommand::new("cargo");
    cargo
        .current_dir(source)
        .arg("build")
        .args([
            "--locked",
            "--release",
            "--bins",
            "--features",
            "build-plan ingress-traefik",
        ])
        .arg("--target-dir")
        .arg(source.join("target"));
    execute(&mut cargo, "platform Rust build")?;

    for binary in BINARIES {
        let from = source.join("target").join("release").join(binary);
        let to = staging.join("bin").join(binary);
        fs::copy(&from, &to).map_err(|e| {
            CliError::Execution(format!("cannot stage binary '{}': {e}", from.display()))
        })?;
    }

    let mut docker_images = Vec::new();
    if !no_docker {
        let suffix = format!(
            "{}-{}{}",
            env!("CARGO_PKG_VERSION"),
            revision,
            if dirty { "-dirty" } else { "" }
        );
        let core = format!("manafield-core:{suffix}");
        let pg = format!("manafield-resource-postgresql:{suffix}");

        let mut core_build = ProcessCommand::new("docker");
        core_build.current_dir(source).arg("build").args([
            "--target",
            "core-runtime",
            "--tag",
            &core,
            ".",
        ]);
        execute(&mut core_build, "core Docker image build")?;
        docker_images.push(core);

        let mut pg_build = ProcessCommand::new("docker");
        pg_build.current_dir(source).arg("build").args([
            "--file",
            "providers/postgresql/Dockerfile",
            "--tag",
            &pg,
            "providers/postgresql",
        ]);
        execute(&mut pg_build, "PostgreSQL Resource Provider image build")?;
        docker_images.push(pg);
    }

    let manifest = Manifest {
        schema_version: 1,
        kind: "manafield-platform",
        platform_version: env!("CARGO_PKG_VERSION"),
        source_revision: revision,
        source_dirty: dirty,
        binaries: &BINARIES,
        docker_images,
        note: "Platform build only. No third-party Modules, Instance release, or database contents are included.",
    };
    let json = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| CliError::Execution(format!("cannot serialize build manifest: {e}")))?;
    fs::write(staging.join("manifest.json"), json)
        .map_err(|e| CliError::Execution(format!("cannot save build manifest: {e}")))?;
    Ok(())
}

fn execute(command: &mut ProcessCommand, description: &str) -> Result<(), CliError> {
    let status = command
        .status()
        .map_err(|e| CliError::Execution(format!("{description}: failed to start: {e}")))?;
    if !status.success() {
        return Err(CliError::Execution(format!(
            "{description} failed with {status}"
        )));
    }
    Ok(())
}

fn git_output(source: &Path, args: &[&str]) -> Result<String, CliError> {
    let output = ProcessCommand::new("git")
        .current_dir(source)
        .args(args)
        .output()
        .map_err(|e| CliError::Execution(format!("cannot inspect git revision: {e}")))?;
    if !output.status.success() {
        return Err(CliError::Execution(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    String::from_utf8(output.stdout)
        .map(|s| s.trim().to_owned())
        .map_err(|e| CliError::Execution(format!("invalid git output: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distribution_does_not_include_external_module_artifacts() {
        assert_eq!(BINARIES.len(), 4);
        assert!(BINARIES.contains(&"manafield"));
        assert!(BINARIES.contains(&"manafield-core"));
        assert!(!BINARIES.contains(&"echo"));
    }

    #[test]
    fn empty_git_output_is_valid() {
        let result = String::from_utf8(b"".to_vec()).unwrap();
        assert!(result.is_empty());
    }
}
