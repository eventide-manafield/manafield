//! Immutable Instance Release snapshots and safe staged-artifact deployment.
//! v0 bridge: Source/artifact materialization still happens outside this command.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use super::{CliError, workspace};

pub(super) struct Options {
    pub id: Option<String>,
    pub instance_root: Option<String>,
    pub staged: Option<String>,
    pub snapshot_only: bool,
}

#[derive(Serialize)]
struct Attempt<'a> {
    release_id: &'a str,
    status: &'a str,
    detail: &'a str,
}

pub(super) fn run(options: Options) -> Result<(), CliError> {
    let root = workspace::instance_root(options.instance_root.as_deref());
    let id = match options.id {
        Some(id) => id,
        None => next_id(&root)?,
    };
    if !workspace::valid_release_id(&id) {
        return Err(CliError::Usage(format!("invalid Release ID '{id}'")));
    }
    let snapshot = root.join("manafield/release").join(format!("{id}.yaml"));
    let existing = snapshot.exists();

    // A selected working copy may contain uncommitted edits. Reapplying an
    // old Release must not quietly ignore the user's changes.
    if existing {
        if workspace::has_dirty_working_copy(&root)? {
            return Err(CliError::Execution(
                "unsaved working-copy edits exist; use another new Release ID or 'manafield use <id>' to discard them".into()
            ));
        }
        crate::build_plan::plan_value(&snapshot)
            .map_err(|e| CliError::Execution(format!("saved Release is invalid: {e}")))?;
    } else {
        let (yaml, _, _) = workspace::get_workspace(&root)?;
        let text = std::str::from_utf8(&yaml)
            .map_err(|e| CliError::Execution(format!("working YAML must be UTF-8: {e}")))?;
        crate::build_plan::validate_yaml(text).map_err(|e| {
            CliError::Execution(format!("working Instance Definition is invalid: {e}"))
        })?;
        save_immutable(&snapshot, &yaml)?;
        // Use the saved snapshot as the new clean editing baseline. Even if
        // runtime application fails, retrying the same ID remains possible.
        workspace::adopt_snapshot(&root, &id)?;
        println!("Saved immutable Release: {}", snapshot.display());
    }

    if options.snapshot_only {
        println!("Snapshot-only: no Docker command was run.");
        return Ok(());
    }
    let Some(staged_dir) = options.staged else {
        let detail = "no prepared deployment artifacts supplied; retry with --staged-dir DIR after staging this Release";
        record_attempt(&root, &id, "not_applied", detail)?;
        return Err(CliError::Execution(format!(
            "Release '{id}' is saved but not deployed: {detail}"
        )));
    };
    let staged_dir = PathBuf::from(staged_dir);
    if let Err(error) = validate_staged(&snapshot, &staged_dir) {
        record_attempt(
            &root,
            &id,
            "not_applied",
            "staged artifacts missing or do not match Release",
        )?;
        return Err(error);
    }

    if let Err(error) = apply_staged(&staged_dir) {
        record_attempt(&root, &id, "failed", &error.to_string())?;
        return Err(error);
    }
    // 'active' only after Compose success and Core HTTP health verification.
    // Module/Resource readiness is still owned by the full verification path.
    write_active(&root, &id)?;
    record_attempt(
        &root,
        &id,
        "core_healthy",
        "Compose applied and Core HTTP health passed; Module/Resource verification remains separate",
    )?;
    println!("Applied Release {id} (Core healthy; full Module/Resource verification pending).");
    Ok(())
}

