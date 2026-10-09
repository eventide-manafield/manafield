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

pub fn run(config: Config, queue: Receiver<LogRecord>) {
    let mut client = None;
    let mut next_attempt = Instant::now();

    loop {
        // A configured sink must retry even when Core is idle. The database
        // may be provisioned after Core becomes healthy, and waiting for a
        // fresh event would otherwise leave the sink disconnected indefinitely.
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

        let wait = if client.is_none() {
            next_attempt.saturating_duration_since(Instant::now())
        } else {
            Duration::from_secs(10)
        };
        let event = match queue.recv_timeout(wait) {
            Ok(event) => event,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return,
        };

        if let Some(ref mut live) = client
            && insert(live, &config.schema, &event).is_err()
        {
            eprintln!(
                "(warn)[logging] PostgreSQL log insert failed; console/file logging continues"
            );
            client = None;
            next_attempt = Instant::now() + Duration::from_secs(10);
        }
        // If the database is down, drop only its mirror copy; the event
        // already reached the console and private JSONL storage.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
