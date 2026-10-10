//! Secret creation, permissions and private-file durability policy.
use super::{CliError, fail};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

pub(super) fn create_secret(path: &Path) -> Result<(), CliError> {
    if path.exists() {
        if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(fail(format!(
                "refusing symlink secret '{}'",
                path.display()
            )));
        }
        if !path.is_file() {
            return Err(fail(format!(
                "secret path is not a file: {}",
                path.display()
            )));
        }
        if path.file_name().is_some_and(|f| f == "admin.password") {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(path, fs::Permissions::from_mode(0o600))
                    .map_err(|e| fail(format!("cannot restrict PostgreSQL admin secret: {e}")))?;
            }
        }
        return Ok(());
    }
    let parent = path.parent().ok_or_else(|| fail("invalid secret path"))?;
    fs::create_dir_all(parent).map_err(|e| fail(e.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
            .map_err(|e| fail(e.to_string()))?;
    }
    let mut random = [0u8; 24];
    fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut random))
        .map_err(|e| fail(format!("cannot obtain secure random password: {e}")))?;
    let mut value = String::with_capacity(49);
    for x in random {
        value.push_str(&format!("{x:02x}"));
    }
    value.push('\n');
    write_secret_file(path, value.as_bytes())?;
    // Admin password never needs to be read by the Module container.
    if path.file_name().is_some_and(|f| f == "admin.password") {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))
                .map_err(|e| fail(format!("cannot restrict PostgreSQL admin secret: {e}")))?;
        }
    }
    Ok(())
}
fn write_secret_file(path: &Path, bytes: &[u8]) -> Result<(), CliError> {
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Needed for nonroot Module containers behind private host dirs.
        opts.mode(0o444);
    }
    let mut f = opts
        .open(path)
        .map_err(|e| fail(format!("cannot create secret '{}': {e}", path.display())))?;
    f.write_all(bytes)
        .and_then(|_| f.sync_all())
        .map_err(|e| fail(e.to_string()))?;
    Ok(())
}
pub(super) fn write_private(path: &Path, bytes: &[u8]) -> Result<(), CliError> {
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(path).map_err(|e| {
        fail(format!(
            "cannot create private file '{}': {e}",
            path.display()
        ))
    })?;
    f.write_all(bytes)
        .and_then(|_| f.sync_all())
        .map_err(|e| fail(e.to_string()))?;
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn secret_files_remain_private_and_existing_secrets_are_reused() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "manafield-staging-secret-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let admin = directory.join("admin.password");
        create_secret(&admin).unwrap();
        let first = fs::read(&admin).unwrap();
        assert_eq!(first.len(), 49);
        assert_eq!(
            fs::metadata(&admin).unwrap().permissions().mode() & 0o777,
            0o600
        );

        create_secret(&admin).unwrap();
        assert_eq!(fs::read(&admin).unwrap(), first);

        let private = directory.join("release.env");
        write_private(&private, b"TEST_VALUE=ok\n").unwrap();
        assert_eq!(
            fs::metadata(&private).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(write_private(&private, b"overwrite").is_err());

        let link = directory.join("symlink.password");
        symlink(&admin, &link).unwrap();
        assert!(create_secret(&link).is_err());

        fs::remove_dir_all(directory).unwrap();
    }
}
