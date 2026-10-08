//! Persistent, per-Instance Release editing workspace.
//! Selecting a Release never changes live Runtime state.

use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::CliError;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct Context {
    schema_version: u32,
    base_release: String,
    source: String,
}

pub(super) fn run(root: Option<&str>, selected: Option<&str>) -> Result<(), CliError> {
    let root = match root {
        Some(root) => PathBuf::from(root),
        None => env::var("MANAFIELD_INSTANCE_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(".")),
    };
    match selected {
        Some(release) => select(&root, release),
        None => show(&root),
    }
}

fn valid_release_id(id: &str) -> bool {
    // v<positive-integer>_YYYYMMDDTHHMMSSZ, no path separators or traversal.
    let Some((number, timestamp)) = id.strip_prefix('v').and_then(|x| x.split_once('_')) else {
        return false;
    };
    !number.starts_with('0')
        && !number.is_empty()
        && number.bytes().all(|x| x.is_ascii_digit())
        && timestamp.len() == 16
        && timestamp.as_bytes()[8] == b'T'
        && timestamp.as_bytes()[15] == b'Z'
        && timestamp
            .bytes()
            .enumerate()
            .all(|(i, ch)| i == 8 || i == 15 || ch.is_ascii_digit())
}

fn select(root: &Path, release: &str) -> Result<(), CliError> {
    if !valid_release_id(release) {
        return Err(CliError::Usage(format!(
            "invalid Release ID '{release}' (expected vN_YYYYMMDDTHHMMSSZ)"
        )));
    }
    let base = root.join("manafield");
    let release_path = base.join("release").join(format!("{release}.yaml"));
    let yaml = fs::read(&release_path).map_err(|e| {
        CliError::Execution(format!(
            "cannot read Release '{}': {e}",
            release_path.display()
        ))
    })?;
    let parsed: serde_yaml_ng::Value = serde_yaml_ng::from_slice(&yaml).map_err(|e| {
        CliError::Execution(format!(
            "Release '{}' has invalid YAML: {e}",
            release_path.display()
        ))
    })?;
    if !parsed.is_mapping() || parsed.get("instance").is_none() {
        return Err(CliError::Execution(format!(
            "Release '{}' is not an Instance Definition (missing 'instance')",
            release_path.display()
        )));
    }

    let tmp = base.join("temp");
    fs::create_dir_all(&tmp)
        .map_err(|e| CliError::Execution(format!("cannot create '{}': {e}", tmp.display())))?;
    let working = tmp.join("working.yaml");
    let context = tmp.join("context.json");

    // Avoid following symlinks when replacing a working copy or its context.
    for target in [&working, &context] {
        if fs::symlink_metadata(target).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return Err(CliError::Execution(format!(
                "refusing to replace symlink '{}'",
                target.display()
            )));
        }
    }

    let info = Context {
        schema_version: 1,
        base_release: release.to_owned(),
        source: "release".to_owned(),
    };
    let json = serde_json::to_vec_pretty(&info)
        .map_err(|e| CliError::Execution(format!("cannot encode workspace state: {e}")))?;
    // Both files use temp + rename so readers never observe partial file contents.
    // A future multi-file transaction protocol can cover crashes between the two renames.
    let staged_yaml = stage_file(&working, &yaml)?;
    let staged_context = match stage_file(&context, &json) {
        Ok(path) => path,
        Err(error) => {
            let _ = fs::remove_file(staged_yaml);
            return Err(error);
        }
    };
    if let Err(e) = fs::rename(&staged_yaml, &working) {
        let _ = fs::remove_file(&staged_yaml);
        let _ = fs::remove_file(&staged_context);
        return Err(CliError::Execution(format!(
            "cannot replace '{}': {e}",
            working.display()
        )));
    }
    if let Err(e) = fs::rename(&staged_context, &context) {
        let _ = fs::remove_file(staged_context);
        return Err(CliError::Execution(format!(
            "working YAML written but cannot update '{}': {e}; run 'manafield use {release}' again",
            context.display()
        )));
    }

    println!("Selected Release {release} for editing.");
    println!("Working copy: {}", working.display());
    println!("Previous unsaved working changes, if any, were discarded.");
    println!("Running Instance was not modified.");
    Ok(())
}

