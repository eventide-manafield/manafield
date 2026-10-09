//! Operator-only local log reader. Never exposes logs on an unauthenticated
//! Core HTTP endpoint. Use --file or MANAFIELD_LOG_FILE from a trusted shell.
use std::collections::VecDeque;
use std::fs::{File, symlink_metadata};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::cli::CliError;
use crate::logging::LogRecord;

#[derive(Debug, PartialEq, Eq)]
pub struct Options {
    pub file: Option<PathBuf>,
    pub level: Option<String>,
    pub source: Option<String>,
    pub since: Option<Duration>,
    pub audit: bool,
    pub limit: usize,
    pub json: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            file: None,
            level: None,
            source: None,
            since: None,
            audit: false,
            limit: 100,
            json: false,
        }
    }
}

pub fn parse<I: Iterator<Item = String>>(mut args: I) -> Result<Options, CliError> {
    let mut options = Options::default();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--file" if options.file.is_none() => {
                options.file =
                    Some(PathBuf::from(args.next().ok_or_else(|| {
                        CliError::Usage("log --file needs a path".into())
                    })?));
            }
            "--level" if options.level.is_none() => {
                let level = args
                    .next()
                    .ok_or_else(|| CliError::Usage("log --level needs a severity".into()))?;
                if severity(&level).is_none() {
                    return Err(CliError::Usage(
                        "log level must be trace, debug, info, warn or error".into(),
                    ));
                }
                options.level = Some(level);
            }
            "--source" if options.source.is_none() => {
                let source = args
                    .next()
                    .ok_or_else(|| CliError::Usage("log --source needs a source name".into()))?;
                if source.is_empty() || source.len() > 128 {
                    return Err(CliError::Usage("invalid log source".into()));
                }
                options.source = Some(source);
            }
            "--since" if options.since.is_none() => {
                let value = args
                    .next()
                    .ok_or_else(|| CliError::Usage("log --since needs a duration".into()))?;
                options.since = Some(parse_duration(&value)?);
            }
            "--limit" => {
                let limit = args
                    .next()
                    .ok_or_else(|| CliError::Usage("log --limit needs a count".into()))?;
                let n: usize = limit
                    .parse()
                    .map_err(|_| CliError::Usage("invalid log limit".into()))?;
                if !(1..=10000).contains(&n) {
                    return Err(CliError::Usage("log limit must be 1-10000".into()));
                }
                options.limit = n;
            }
            "--audit" if !options.audit => options.audit = true,
            "--json" if !options.json => options.json = true,
            _ => {
                return Err(CliError::Usage(format!(
                    "unknown or repeated log option '{arg}'"
                )));
            }
        }
    }
    Ok(options)
}

fn parse_duration(input: &str) -> Result<Duration, CliError> {
    if input.len() < 2 {
        return Err(CliError::Usage("log --since example: 15m, 1h, 7d".into()));
    }
    let (n, unit) = input.split_at(input.len() - 1);
    let value: u64 = n
        .parse()
        .map_err(|_| CliError::Usage("invalid log duration".into()))?;
    if value == 0 {
        return Err(CliError::Usage("duration must be positive".into()));
    }
    let multiplier = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3600,
        "d" => 86400,
        _ => return Err(CliError::Usage("duration unit must be s/m/h/d".into())),
    };
    let seconds = value
        .checked_mul(multiplier)
        .ok_or_else(|| CliError::Usage("duration too large".into()))?;
    Ok(Duration::from_secs(seconds))
}

fn severity(level: &str) -> Option<u8> {
    match level.to_ascii_lowercase().as_str() {
        "trace" => Some(0),
        "debug" => Some(1),
        "info" => Some(2),
        "warn" => Some(3),
        "error" => Some(4),
        _ => None,
    }
}

fn audit(value: &LogRecord) -> bool {
    value
        .event
        .as_deref()
        .is_some_and(|event| event.starts_with("audit."))
}

fn matching(value: &LogRecord, options: &Options, threshold: u64) -> bool {
    if let Some(level) = &options.level {
        let Some(observed) = severity(&value.level) else {
            return false;
        };
        if observed < severity(level).unwrap_or(0) {
            return false;
        }
    }
    if let Some(source) = &options.source
        && &value.source != source
    {
        return false;
    }
    if options.audit && !audit(value) {
        return false;
    }
    value.timestamp_ms >= threshold
}

