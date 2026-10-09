//! Optional PostgreSQL mirror for Core logs. The caller configures a dedicated,
//! least-privileged schema/role. Core's stdout and local file always work without it.
use std::io;
use std::path::Path;
use std::str::FromStr;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use postgres::{Client, NoTls};

use super::LogRecord;

pub struct Config {
    dsn: String,
    schema: String,
}

impl Config {
    pub fn from_env() -> io::Result<Option<Self>> {
        let secret_file = std::env::var("MANAFIELD_LOG_POSTGRES_DSN_FILE")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let schema = std::env::var("MANAFIELD_LOG_POSTGRES_SCHEMA")
            .ok()
            .filter(|value| !value.trim().is_empty());
        match (secret_file, schema) {
            (None, None) => Ok(None),
            (Some(file), Some(schema)) => {
                if !valid_schema(&schema) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "invalid log PostgreSQL schema",
                    ));
                }
                let dsn = std::fs::read_to_string(Path::new(&file))?;
                if dsn.trim().is_empty() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "empty PostgreSQL log DSN file",
                    ));
                }
                Ok(Some(Self {
                    dsn: dsn.trim().to_owned(),
                    schema,
                }))
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "log PostgreSQL binding requires both DSN_FILE and SCHEMA",
            )),
        }
    }
}

fn valid_schema(schema: &str) -> bool {
    schema.len() <= 63
        && schema
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_lowercase)
        && schema
            .as_bytes()
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_')
}

fn connect(config: &Config) -> Result<Client, postgres::Error> {
    let mut connection = postgres::Config::from_str(&config.dsn)?;
    connection.connect_timeout(Duration::from_secs(3));
    let mut client = connection.connect(NoTls)?;
    // Schema creation/allocation is the PostgreSQL Provider's responsibility.
    // The log DB user should NOT have privileges to create arbitrary schemas.
    let schema = &config.schema;
    client.batch_execute(&format!(
        "CREATE TABLE IF NOT EXISTS {schema}.core_log_events (
            id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
            timestamp_ms BIGINT NOT NULL,
            level TEXT NOT NULL,
            source TEXT NOT NULL,
            event TEXT,
            message TEXT NOT NULL,
            fields JSONB NOT NULL DEFAULT '{{}}'::jsonb
        );
        CREATE INDEX IF NOT EXISTS core_log_events_timestamp_idx
            ON {schema}.core_log_events(timestamp_ms DESC);"
    ))?;
    Ok(client)
}

fn insert(client: &mut Client, schema: &str, value: &LogRecord) -> Result<(), postgres::Error> {
    let sql = format!(
        "INSERT INTO {schema}.core_log_events (timestamp_ms,level,source,event,message,fields)
         VALUES ($1,$2,$3,$4,$5,$6::jsonb)"
    );
    let fields = serde_json::to_string(&value.fields).unwrap_or_default();
    client.execute(
        &sql,
        &[
            &(value.timestamp_ms as i64),
            &value.level,
            &value.source,
            &value.event,
            &value.message,
            &fields,
        ],
    )?;
    Ok(())
}

// Keep the oldest event in flight while waiting for a disconnected DB.
// Remaining records stay in the bounded channel rather than being drained
// and dropped before the PostgreSQL Provider has created the schema.
fn wait_for_connection(
    queue: &Receiver<LogRecord>,
    pending: &mut Option<LogRecord>,
    next_attempt: Instant,
) -> bool {
    let delay = next_attempt.saturating_duration_since(Instant::now());
    if pending.is_some() {
        std::thread::sleep(delay);
        return true;
    }
    match queue.recv_timeout(delay) {
        Ok(event) => {
            *pending = Some(event);
            true
        }
        Err(RecvTimeoutError::Timeout) => true,
        Err(RecvTimeoutError::Disconnected) => false,
    }
}

pub fn run(config: Config, queue: Receiver<LogRecord>) {
    let mut client = None;
    let mut next_attempt = Instant::now();
    let mut pending = None;

    loop {
        // The Provider starts only after Core is healthy. Retain queued
        // startup events until its schema becomes available, then flush them.
        if client.is_none() && Instant::now() >= next_attempt {
            match connect(&config) {
                Ok(connected) => client = Some(connected),
                Err(_) => {
                    eprintln!(
                        "(warn)[logging] optional PostgreSQL log sink unavailable; console/file logging continues"
                    );
                    next_attempt = Instant::now() + Duration::from_secs(10);
                }
            }
        }

        if client.is_none() {
            if !wait_for_connection(&queue, &mut pending, next_attempt) {
                return;
            }
            continue;
        }

        let event = if let Some(event) = pending.take() {
            event
        } else {
            match queue.recv() {
                Ok(event) => event,
                Err(_) => return,
            }
        };
        if let Some(ref mut live) = client
            && insert(live, &config.schema, &event).is_err()
        {
            eprintln!(
                "(warn)[logging] PostgreSQL log insert failed; console/file logging continues"
            );
            // Keep the failed event for retry. After an ambiguous transport
            // failure, the mirror can contain duplicates (at-least-once).
            pending = Some(event);
            client = None;
            next_attempt = Instant::now() + Duration::from_secs(10);
        }
        // The producer uses try_send on a 1024-entry bounded channel, so an
        // extended outage never stalls Core or grows memory without limit.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_record_is_kept_while_db_is_unavailable() {
        use std::sync::mpsc::sync_channel;

        let (sender, receiver) = sync_channel(2);
        let first = LogRecord {
            timestamp_ms: 1,
            level: "info".into(),
            source: "core".into(),
            event: None,
            message: "Core is listening".into(),
            fields: Default::default(),
        };
        let second = LogRecord {
            timestamp_ms: 2,
            message: "Resource registered".into(),
            ..first.clone()
        };
        sender.send(first.clone()).unwrap();
        let mut pending = None;
        assert!(wait_for_connection(
            &receiver,
            &mut pending,
            Instant::now() + Duration::from_millis(10),
        ));
        assert_eq!(pending, Some(first));

        sender.send(second.clone()).unwrap();
        assert!(wait_for_connection(
            &receiver,
            &mut pending,
            Instant::now() + Duration::from_millis(2),
        ));
        assert_eq!(pending.as_ref().unwrap().message, "Core is listening");
        assert_eq!(receiver.try_recv().unwrap(), second);
    }

    #[test]
    fn disconnected_queue_exits_if_no_event_is_pending() {
        use std::sync::mpsc::sync_channel;
        let (sender, receiver) = sync_channel(1);
        drop(sender);
        let mut pending = None;
        assert!(!wait_for_connection(
            &receiver,
            &mut pending,
            Instant::now(),
        ));
    }

    #[test]
    fn only_safe_postgres_identifiers_accepted() {
        for valid in ["mf_core_logs", "logs", "a1"] {
            assert!(valid_schema(valid));
        }
        for invalid in [
            "",
            "0abc",
            "schema;drop",
            "public.core",
            "\"other\"",
            "A",
            "a-b",
        ] {
            assert!(!valid_schema(invalid));
        }
    }
}