fn next_id(root: &Path) -> Result<String, CliError> {
    let dir = root.join("manafield/release");
    let max = match fs::read_dir(&dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter_map(|e| {
                let filename = e.file_name().to_string_lossy().to_string();
                let id = filename.strip_suffix(".yaml")?;
                if !workspace::valid_release_id(id) {
                    return None;
                }
                id.strip_prefix('v')?.split('_').next()?.parse::<u64>().ok()
            })
            .max()
            .unwrap_or(0),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
        Err(e) => return Err(CliError::Execution(format!("cannot list Releases: {e}"))),
    };
    let number = max
        .checked_add(1)
        .ok_or_else(|| CliError::Execution("Release sequence overflow".into()))?;
    let result = ProcessCommand::new("date")
        .args(["-u", "+%Y%m%dT%H%M%SZ"])
        .output()
        .map_err(|e| CliError::Execution(format!("cannot get UTC timestamp: {e}")))?;
    if !result.status.success() {
        return Err(CliError::Execution("date command failed".into()));
    }
    let stamp = String::from_utf8(result.stdout)
        .map_err(|e| CliError::Execution(format!("invalid timestamp: {e}")))?;
    let id = format!("v{number}_{}", stamp.trim());
    if !workspace::valid_release_id(&id) {
        return Err(CliError::Execution("invalid system timestamp".into()));
    }
    Ok(id)
}

fn save_immutable(path: &Path, bytes: &[u8]) -> Result<(), CliError> {
    let parent = path
        .parent()
        .ok_or_else(|| CliError::Execution("invalid Release path".into()))?;
    fs::create_dir_all(parent)
        .map_err(|e| CliError::Execution(format!("cannot create Release directory: {e}")))?;
    if path.exists() {
        return Err(CliError::Execution(format!(
            "Release '{}' already exists",
            path.display()
        )));
    }
    // Fully write and sync a temp file, then atomically publish a hard link.
    // Unlike rename, hard_link never overwrites an existing Release.
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| CliError::Execution(e.to_string()))?
        .as_nanos();
    let temporary = path.with_extension(format!("stage.{}.{nanos}", std::process::id()));
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| CliError::Execution(format!("cannot stage immutable Release: {e}")))?;
    if let Err(e) = f.write_all(bytes).and_then(|_| f.sync_all()) {
        let _ = fs::remove_file(&temporary);
        return Err(CliError::Execution(format!("cannot write Release: {e}")));
    }
    drop(f);
    let result = fs::hard_link(&temporary, path);
    let _ = fs::remove_file(&temporary);
    result.map_err(|e| {
        CliError::Execution(format!(
            "cannot atomically publish immutable Release '{}': {e}",
            path.display()
        ))
    })?;
    Ok(())
}

fn validate_staged(release: &Path, staged: &Path) -> Result<(), CliError> {
    for name in [
        "release.env",
        "compose.yml",
        "modules.compose.yml",
        "compose-profiles.txt",
        "build-plan.json",
    ] {
        if !staged.join(name).is_file() {
            return Err(CliError::Execution(format!(
                "staged artifact '{}' is missing",
                staged.join(name).display()
            )));
        }
    }
    let expected = crate::build_plan::plan_value(release)
        .map_err(|e| CliError::Execution(format!("cannot resolve saved Release: {e}")))?;
    let data = fs::read(staged.join("build-plan.json"))
        .map_err(|e| CliError::Execution(format!("cannot read staged Build Plan: {e}")))?;
    let actual: serde_json::Value = serde_json::from_slice(&data)
        .map_err(|e| CliError::Execution(format!("invalid staged Build Plan JSON: {e}")))?;
    if expected != actual {
        return Err(CliError::Execution(
            "prepared Build Plan differs from the immutable Release; regenerate artifacts before deployment".into(),
        ));
    }
    Ok(())
}

fn apply_staged(staged: &Path) -> Result<(), CliError> {
    apply_staged_with(staged, |command, args| {
        let status = command
            .status()
            .map_err(|e| CliError::Execution(format!("failed to run Docker Compose: {e}")))?;
        if !status.success() {
            return Err(CliError::Execution(format!(
                "Docker Compose '{}' failed with {status}",
                args.join(" ")
            )));
        }
        Ok(())
    })
}

