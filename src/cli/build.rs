use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use super::CliError;

pub(super) fn run(workspace: &str, revision: &str, core_image: &str) -> Result<(), CliError> {
    let root = Path::new(workspace);
    let modules = read_rows(&root.join("module-sources.tsv"), 8)?;
    let resources_path = root.join("ci-plan/resources.tsv");
    let resources = if resources_path.is_file() {
        read_rows(&resources_path, 2)?
    } else {
        Vec::new()
    };
    let mut images = format!("core.image={core_image}\ncore.revision={revision}\n");
    let mut providers = String::new();

    // Check all known build types/providers before starting potentially expensive builds.
    for row in &modules {
        if row[5] != "docker" {
            return Err(CliError::Execution(format!(
                "unsupported Module build type '{}'",
                row[5]
            )));
        }
    }
    for row in &resources {
        if row[1] != "postgresql" {
            return Err(CliError::Execution(format!(
                "unsupported Resource provider '{}'",
                row[1]
            )));
        }
    }

    docker_build(
        root,
        &[
            "--target",
            "core-runtime",
            "--label",
            &format!("org.opencontainers.image.revision={revision}"),
            "--tag",
            core_image,
            ".",
        ],
    )?;

    for row in modules {
        let module_id = &row[0];
        let directory = PathBuf::from(&row[2]);
        let revision = &row[3];
        let image = &row[4];
        let context = directory.join(&row[6]);
        let dockerfile = directory.join(&row[7]);
        let label = format!("org.opencontainers.image.revision={revision}");
        docker_build(
            root,
            &[
                "--label",
                &label,
                "--tag",
                image,
                "--file",
                &dockerfile.to_string_lossy(),
                &context.to_string_lossy(),
            ],
        )?;
        images.push_str(&format!(
            "module.{module_id}.image={image}\nmodule.{module_id}.revision={revision}\n"
        ));
    }

    for row in resources {
        let id = &row[0];
        let provider = &row[1];
        let image = format!("manafield-resource-postgresql:{revision}");
        let label = format!("org.opencontainers.image.revision={revision}");
        docker_build(
            root,
            &[
                "--label",
                &label,
                "--tag",
                &image,
                "--file",
                "providers/postgresql/Dockerfile",
                "providers/postgresql",
            ],
        )?;
        providers.push_str(&format!("{id}\t{provider}\t{image}\n"));
        images.push_str(&format!("resource.{id}.provider.image={image}\n"));
    }

    write_file(&root.join("resource-providers.tsv"), &providers)?;
    write_file(&root.join("resolved-images.env"), &images)?;
    Ok(())
}

fn docker_build(cwd: &Path, args: &[&str]) -> Result<(), CliError> {
    let status = ProcessCommand::new("docker")
        .current_dir(cwd)
        .arg("build")
        .args(args)
        .status()
        .map_err(|error| CliError::Execution(format!("cannot execute Docker build: {error}")))?;
    if !status.success() {
        return Err(CliError::Execution(format!(
            "Docker build failed: {status}"
        )));
    }
    Ok(())
}

fn read_rows(path: &Path, columns: usize) -> Result<Vec<Vec<String>>, CliError> {
    let file = File::open(path).map_err(|error| {
        CliError::Execution(format!("cannot read '{}': {error}", path.display()))
    })?;
    let mut rows = Vec::new();
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line = line.map_err(|error| CliError::Execution(error.to_string()))?;
        if line.is_empty() {
            continue;
        }
        let row: Vec<String> = line.split('\t').map(str::to_owned).collect();
        if row.len() != columns || row.iter().any(String::is_empty) {
            return Err(CliError::Execution(format!(
                "invalid {}-column TSV at {}:{}",
                columns,
                path.display(),
                index + 1
            )));
        }
        rows.push(row);
    }
    Ok(rows)
}

fn write_file(path: &Path, data: &str) -> Result<(), CliError> {
    fs::write(path, data)
        .map_err(|error| CliError::Execution(format!("cannot write '{}': {error}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn malformed_tsv_is_rejected() {
        let path = std::env::temp_dir().join(format!(
            "manafield-test-{}-{}.tsv",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut file = File::create(&path).unwrap();
        writeln!(file, "only-one-field").unwrap();
        drop(file);
        assert!(read_rows(&path, 2).is_err());
        fs::remove_file(path).unwrap();
    }
}
