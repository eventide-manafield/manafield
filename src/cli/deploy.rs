//! v0 staged-release Docker Compose executor, run by the CLI (not the Core API).

use std::fs;
use std::path::Path;
use std::process::Command as ProcessCommand;

use super::CliError;

const REQUIRED_FILES: [&str; 4] = [
    "release.env",
    "compose.yml",
    "modules.compose.yml",
    "compose-profiles.txt",
];

pub(super) fn run(release_dir: &str) -> Result<(), CliError> {
    let release_dir = Path::new(release_dir);
    for name in REQUIRED_FILES {
        let path = release_dir.join(name);
        if !path.is_file() {
            return Err(CliError::Execution(format!(
                "staged release is missing required file '{}'",
                path.display()
            )));
        }
    }

    let profiles_path = release_dir.join("compose-profiles.txt");
    let profiles = fs::read_to_string(&profiles_path).map_err(|error| {
        CliError::Execution(format!(
            "failed to read Compose profiles from '{}': {error}",
            profiles_path.display()
        ))
    })?;
    let profiles = profiles.trim();

    for args in [
        &["up", "-d", "--no-build", "--remove-orphans"][..],
        &["ps"][..],
    ] {
        let status = compose_command(release_dir, profiles, args)
            .status()
            .map_err(|error| {
                CliError::Execution(format!(
                    "failed to run Docker Compose (Docker CLI and Compose plugin are required): {error}"
                ))
            })?;

        if !status.success() {
            return Err(CliError::Execution(format!(
                "Docker Compose '{}' failed with {status}",
                args.join(" ")
            )));
        }
    }

    Ok(())
}

fn compose_command(release_dir: &Path, profiles: &str, args: &[&str]) -> ProcessCommand {
    let mut command = ProcessCommand::new("docker");
    command
        .arg("compose")
        .arg("--env-file")
        .arg(release_dir.join("release.env"))
        .arg("--file")
        .arg(release_dir.join("compose.yml"))
        .arg("--file")
        .arg(release_dir.join("modules.compose.yml"))
        .args(args);

    // Empty staged profiles must not inherit unrelated COMPOSE_PROFILES.
    if profiles.is_empty() {
        command.env_remove("COMPOSE_PROFILES");
    } else {
        command.env("COMPOSE_PROFILES", profiles);
    }

    command
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn builds_equivalent_compose_command_without_shell_parsing() {
        let command = compose_command(
            Path::new("/tmp/release 123"),
            "postgresql",
            &["up", "-d", "--no-build", "--remove-orphans"],
        );
        let args = command
            .get_args()
            .map(|value| value.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(command.get_program(), OsStr::new("docker"));
        assert_eq!(
            args,
            vec![
                "compose", "--env-file", "/tmp/release 123/release.env",
                "--file", "/tmp/release 123/compose.yml",
                "--file", "/tmp/release 123/modules.compose.yml",
                "up", "-d", "--no-build", "--remove-orphans",
            ]
        );
        assert_eq!(
            command.get_envs()
                .find(|(key, _)| *key == OsStr::new("COMPOSE_PROFILES"))
                .and_then(|(_, value)| value),
            Some(OsStr::new("postgresql"))
        );
    }

    #[test]
    fn empty_profiles_clear_inherited_compose_profiles() {
        let command = compose_command(Path::new("/tmp/release"), "", &["ps"]);
        assert!(command.get_envs().any(|(key, value)| {
            key == OsStr::new("COMPOSE_PROFILES") && value.is_none()
        }));
    }
}
