//! Strict, secret-free projection of an already resolved Instance Build Plan.
//! Jenkins only publishes this projection after verifying the deployed Release.
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use super::CliError;

#[derive(Debug, Serialize, PartialEq, Eq)]
struct BindingSnapshot {
    modules: BTreeMap<String, BTreeMap<String, String>>,
}

fn projection(plan: &Value) -> Result<BindingSnapshot, String> {
    let entries = plan["modules"]
        .as_array()
        .ok_or("Build Plan is missing modules array")?;
    let mut modules = BTreeMap::new();
    for module in entries {
        let id = module["id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or("module has no ID")?;
        let mut bindings = BTreeMap::new();
        let original = module
            .get("bindings")
            .filter(|value| !value.is_null())
            .and_then(Value::as_object);
        if module.get("bindings").is_some_and(|v| !v.is_null()) && original.is_none() {
            return Err(format!("bindings for '{id}' are not an object"));
        }
        if let Some(original) = original {
            for (slot, target) in original {
                let target = target
                    .as_str()
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| format!("binding target for '{id}.{slot}' is invalid"))?;
                bindings.insert(slot.to_owned(), target.to_owned());
            }
        }
        if modules.insert(id.to_owned(), bindings).is_some() {
            return Err(format!("duplicate module ID '{id}'"));
        }
    }
    Ok(BindingSnapshot { modules })
}

pub(super) fn export(plan_file: &str, output_file: &str) -> Result<(), CliError> {
    let input = fs::read_to_string(plan_file)
        .map_err(|e| CliError::Execution(format!("cannot read Build Plan: {e}")))?;
    let plan: Value = serde_json::from_str(&input)
        .map_err(|e| CliError::Execution(format!("invalid Build Plan JSON: {e}")))?;
    let snapshot = projection(&plan).map_err(CliError::Execution)?;
    let output = Path::new(output_file);
    if output.exists() {
        return Err(CliError::Execution(format!(
            "refusing to overwrite binding snapshot '{}'",
            output.display()
        )));
    }

    let bytes = serde_json::to_vec_pretty(&snapshot)
        .map_err(|e| CliError::Execution(format!("serialize bindings: {e}")))?;
    // Create-only, private by default, and no secret or full Instance fields.
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(output)
        .map_err(|e| CliError::Execution(format!("cannot create binding snapshot: {e}")))?;
    file.write_all(&bytes)
        .and_then(|_| file.write_all(b"\n"))
        .and_then(|_| file.sync_all())
        .map_err(|e| CliError::Execution(format!("cannot write binding snapshot: {e}")))?;
    println!("Created safe Manage bindings projection: {}", output.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn exports_only_module_ids_and_target_ids() {
        let plan = json!({
            "instanceId": "private",
            "secret": "must-not-appear",
            "modules": [
                {"id": "account", "bindings": {"state": "db"},
                 "source": {"private": "never-copy"}, "exposure": {"host": "private.example"}},
                {"id": "web", "bindings": {}},
                {"id": "plain"}
            ],
            "resources": [{"id":"db", "credentials":"never-copy"}]
        });
        let snapshot = projection(&plan).unwrap();
        assert_eq!(snapshot.modules["account"]["state"], "db");
        assert!(snapshot.modules["web"].is_empty());
        assert!(snapshot.modules["plain"].is_empty());
        let text = serde_json::to_string(&snapshot).unwrap();
        for forbidden in ["must-not-appear", "private.example", "never-copy", "credentials"] {
            assert!(!text.contains(forbidden));
        }
    }

    #[test]
    fn rejects_broken_binding_data() {
        assert!(projection(&json!({})).is_err());
        assert!(projection(&json!({"modules":[{"id":"m","bindings":{"state":9}}]})).is_err());
        assert!(projection(&json!({"modules":[{"id":"m","bindings":[]}]})).is_err());
        assert!(projection(&json!({"modules":[{"id":"m"},{"id":"m"}]})).is_err());
    }
}
