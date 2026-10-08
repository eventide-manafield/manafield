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
    let root = instance_root(root);
    match selected {
        Some(release) => select(&root, release),
        None => show(&root),
    }
}

fn instance_root(root: Option<&str>) -> PathBuf {
    match root {
        Some(root) => PathBuf::from(root),
        None => env::var("MANAFIELD_INSTANCE_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(".")),
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
    let temp = root.join("manafield/temp");
    let context_path = temp.join("context.json");
    let working = temp.join("working.yaml");
    let context_exists = context_path.exists();
    let working_exists = working.exists();
    if !context_exists && !working_exists {
        println!("No Release selected for editing.");
        return Ok(());
    }
    if context_exists != working_exists {
        return Err(CliError::Execution(
            "workspace is inconsistent (working.yaml and context.json must both exist)".to_owned(),
        ));
    }
    let context = load_context(&context_path)?;
    let source = source_path(root, &context)?;
    let original = fs::read(&source).map_err(|e| {
        CliError::Execution(format!(
            "cannot read working copy's source '{}': {e}",
            source.display()
        ))
    })?;
    let edited = fs::read(&working).map_err(|e| {
        CliError::Execution(format!(
            "cannot read working copy '{}': {e}",
            working.display()
        ))
    })?;
    if context.source == "release" {
        println!("Selected Release: {}", context.base_release);
    } else {
        println!("Selected source: instance.yaml");
    }
    println!("Working copy: {}", working.display());
    println!(
        "Unsaved changes: {}",
        if original == edited { "no" } else { "yes" }
    );
    Ok(())
}

fn load_context(path: &Path) -> Result<Context, CliError> {
    let data = fs::read(path).map_err(|e| {
        CliError::Execution(format!(
            "cannot read workspace context '{}': {e}",
            path.display()
        ))
    })?;
    let context: Context = serde_json::from_slice(&data)
        .map_err(|e| CliError::Execution(format!("invalid workspace context: {e}")))?;
    if context.schema_version != 1
        || !((context.source == "release" && valid_release_id(&context.base_release))
            || (context.source == "instance" && context.base_release.is_empty()))
    {
        return Err(CliError::Execution(
            "workspace context has an unsupported format".to_owned(),
        ));
    }
    Ok(context)
}

fn source_path(root: &Path, context: &Context) -> Result<PathBuf, CliError> {
    match context.source.as_str() {
        "release" if valid_release_id(&context.base_release) => Ok(root
            .join("manafield/release")
            .join(format!("{}.yaml", context.base_release))),
        "instance" if context.base_release.is_empty() => Ok(root.join("instance.yaml")),
        _ => Err(CliError::Execution(
            "workspace context has an unsupported source".to_owned(),
        )),
    }
}

fn yaml_value(raw: &[u8]) -> Result<serde_yaml_ng::Value, CliError> {
    let value: serde_yaml_ng::Value = serde_yaml_ng::from_slice(raw)
        .map_err(|e| CliError::Execution(format!("invalid Instance YAML: {e}")))?;
    if !value.is_mapping() || !value.get("instance").is_some_and(|v| v.is_mapping()) {
        return Err(CliError::Execution(
            "Instance YAML must contain an 'instance' mapping".to_owned(),
        ));
    }
    Ok(value)
}

fn get_workspace(root: &Path) -> Result<(Vec<u8>, Context, bool), CliError> {
    let temp = root.join("manafield/temp");
    let working = temp.join("working.yaml");
    let context_file = temp.join("context.json");
    reject_symlinks(&[&working, &context_file])?;
    let existing_yaml = fs::metadata(&working).map(|m| m.is_file()).unwrap_or(false);
    let existing_context = fs::metadata(&context_file)
        .map(|m| m.is_file())
        .unwrap_or(false);
    if existing_yaml != existing_context {
        return Err(CliError::Execution(
            "workspace is inconsistent (working.yaml and context.json must both exist)".to_owned(),
        ));
    }
    if existing_yaml {
        let context = load_context(&context_file)?;
        let raw = fs::read(&working).map_err(|e| {
            CliError::Execution(format!("cannot read '{}': {e}", working.display()))
        })?;
        return Ok((raw, context, true));
    }

    let active = root.join("manafield/state/active-release.json");
    let context = if active.exists() {
        #[derive(Deserialize)]
        struct Active {
            release_id: String,
        }
        let data = fs::read(&active)
            .map_err(|e| CliError::Execution(format!("cannot read active Release state: {e}")))?;
        let state: Active = serde_json::from_slice(&data)
            .map_err(|e| CliError::Execution(format!("invalid active Release state: {e}")))?;
        if !valid_release_id(&state.release_id) {
            return Err(CliError::Execution(
                "active Release ID is invalid".to_owned(),
            ));
        }
        Context {
            schema_version: 1,
            base_release: state.release_id,
            source: "release".to_owned(),
        }
    } else if root.join("instance.yaml").is_file() {
        Context {
            schema_version: 1,
            base_release: String::new(),
            source: "instance".to_owned(),
        }
    } else {
        return Err(CliError::Execution(
            "no working copy, active Release, or instance.yaml found; initialize an Instance first"
                .to_owned(),
        ));
    };
    let path = source_path(root, &context)?;
    let raw = fs::read(&path).map_err(|e| {
        CliError::Execution(format!(
            "cannot read Instance source '{}': {e}",
            path.display()
        ))
    })?;
    Ok((raw, context, false))
}

fn reject_symlinks(paths: &[&Path]) -> Result<(), CliError> {
    for path in paths {
        match fs::symlink_metadata(path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(CliError::Execution(format!(
                    "refusing to modify symlink '{}'",
                    path.display()
                )));
            }
            Ok(meta) if !meta.is_file() => {
                return Err(CliError::Execution(format!(
                    "expected regular file at '{}'",
                    path.display()
                )));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(CliError::Execution(format!(
                    "cannot inspect '{}': {e}",
                    path.display()
                )));
            }
        }
    }
    Ok(())
}

fn commit_working(
    root: &Path,
    yaml: &[u8],
    context: &Context,
    existing: bool,
) -> Result<(), CliError> {
    let dir = root.join("manafield/temp");
    fs::create_dir_all(&dir)
        .map_err(|e| CliError::Execution(format!("cannot create '{}': {e}", dir.display())))?;
    let working = dir.join("working.yaml");
    let context_file = dir.join("context.json");
    reject_symlinks(&[&working, &context_file])?;
    let encoded = serde_json::to_vec_pretty(context)
        .map_err(|e| CliError::Execution(format!("cannot encode context: {e}")))?;
    // Stage both files before modifying either destination.
    let staged = stage_file(&working, yaml)?;
    let context_stage = if existing {
        None
    } else {
        match stage_file(&context_file, &encoded) {
            Ok(value) => Some(value),
            Err(e) => {
                let _ = fs::remove_file(&staged);
                return Err(e);
            }
        }
    };
    if let Err(e) = fs::rename(&staged, &working) {
        let _ = fs::remove_file(staged);
        if let Some(path) = context_stage {
            let _ = fs::remove_file(path);
        }
        return Err(CliError::Execution(format!(
            "cannot save working YAML: {e}"
        )));
    }
    if let Some(path) = context_stage {
        fs::rename(&path, &context_file).map_err(|e| {
            CliError::Execution(format!("working YAML saved but context update failed: {e}"))
        })?;
    }
    Ok(())
}

fn valid_slot(slot: &str) -> bool {
    let mut chars = slot.bytes();
    matches!(chars.next(), Some(b'A'..=b'Z' | b'a'..=b'z' | b'_'))
        && chars.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

pub(super) fn bind(
    root: Option<&str>,
    consumer: &str,
    slot: &str,
    target: &str,
) -> Result<(), CliError> {
    if consumer.is_empty() || target.is_empty() || !valid_slot(slot) {
        return Err(CliError::Usage(
            "module bind requires a non-empty consumer/target and slot matching [A-Za-z_][A-Za-z0-9_]*".to_owned()
        ));
    }
    let root = instance_root(root);
    let (raw, context, existing) = get_workspace(&root)?;
    let mut yaml = yaml_value(&raw)?;
    let modules = yaml
        .get("modules")
        .and_then(serde_yaml_ng::Value::as_sequence)
        .ok_or_else(|| CliError::Execution("Instance YAML has no modules list".to_owned()))?;

    // Check duplicate consumer IDs and unique enabled target across Module and Resource.
    let mut matches = 0;
    let mut targets = 0;
    for entry in modules {
        let id = entry.get("id").and_then(serde_yaml_ng::Value::as_str);
        if id == Some(consumer) {
            matches += 1;
        }
        if id == Some(target)
            && entry.get("enabled").and_then(serde_yaml_ng::Value::as_bool) != Some(false)
        {
            targets += 1;
        }
    }
    if let Some(resources) = yaml
        .get("resources")
        .and_then(serde_yaml_ng::Value::as_sequence)
    {
        for entry in resources {
            if entry.get("id").and_then(serde_yaml_ng::Value::as_str) == Some(target)
                && entry.get("enabled").and_then(serde_yaml_ng::Value::as_bool) != Some(false)
            {
                targets += 1;
            }
        }
    }
    if matches != 1 {
        return Err(CliError::Execution(format!(
            "consumer Module '{consumer}' must exist exactly once (found {matches})"
        )));
    }
    if targets != 1 {
        return Err(CliError::Execution(format!(
            "target Instance '{target}' must exist exactly once and be enabled (found {targets})"
        )));
    }

    let modules_mut = yaml
        .get_mut("modules")
        .and_then(serde_yaml_ng::Value::as_sequence_mut)
        .ok_or_else(|| CliError::Execution("Instance YAML has no modules list".to_owned()))?;
    let module = modules_mut
        .iter_mut()
        .find(|entry| entry.get("id").and_then(serde_yaml_ng::Value::as_str) == Some(consumer))
        .ok_or_else(|| CliError::Execution("consumer Module not found".to_owned()))?;

    let key = serde_yaml_ng::Value::String("bindings".to_owned());
    let module_map = module.as_mapping_mut().ok_or_else(|| {
        CliError::Execution("consumer Module entry is not a YAML mapping".to_owned())
    })?;
    if !module_map.contains_key(&key) {
        module_map.insert(
            key.clone(),
            serde_yaml_ng::Value::Mapping(Default::default()),
        );
    }
    let binding_map = module_map
        .get_mut(&key)
        .and_then(serde_yaml_ng::Value::as_mapping_mut)
        .ok_or_else(|| {
            CliError::Execution("consumer Module 'bindings' must be a mapping".to_owned())
        })?;
    if binding_map
        .get(serde_yaml_ng::Value::String(slot.to_owned()))
        .and_then(serde_yaml_ng::Value::as_str)
        == Some(target)
    {
        println!("Binding {consumer}.{slot} already targets {target}; no change.");
        return Ok(());
    }
    binding_map.insert(
        serde_yaml_ng::Value::String(slot.to_owned()),
        serde_yaml_ng::Value::String(target.to_owned()),
    );
    let updated = serde_yaml_ng::to_string(&yaml)
        .map_err(|e| CliError::Execution(format!("failed to serialize working YAML: {e}")))?;
    commit_working(&root, updated.as_bytes(), &context, existing)?;
    println!("Bound {consumer}.{slot} -> {target} in the working copy.");
    println!("No runtime changes were applied.");
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
    fn example_yaml() -> String {
        r#"version: 0
instance:
  id: demo
core:
  source:
    repository: https://example.test/manafield.git
    ref: main
modules:
  - id: echo
    source:
      type: dir
    build:
      type: docker
      context: .
      dockerfile: Dockerfile
  - id: account
    source:
      type: dir
    build:
      type: docker
      context: .
      dockerfile: Dockerfile
resources:
  - id: db
    provider: postgresql
deployment:
  modulesNetwork: modules
  edgeNetwork: edge
"#
        .to_owned()
    }

    fn bound_target(root: &Path, slot: &str) -> Option<String> {
        let raw = fs::read(root.join("manafield/temp/working.yaml")).unwrap();
        let yaml = yaml_value(&raw).unwrap();
        let modules = yaml.get("modules").unwrap().as_sequence().unwrap();
        let echo = modules
            .iter()
            .find(|item| item.get("id").and_then(serde_yaml_ng::Value::as_str) == Some("echo"))
            .unwrap();
        echo.get("bindings")?.get(slot)?.as_str().map(str::to_owned)
    }

    #[test]
    fn bind_without_use_starts_from_instance_yaml_and_persists() {
        let root = fixture();
        fs::write(root.join("instance.yaml"), example_yaml()).unwrap();
        bind(Some(root.to_str().unwrap()), "echo", "state", "db").unwrap();
        assert_eq!(bound_target(&root, "state").as_deref(), Some("db"));
        bind(Some(root.to_str().unwrap()), "echo", "identity", "account").unwrap();
        assert_eq!(bound_target(&root, "state").as_deref(), Some("db"));
        assert_eq!(bound_target(&root, "identity").as_deref(), Some("account"));
        let c = load_context(&root.join("manafield/temp/context.json")).unwrap();
        assert_eq!(c.source, "instance");
        assert_eq!(c.base_release, "");
        assert!(
            !fs::read_to_string(root.join("instance.yaml"))
                .unwrap()
                .contains("bindings:")
        );
        show(&root).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn bind_without_use_prefers_active_release() {
        let root = fixture();
        fs::write(root.join("instance.yaml"), "instance:\n  id: wrong\n").unwrap();
        let id = "v3_20261008T080000Z";
        fs::write(
            root.join("manafield/release").join(format!("{id}.yaml")),
            example_yaml(),
        )
        .unwrap();
        fs::create_dir_all(root.join("manafield/state")).unwrap();
        fs::write(
            root.join("manafield/state/active-release.json"),
            format!(r#"{{"release_id":"{id}"}}"#),
        )
        .unwrap();
        bind(Some(root.to_str().unwrap()), "echo", "state", "db").unwrap();
        assert_eq!(bound_target(&root, "state").as_deref(), Some("db"));
        let c = load_context(&root.join("manafield/temp/context.json")).unwrap();
        assert_eq!(c.base_release, id);
        assert_eq!(c.source, "release");
        assert!(
            !fs::read_to_string(root.join("manafield/release").join(format!("{id}.yaml")))
                .unwrap()
                .contains("bindings:")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn existing_working_copy_takes_priority_and_use_resets_it() {
        let root = fixture();
        let id = "v4_20261008T080000Z";
        fs::write(
            root.join("manafield/release").join(format!("{id}.yaml")),
            example_yaml(),
        )
        .unwrap();
        select(&root, id).unwrap();
        bind(Some(root.to_str().unwrap()), "echo", "state", "db").unwrap();
        assert_eq!(bound_target(&root, "state").as_deref(), Some("db"));
        select(&root, id).unwrap();
        assert_eq!(bound_target(&root, "state"), None);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_bind_never_initializes_working_copy() {
        let root = fixture();
        fs::write(root.join("instance.yaml"), example_yaml()).unwrap();
        for (consumer, slot, target) in [
            ("unknown", "state", "db"),
            ("echo", "bad-slot", "db"),
            ("echo", "state", "unknown"),
        ] {
            assert!(bind(Some(root.to_str().unwrap()), consumer, slot, target).is_err());
            assert!(!root.join("manafield/temp/working.yaml").exists());
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn no_sources_and_partial_working_copy_are_errors() {
        let root = fixture();
        assert!(bind(Some(root.to_str().unwrap()), "echo", "state", "db").is_err());
        let temp = root.join("manafield/temp");
        fs::create_dir_all(&temp).unwrap();
        fs::write(temp.join("working.yaml"), example_yaml()).unwrap();
        assert!(bind(Some(root.to_str().unwrap()), "echo", "state", "db").is_err());
        assert!(show(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn disabled_target_and_bad_active_state_are_rejected() {
        let root = fixture();
        let yaml = example_yaml().replace(
            "  - id: db\n    provider:",
            "  - id: db\n    enabled: false\n    provider:",
        );
        fs::write(root.join("instance.yaml"), yaml).unwrap();
        assert!(bind(Some(root.to_str().unwrap()), "echo", "state", "db").is_err());
        assert!(!root.join("manafield/temp/working.yaml").exists());
        fs::create_dir_all(root.join("manafield/state")).unwrap();
        fs::write(
            root.join("manafield/state/active-release.json"),
            "{BAD JSON",
        )
        .unwrap();
        assert!(bind(Some(root.to_str().unwrap()), "echo", "identity", "account").is_err());
        assert!(!root.join("manafield/temp/working.yaml").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