fn apply_staged_with<F>(staged: &Path, mut executor: F) -> Result<(), CliError>
where
    F: FnMut(&mut ProcessCommand, &[&str]) -> Result<(), CliError>,
{
    let profiles = fs::read_to_string(staged.join("compose-profiles.txt"))
        .map_err(|e| CliError::Execution(format!("cannot read Compose profiles: {e}")))?;
    for args in [
        &["up", "-d", "--no-build"][..],
        &["ps"][..],
        &[
            "exec",
            "-T",
            "core",
            "curl",
            "--fail",
            "--silent",
            "--show-error",
            "http://127.0.0.1:8080/health",
        ][..],
    ] {
        let mut command = ProcessCommand::new("docker");
        command
            .arg("compose")
            .arg("--env-file")
            .arg(staged.join("release.env"))
            .arg("--file")
            .arg(staged.join("compose.yml"))
            .arg("--file")
            .arg(staged.join("modules.compose.yml"));
        if profiles.trim().is_empty() {
            command.env_remove("COMPOSE_PROFILES");
        } else {
            command.env("COMPOSE_PROFILES", profiles.trim());
        }
        command.args(args);
        executor(&mut command, args)?;
    }
    // Intentionally NO --remove-orphans. Resource removal needs explicit policy.
    Ok(())
}

fn write_active(root: &Path, id: &str) -> Result<(), CliError> {
    if !workspace::valid_release_id(id) {
        return Err(CliError::Usage("invalid active Release ID".into()));
    }
    let path = root.join("manafield/state/active-release.json");
    let bytes = serde_json::to_vec_pretty(&ActiveStateWrite { release_id: id })
        .map_err(|e| CliError::Execution(e.to_string()))?;
    replace_state(&path, &bytes)
}

#[derive(Serialize)]
struct ActiveStateWrite<'a> {
    release_id: &'a str,
}

fn record_attempt(root: &Path, id: &str, status: &str, detail: &str) -> Result<(), CliError> {
    let path = root.join("manafield/state/last-attempt.json");
    let bytes = serde_json::to_vec_pretty(&Attempt {
        release_id: id,
        status,
        detail,
    })
    .map_err(|e| CliError::Execution(e.to_string()))?;
    replace_state(&path, &bytes)
}

