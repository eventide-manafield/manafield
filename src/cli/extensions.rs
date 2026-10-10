use super::CliError;
use std::env;
use std::path::PathBuf;
use std::process::Command as ProcessCommand;

// Plugins are opt-in executable adapters from a *trusted directory* and are
// executed without a shell. A Module may offer CLI features without extending
// the Rust Core with its domain-specific operations.
pub(super) fn valid_extension_namespace(name: &str) -> bool {
    let bytes = name.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 64
        && bytes[0].is_ascii_lowercase()
        && bytes
            .iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

pub(super) fn run_extension(namespace: &str, arguments: &[String]) -> Result<(), CliError> {
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
