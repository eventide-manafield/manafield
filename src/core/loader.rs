use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::ModuleDescriptor;

const MODULE_MANIFEST_FILE: &str = "manafield.module.json";

pub fn discover_modules(root: &Path) -> Result<Vec<ModuleDescriptor>, ModuleLoadError> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut manifest_paths = Vec::new();
    collect_manifest_paths(root, &mut manifest_paths)?;
    manifest_paths.sort();

    manifest_paths
        .into_iter()
        .map(|path| load_manifest(&path))
        .collect()
}

fn collect_manifest_paths(
    directory: &Path,
    manifests: &mut Vec<PathBuf>,
) -> Result<(), ModuleLoadError> {
    let entries = fs::read_dir(directory).map_err(|source| ModuleLoadError::Io {
        path: directory.to_path_buf(),
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| ModuleLoadError::Io {
            path: directory.to_path_buf(),
            source,
        })?;

        let path = entry.path();

        if path.is_dir() {
            collect_manifest_paths(&path, manifests)?;
        } else if path
            .file_name()
            .is_some_and(|name| name == MODULE_MANIFEST_FILE)
        {
            manifests.push(path);
        }
    }

    Ok(())
}

fn load_manifest(path: &Path) -> Result<ModuleDescriptor, ModuleLoadError> {
    let contents = fs::read_to_string(path).map_err(|source| ModuleLoadError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    serde_json::from_str(&contents).map_err(|source| ModuleLoadError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

#[derive(Debug)]
pub enum ModuleLoadError {
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
}

impl fmt::Display for ModuleLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(f, "failed to read '{}': {source}", path.display())
            }
            Self::Parse { path, source } => {
                write!(f, "failed to parse '{}': {source}", path.display())
            }
        }
    }
}

impl std::error::Error for ModuleLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
        }
    }
}