fn replace_state(dest: &Path, data: &[u8]) -> Result<(), CliError> {
    let parent = dest
        .parent()
        .ok_or_else(|| CliError::Execution("invalid state path".into()))?;
    fs::create_dir_all(parent).map_err(|e| CliError::Execution(e.to_string()))?;
    if fs::symlink_metadata(dest).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(CliError::Execution(format!(
            "refusing to replace symlink '{}'",
            dest.display()
        )));
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| CliError::Execution(e.to_string()))?
        .as_nanos();
    let path = dest.with_extension(format!("tmp.{}.{nanos}", std::process::id()));
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| CliError::Execution(e.to_string()))?;
    f.write_all(data).and_then(|_| f.sync_all()).map_err(|e| {
        let _ = fs::remove_file(&path);
        CliError::Execution(e.to_string())
    })?;
    fs::rename(&path, dest).map_err(|e| CliError::Execution(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn temp_fixture() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = env::temp_dir().join(format!(
            "manafield-release-tests-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn write_instance(root: &Path) {
        fs::write(
            root.join("instance.yaml"),
            include_str!("../../deploy/instance.bootstrap.yaml"),
        )
        .unwrap();
    }

    fn options(root: &Path, id: &str, snapshot_only: bool) -> Options {
        Options {
            id: Some(id.into()),
            instance_root: Some(root.to_str().unwrap().into()),
            staged: None,
            snapshot_only,
        }
    }

    #[test]
    fn snapshots_are_immutable_and_discard_dirty_conflicts() {
        let root = temp_fixture();
        write_instance(&root);
        let id = "v1_20261008T070000Z";
        run(options(&root, id, true)).unwrap();
        let snap = root.join("manafield/release").join(format!("{id}.yaml"));
        assert_eq!(
            fs::read(&snap).unwrap(),
            fs::read(root.join("instance.yaml")).unwrap()
        );
        // Reapplying clean snapshot does not alter it.
        run(options(&root, id, true)).unwrap();
        let working = root.join("manafield/temp/working.yaml");
        fs::write(&working, "UNSAVED CHANGES").unwrap();
        assert!(run(options(&root, id, true)).is_err());
        assert!(
            fs::read_to_string(&snap)
                .unwrap()
                .contains("manafield-bootstrap")
        );
        assert!(save_immutable(&snap, b"OVERWRITE").is_err());
        assert!(!root.join("manafield/state/active-release.json").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_staged_artifacts_preserves_snapshot_and_inactive_state() {
        let root = temp_fixture();
        write_instance(&root);
        let id = "v1_20261008T070000Z";
        let result = run(options(&root, id, false));
        assert!(result.is_err());
        assert!(
            root.join("manafield/release")
                .join(format!("{id}.yaml"))
                .is_file()
        );
        assert!(!root.join("manafield/state/active-release.json").exists());
        let log = fs::read_to_string(root.join("manafield/state/last-attempt.json")).unwrap();
        assert!(log.contains("not_applied"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn refuses_mismatching_staged_build_plan() {
        let root = temp_fixture();
        write_instance(&root);
        let id = "v2_20261008T070000Z";
        run(options(&root, id, true)).unwrap();
        let staged = root.join("staged");
        fs::create_dir_all(&staged).unwrap();
        for name in [
            "release.env",
            "compose.yml",
            "modules.compose.yml",
            "compose-profiles.txt",
        ] {
            fs::write(staged.join(name), "").unwrap();
        }
        fs::write(
            staged.join("build-plan.json"),
            r#"{"instanceId":"another-instance"}"#,
        )
        .unwrap();
        let snap = root.join("manafield/release").join(format!("{id}.yaml"));
        let error = validate_staged(&snap, &staged).unwrap_err().to_string();
        assert!(error.contains("differs"), "{error}");
        let expected = crate::build_plan::plan_value(&snap).unwrap();
        fs::write(
            staged.join("build-plan.json"),
            serde_json::to_vec(&expected).unwrap(),
        )
        .unwrap();
        validate_staged(&snap, &staged).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_definition_does_not_leave_a_release() {
        let root = temp_fixture();
        fs::write(root.join("instance.yaml"), "instance:\n  id: incomplete\n").unwrap();
        let id = "v1_20261008T070000Z";
        assert!(run(options(&root, id, true)).is_err());
        assert!(
            !root
                .join("manafield/release")
                .join(format!("{id}.yaml"))
                .exists()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn compose_application_never_uses_remove_orphans() {
        let root = temp_fixture();
        let staged = root.join("staged");
        fs::create_dir_all(&staged).unwrap();
        for name in [
            "release.env",
            "compose.yml",
            "modules.compose.yml",
            "compose-profiles.txt",
        ] {
            fs::write(staged.join(name), "").unwrap();
        }
        let mut invocations = Vec::new();
        apply_staged_with(&staged, |command, args| {
            assert_eq!(command.get_program(), "docker");
            let vec = command
                .get_args()
                .map(|x| x.to_string_lossy().into_owned())
                .collect::<Vec<_>>();
            assert!(!vec.contains(&"--remove-orphans".into()));
            invocations.push(args.join(" "));
            Ok(())
        })
        .unwrap();
        assert_eq!(
            invocations,
            vec![
                "up -d --no-build",
                "ps",
                "exec -T core curl --fail --silent --show-error http://127.0.0.1:8080/health"
            ]
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn release_number_increment_is_monotonic() {
        let root = env::temp_dir().join(format!("manafield-deploy-unit-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("manafield/release")).unwrap();
        fs::write(
            root.join("manafield/release/v4_20261008T070000Z.yaml"),
            "test",
        )
        .unwrap();
        assert!(next_id(&root).unwrap().starts_with("v5_"));
        fs::remove_dir_all(&root).unwrap();
    }
}
