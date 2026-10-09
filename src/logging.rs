//! Core-owned operational logging. Every event reaches stdout. Optional file
//! and PostgreSQL sinks are best-effort and must never block the Core runtime.
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::mpsc::{self, SyncSender};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tracing::{Event, Subscriber, field::{Field, Visit}, subscriber::Interest};
use tracing_subscriber::{EnvFilter, Layer, layer::{Context, SubscriberExt}, util::SubscriberInitExt};

pub mod postgres_sink;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct LogRecord {
    pub timestamp_ms: u64,
    pub level: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    pub message: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, String>,
}

impl LogRecord {
    pub fn console_line(&self) -> String {
        let level = self.level.to_lowercase();
        let source = self.source.replace(['\n', '\r', '[', ']'], "_");
        format!("({level})[{source}] {}", self.message.replace(['\n', '\r'], " "))
    }
}

#[derive(Default)]
struct EventFields {
    message: Option<String>,
    event: Option<String>,
    fields: BTreeMap<String, String>,
}

fn sensitive_field(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    ["password", "passwd", "secret", "token", "cookie", "authorization",
     "credential", "apikey", "api_key", "private_key", "dsn", "database_url"]
        .iter()
        .any(|name| lower.contains(name))
}

impl Visit for EventFields {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let name = field.name();
        if sensitive_field(name) {
            return;
        }
        let value = format!("{value:?}");
        match name {
            "message" => self.message = Some(value.trim_matches('"').to_owned()),
            "event" => self.event = Some(value.trim_matches('"').to_owned()),
            _ => { self.fields.insert(name.to_owned(), value); }
        }
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        let name = field.name();
        if sensitive_field(name) { return; }
        match name {
            "message" => self.message = Some(value.to_owned()),
            "event" => self.event = Some(value.to_owned()),
            _ => { self.fields.insert(name.to_owned(), value.to_owned()); }
        }
    }
}

fn record(event: &Event<'_>) -> LogRecord {
    let metadata = event.metadata();
    let mut visitor = EventFields::default();
    event.record(&mut visitor);
    let source = metadata.target();
    let source = match source {
        "manafield_core" => "core",
        "manafield::cli" => "cli",
        _ if source.starts_with("manafield::") => &source["manafield::".len()..],
        _ => source,
    };
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
    LogRecord {
        timestamp_ms,
        level: metadata.level().as_str().to_ascii_lowercase(),
        source: source.to_owned(),
        event: visitor.event,
        message: visitor.message.unwrap_or_default(),
        fields: visitor.fields,
    }
}

pub struct LogLayer {
    file: Option<Mutex<File>>,
    db: Option<SyncSender<LogRecord>>,
}

impl LogLayer {
    pub fn new(file: Option<File>, db: Option<SyncSender<LogRecord>>) -> Self {
        Self { file: file.map(Mutex::new), db }
    }
}

impl<S: Subscriber> Layer<S> for LogLayer {
    fn register_callsite(&self, _metadata: &'static tracing::Metadata<'static>) -> Interest {
        Interest::always()
    }
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let value = record(event);
        let _ = writeln!(io::stdout().lock(), "{}", value.console_line());
        if let Some(file) = &self.file
            && let Ok(mut file) = file.lock()
            && serde_json::to_writer(&mut *file, &value).is_ok()
        {
            let _ = file.write_all(b"\n");
        }
        if let Some(db) = &self.db {
            // Bounded, nonblocking queue: an unavailable DB never stalls Core.
            let _ = db.try_send(value);
        }
    }
}

#[cfg(unix)]
fn open_private_log(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    if let Ok(metadata) = std::fs::symlink_metadata(path) {
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "log path must be a regular file"));
        }
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "log file is readable by other users"));
        }
    }
    OpenOptions::new().create(true).append(true).mode(0o600).open(path)
}

#[cfg(not(unix))]
fn open_private_log(path: &Path) -> io::Result<File> {
    OpenOptions::new().create(true).append(true).open(path)
}

pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("manafield=info"));
    let file = match std::env::var("MANAFIELD_LOG_FILE") {
        Ok(path) if !path.trim().is_empty() => Some(open_private_log(Path::new(&path))?),
        _ => None,
    };
    let db = match postgres_sink::Config::from_env()? {
        Some(config) => {
            let (tx, rx) = mpsc::sync_channel(1024);
            std::thread::Builder::new().name("manafield-log-db".into())
                .spawn(move || postgres_sink::run(config, rx))?;
            Some(tx)
        }
        None => None,
    };
    tracing_subscriber::registry().with(filter).with(LogLayer::new(file, db)).init();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn structured_record_formats_expected_console_line() {
        let r = LogRecord { timestamp_ms: 1, level:"warn".into(), source:"account".into(),
            event:Some("session.revoked".into()), message:"Session revoked".into(), fields:BTreeMap::new() };
        assert_eq!(r.console_line(), "(warn)[account] Session revoked");
        assert!(serde_json::to_string(&r).unwrap().contains(r#""event":"session.revoked""#));
    }
    #[test]
    fn sensitive_field_names_are_filtered() {
        for name in ["password", "access_token", "request_cookie", "authorization", "DB_DSN"] {
            assert!(sensitive_field(name));
        }
        assert!(!sensitive_field("module_id"));
    }
}