fn show(root: &Path) -> Result<(), CliError> {
    let tmp = root.join("manafield").join("temp");
    let context_path = tmp.join("context.json");
    let data = match fs::read(&context_path) {
        Ok(data) => data,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            println!("No Release selected for editing.");
            return Ok(());
        }
        Err(e) => {
            return Err(CliError::Execution(format!(
                "cannot read '{}': {e}",
                context_path.display()
            )));
        }
    };
    let context: Context = serde_json::from_slice(&data)
        .map_err(|e| CliError::Execution(format!("invalid workspace context: {e}")))?;
    if context.schema_version != 1 || !valid_release_id(&context.base_release) {
        return Err(CliError::Execution(
            "workspace context has an unsupported format".to_owned(),
        ));
    }
    let source = root
        .join("manafield")
        .join("release")
        .join(format!("{}.yaml", context.base_release));
    let working = tmp.join("working.yaml");
    let original = fs::read(source).map_err(|e| {
        CliError::Execution(format!("cannot read working copy's base Release: {e}"))
    })?;
    let edited = fs::read(&working).map_err(|e| {
        CliError::Execution(format!(
            "cannot read working copy '{}': {e}",
            working.display()
        ))
    })?;
    println!("Selected Release: {}", context.base_release);
    println!("Working copy: {}", working.display());
    println!(
        "Unsaved changes: {}",
        if original == edited { "no" } else { "yes" }
    );
    Ok(())
}

fn stage_file(destination: &Path, content: &[u8]) -> Result<PathBuf, CliError> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| CliError::Execution(e.to_string()))?
        .as_nanos();
    let staged = destination.with_extension(format!("tmp.{}.{}", std::process::id(), timestamp));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staged)
        .map_err(|e| CliError::Execution(format!("cannot create '{}': {e}", staged.display())))?;
    file.write_all(content)
        .and_then(|_| file.sync_all())
        .map_err(|e| {
            let _ = fs::remove_file(&staged);
            CliError::Execution(format!("cannot stage '{}': {e}", staged.display()))
        })?;
    Ok(staged)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture() -> PathBuf {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = env::temp_dir().join(format!("manafield-use-{}-{n}", std::process::id()));
        fs::create_dir_all(root.join("manafield/release")).unwrap();
        root
    }

    fn release(root: &Path, id: &str, module: &str) {
        fs::write(
            root.join("manafield/release").join(format!("{id}.yaml")),
            format!("version: 0\ninstance:\n  id: demo\nmodules:\n  - id: {module}\n"),
        )
        .unwrap();
    }

    #[test]
    fn release_ids_are_path_safe() {
        assert!(valid_release_id("v1_20261008T070000Z"));
        assert!(valid_release_id("v123_20261008T070000Z"));
        for invalid in [
            "A",
            "v0_20261008T070000Z",
            "v01_20261008T070000Z",
            "../release",
            "v1_20261008T070000Z/../../foo",
            "v1_20261008T07000Z",
        ] {
            assert!(!valid_release_id(invalid), "{invalid}");
        }
    }

    #[test]
    fn use_replaces_unsaved_edits_and_persists_selected_release() {
        let root = fixture();
        let a = "v1_20261008T070000Z";
        let b = "v2_20261008T080000Z";
        release(&root, a, "echo");
        release(&root, b, "account");
        select(&root, a).unwrap();
        let working = root.join("manafield/temp/working.yaml");
        fs::write(&working, "UNSAVED CONTENT").unwrap();
        select(&root, b).unwrap();
        assert!(fs::read_to_string(&working).unwrap().contains("account"));
        assert!(!fs::read_to_string(&working).unwrap().contains("UNSAVED"));
        let context: Context =
            serde_json::from_slice(&fs::read(root.join("manafield/temp/context.json")).unwrap())
                .unwrap();
        assert_eq!(context.base_release, b);
        show(&root).unwrap();
        assert!(
            root.join("manafield/release")
                .join(format!("{a}.yaml"))
                .exists()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_or_invalid_release_never_overwrites_workspace() {
        let root = fixture();
        let a = "v1_20261008T070000Z";
        release(&root, a, "echo");
        select(&root, a).unwrap();
        let working = root.join("manafield/temp/working.yaml");
        fs::write(&working, "UNSAVED CONTENT").unwrap();
        assert!(select(&root, "v9_20261008T070000Z").is_err());
        assert!(select(&root, "../evil").is_err());
        assert_eq!(fs::read_to_string(&working).unwrap(), "UNSAVED CONTENT");
        fs::remove_dir_all(root).unwrap();
    }
}