pub fn run(options: Options) -> Result<(), CliError> {
    let file = match options.file.clone() {
        Some(path) => path,
        None => {
            let value = std::env::var_os("MANAFIELD_LOG_FILE").ok_or_else(|| {
                CliError::Usage(
                    "log needs --file FILE or MANAFIELD_LOG_FILE (local operator-only path)".into(),
                )
            })?;
            PathBuf::from(value)
        }
    };
    let records = read(&file, &options)
        .map_err(|e| CliError::Execution(format!("cannot read operator log: {e}")))?;
    for value in records {
        if options.json {
            println!(
                "{}",
                serde_json::to_string(&value).map_err(|e| CliError::Execution(e.to_string()))?
            );
        } else {
            println!("{}", value.console_line());
        }
    }
    Ok(())
}

pub fn read(file: &Path, options: &Options) -> std::io::Result<Vec<LogRecord>> {
    // Follow neither a symlink nor a globally readable log. A system operator
    // must intentionally select a private path, not an arbitrary HTTP resource.
    let meta = symlink_metadata(file)?;
    if !meta.file_type().is_file() || meta.file_type().is_symlink() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "log path is not a regular file",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "log file must be private (0600)",
            ));
        }
    }
    let cutoff = options
        .since
        .map(|since| {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            now.saturating_sub(since.as_millis() as u64)
        })
        .unwrap_or_default();
    let mut ring = VecDeque::with_capacity(options.limit.min(1024));
    for line in BufReader::new(File::open(file)?).lines() {
        let line = line?;
        let Ok(value) = serde_json::from_str::<LogRecord>(&line) else {
            continue;
        };
        if !matching(&value, options, cutoff) {
            continue;
        }
        if ring.len() == options.limit {
            ring.pop_front();
        }
        ring.push_back(value);
    }
    Ok(ring.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_log_filters() {
        let v = parse(
            [
                "--level", "warn", "--source", "account", "--since", "1h", "--limit", "25",
                "--audit",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .unwrap();
        assert_eq!(v.level.as_deref(), Some("warn"));
        assert_eq!(v.since, Some(Duration::from_secs(3600)));
        assert_eq!(v.limit, 25);
        assert!(v.audit);
        assert!(parse(["--limit".into(), "0".into()].into_iter()).is_err());
        assert!(parse(["--since".into(), "1x".into()].into_iter()).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn private_jsonl_reader_rejects_world_readable_and_symlink_files() {
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;
        let name = format!(
            "manafield-log-test-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        );
        let directory = std::env::temp_dir().join(name);
        std::fs::create_dir(&directory).unwrap();
        let file = directory.join("core.jsonl");
        let mut handle = std::fs::OpenOptions::new()
            .write(true).create_new(true).mode(0o600).open(&file).unwrap();
        let event = LogRecord {
            timestamp_ms: 1, level: "warn".into(), source: "core".into(),
            event: None, message: "hello".into(), fields:Default::default(),
        };
        writeln!(handle, "{}", serde_json::to_string(&event).unwrap()).unwrap();
        drop(handle);
        let opts = Options::default();
        assert_eq!(read(&file, &opts).unwrap(), vec![event]);
        let link = directory.join("link.jsonl");
        std::os::unix::fs::symlink(&file, &link).unwrap();
        assert!(read(&link, &opts).is_err());
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(read(&file, &opts).is_err());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn severity_and_audit_filters() {
        let event = LogRecord {
            timestamp_ms: 2,
            level: "warn".into(),
            source: "account".into(),
            event: Some("audit.session.revoked".into()),
            message: "revoked".into(),
            fields: Default::default(),
        };
        let options = Options {
            level: Some("warn".into()),
            audit: true,
            ..Options::default()
        };
        assert!(matching(&event, &options, 1));
        assert!(!matching(&event, &options, 3));
        assert!(!matching(
            &event,
            &Options {
                source: Some("manage".into()),
                ..options
            },
            0
        ));
    }
}
