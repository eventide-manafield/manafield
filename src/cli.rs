use std::env;
use std::error::Error;
use std::fmt;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::core::{ModuleDescriptor, ResourceDescriptor};

const DEFAULT_CORE_URL: &str = "http://127.0.0.1:8080";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Serve,
    Health,
    Ps,
    Resource(Option<String>),
    Help,
    Version,
}

pub fn parse_args<I>(args: I) -> Result<Command, CliError>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();

    let Some(command) = args.next() else {
        return Ok(Command::Serve);
    };

    match command.as_str() {
        "serve" => ensure_no_more(args, Command::Serve),
        "health" => ensure_no_more(args, Command::Health),
        "ps" => ensure_no_more(args, Command::Ps),
        "resource" | "resources" => {
            let id = args.next();

            if let Some(extra) = args.next() {
                return Err(CliError::Usage(format!(
                    "unexpected argument '{extra}' after resource ID"
                )));
            }

            Ok(Command::Resource(id))
        }
        "help" | "-h" | "--help" => ensure_no_more(args, Command::Help),
        "version" | "-V" | "--version" => ensure_no_more(args, Command::Version),
        other => Err(CliError::Usage(format!("unknown command '{other}'"))),
    }
}

fn ensure_no_more<I>(mut args: I, command: Command) -> Result<Command, CliError>
where
    I: Iterator<Item = String>,
{
    if let Some(extra) = args.next() {
        Err(CliError::Usage(format!("unexpected argument '{extra}'")))
    } else {
        Ok(command)
    }
}

pub fn run(command: Command) -> Result<(), CliError> {
    match command {
        Command::Serve => Err(CliError::Usage(
            "serve must be handled by the server entrypoint".to_owned(),
        )),
        Command::Health => print_health(),
        Command::Ps => print_ps(),
        Command::Resource(id) => print_resource(id.as_deref()),
        Command::Help => {
            print_help();
            Ok(())
        }
        Command::Version => {
            println!("manafield {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}

fn print_health() -> Result<(), CliError> {
    let client = CoreClient::from_env()?;
    let health: HealthResponse = client.get("/health")?;

    println!(
        "Core {} ({} {})",
        health.status, health.service, health.version
    );

    Ok(())
}

fn print_ps() -> Result<(), CliError> {
    let client = CoreClient::from_env()?;
    let health: HealthResponse = client.get("/health")?;
    let modules: Vec<ModuleDescriptor> = client.get("/modules")?;
    let resources: Vec<ResourceDescriptor> = client.get("/resources")?;

    println!("Manafield");
    println!();
    println!("CORE");
    println!(
        "  {:<24} {} ({})",
        health.service, health.status, health.version
    );

    println!();
    println!("MODULES ({})", modules.len());
    if modules.is_empty() {
        println!("  (none)");
    } else {
        for module in modules {
            println!("  {:<24} registered  {}", module.id, module.version);
        }
    }

    println!();
    println!("RESOURCES ({})", resources.len());
    if resources.is_empty() {
        println!("  (none)");
    } else {
        for resource in resources {
            println!(
                "  {:<24} registered  {}",
                resource.id, resource.resource_type
            );

            for capability in resource.provides.capabilities {
                println!("    - {} @ {}", capability.id, capability.version);
            }
        }
    }

    Ok(())
}

fn print_resource(id: Option<&str>) -> Result<(), CliError> {
    let client = CoreClient::from_env()?;

    match id {
        Some(id) => {
            let resource: ResourceDescriptor = client.get(&format!("/resources/{id}"))?;

            println!("Resource {}", resource.id);
            println!("  Name: {}", resource.name);
            println!("  Type: {}", resource.resource_type);

            if let Some(description) = resource.description {
                println!("  Description: {description}");
            }

            println!("  Capabilities:");
            if resource.provides.capabilities.is_empty() {
                println!("    (none)");
            } else {
                for capability in resource.provides.capabilities {
                    println!("    - {} @ {}", capability.id, capability.version);
                }
            }
        }
        None => {
            let resources: Vec<ResourceDescriptor> = client.get("/resources")?;

            if resources.is_empty() {
                println!("No Resources registered.");
            } else {
                println!("{:<24} {:<18} CAPABILITIES", "ID", "TYPE");

                for resource in resources {
                    let capabilities = resource
                        .provides
                        .capabilities
                        .iter()
                        .map(|capability| format!("{}@{}", capability.id, capability.version))
                        .collect::<Vec<_>>()
                        .join(", ");

                    println!(
                        "{:<24} {:<18} {}",
                        resource.id, resource.resource_type, capabilities
                    );
                }
            }
        }
    }

    Ok(())
}

pub fn print_help() {
    println!(
        "Manafield {}\n\
         \n\
         Usage:\n\
           manafield [COMMAND]\n\
         \n\
         Commands:\n\
           serve             Run Manafield Core (default when no command is given)\n\
           health            Show Core health\n\
           ps                Show Core, Module, and Resource summary\n\
           resource [ID]     List Resources or show one Resource\n\
           help              Show this help\n\
           version           Show version\n\
         \n\
         Environment:\n\
           MANAFIELD_CORE_URL  Core API base URL for CLI commands\n\
                               default: {}",
        env!("CARGO_PKG_VERSION"),
        DEFAULT_CORE_URL
    );
}

#[derive(Debug, Deserialize)]
struct HealthResponse {
    status: String,
    service: String,
    version: String,
}

struct CoreClient {
    endpoint: HttpEndpoint,
}

impl CoreClient {
    fn from_env() -> Result<Self, CliError> {
        let base_url =
            env::var("MANAFIELD_CORE_URL").unwrap_or_else(|_| DEFAULT_CORE_URL.to_owned());

        Ok(Self {
            endpoint: HttpEndpoint::parse(&base_url)?,
        })
    }

    fn get<T>(&self, path: &str) -> Result<T, CliError>
    where
        T: DeserializeOwned,
    {
        let target = self.endpoint.target(path);
        let url = format!("http://{}{}", self.endpoint.authority, target);

        let mut stream =
            TcpStream::connect(&self.endpoint.address).map_err(|error| CliError::Request {
                url: url.clone(),
                source: error.to_string(),
            })?;

        stream
            .set_read_timeout(Some(REQUEST_TIMEOUT))
            .map_err(|error| CliError::Request {
                url: url.clone(),
                source: error.to_string(),
            })?;
        stream
            .set_write_timeout(Some(REQUEST_TIMEOUT))
            .map_err(|error| CliError::Request {
                url: url.clone(),
                source: error.to_string(),
            })?;

        let request = format!(
            "GET {target} HTTP/1.1\r\n\
             Host: {}\r\n\
             Accept: application/json\r\n\
             Connection: close\r\n\
             User-Agent: manafield/{}\r\n\
             \r\n",
            self.endpoint.authority,
            env!("CARGO_PKG_VERSION")
        );

        stream
            .write_all(request.as_bytes())
            .map_err(|error| CliError::Request {
                url: url.clone(),
                source: error.to_string(),
            })?;

        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .map_err(|error| CliError::Request {
                url: url.clone(),
                source: error.to_string(),
            })?;

        let header_end = find_bytes(&response, b"\r\n\r\n").ok_or_else(|| CliError::Response {
            url: url.clone(),
            source: "invalid HTTP response".to_owned(),
        })?;

        let headers =
            std::str::from_utf8(&response[..header_end]).map_err(|error| CliError::Response {
                url: url.clone(),
                source: error.to_string(),
            })?;

        let status = headers
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|status| status.parse::<u16>().ok())
            .ok_or_else(|| CliError::Response {
                url: url.clone(),
                source: "invalid HTTP status line".to_owned(),
            })?;

        if !(200..300).contains(&status) {
            return Err(CliError::HttpStatus { url, status });
        }

        let raw_body = &response[header_end + 4..];
        let body = if headers
            .lines()
            .any(|line| line.eq_ignore_ascii_case("transfer-encoding: chunked"))
        {
            decode_chunked(raw_body).map_err(|source| CliError::Response {
                url: url.clone(),
                source,
            })?
        } else {
            raw_body.to_vec()
        };

        serde_json::from_slice(&body).map_err(|error| CliError::Response {
            url,
            source: error.to_string(),
        })
    }
}

struct HttpEndpoint {
    authority: String,
    address: String,
    base_path: String,
}

impl HttpEndpoint {
    fn parse(base_url: &str) -> Result<Self, CliError> {
        let raw = base_url.strip_prefix("http://").ok_or_else(|| {
            CliError::InvalidCoreUrl("MANAFIELD_CORE_URL must use http:// in CLI v0".to_owned())
        })?;

        let (authority, path) = match raw.split_once('/') {
            Some((authority, path)) => (authority, path),
            None => (raw, ""),
        };

        if authority.is_empty() {
            return Err(CliError::InvalidCoreUrl(
                "MANAFIELD_CORE_URL is missing a host".to_owned(),
            ));
        }

        if authority.starts_with('[') {
            return Err(CliError::InvalidCoreUrl(
                "IPv6 Core URLs are not supported by CLI v0".to_owned(),
            ));
        }

        let address = match authority.rsplit_once(':') {
            Some((host, port)) if !host.is_empty() && port.parse::<u16>().is_ok() => {
                authority.to_owned()
            }
            Some(_) => {
                return Err(CliError::InvalidCoreUrl(
                    "MANAFIELD_CORE_URL contains an invalid port".to_owned(),
                ));
            }
            None => format!("{authority}:80"),
        };

        let base_path = if path.trim_matches('/').is_empty() {
            String::new()
        } else {
            format!("/{}", path.trim_matches('/'))
        };

        Ok(Self {
            authority: authority.to_owned(),
            address,
            base_path,
        })
    }

    fn target(&self, path: &str) -> String {
        format!("{}{}", self.base_path, path)
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn decode_chunked(input: &[u8]) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    let mut cursor = 0;

    loop {
        let line_end = find_bytes(&input[cursor..], b"\r\n")
            .map(|offset| cursor + offset)
            .ok_or_else(|| "invalid chunked response".to_owned())?;

        let size_line =
            std::str::from_utf8(&input[cursor..line_end]).map_err(|error| error.to_string())?;
        let size_hex = size_line.split(';').next().unwrap_or(size_line);
        let size = usize::from_str_radix(size_hex.trim(), 16).map_err(|error| error.to_string())?;

        cursor = line_end + 2;

        if size == 0 {
            break;
        }

        let chunk_end = cursor
            .checked_add(size)
            .filter(|end| *end <= input.len())
            .ok_or_else(|| "truncated chunked response".to_owned())?;

        output.extend_from_slice(&input[cursor..chunk_end]);
        cursor = chunk_end;

        if input.get(cursor..cursor + 2) != Some(b"\r\n") {
            return Err("invalid chunk terminator".to_owned());
        }

        cursor += 2;
    }

    Ok(output)
}

#[derive(Debug)]
pub enum CliError {
    Usage(String),
    InvalidCoreUrl(String),
    Request { url: String, source: String },
    HttpStatus { url: String, status: u16 },
    Response { url: String, source: String },
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(message) | Self::InvalidCoreUrl(message) => write!(f, "{message}"),
            Self::Request { url, source } => {
                write!(f, "failed to request Core API '{url}': {source}")
            }
            Self::HttpStatus { url, status } => {
                write!(f, "Core API '{url}' returned HTTP {status}")
            }
            Self::Response { url, source } => {
                write!(f, "failed to decode Core response from '{url}': {source}")
            }
        }
    }
}

impl Error for CliError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn no_arguments_defaults_to_serve() {
        assert_eq!(parse_args(args(&[])).unwrap(), Command::Serve);
    }

    #[test]
    fn parses_ps() {
        assert_eq!(parse_args(args(&["ps"])).unwrap(), Command::Ps);
    }

    #[test]
    fn parses_resource_with_id() {
        assert_eq!(
            parse_args(args(&["resource", "main-db"])).unwrap(),
            Command::Resource(Some("main-db".to_owned()))
        );
    }

    #[test]
    fn rejects_unknown_command() {
        assert!(matches!(
            parse_args(args(&["wat"])),
            Err(CliError::Usage(_))
        ));
    }

    #[test]
    fn parses_core_url() {
        let endpoint = HttpEndpoint::parse("http://127.0.0.1:18080/base").unwrap();

        assert_eq!(endpoint.address, "127.0.0.1:18080");
        assert_eq!(endpoint.authority, "127.0.0.1:18080");
        assert_eq!(endpoint.target("/health"), "/base/health");
    }

    #[test]
    fn decodes_chunked_body() {
        let decoded = decode_chunked(b"4\r\ntest\r\n0\r\n\r\n").unwrap();

        assert_eq!(decoded, b"test");
    }
}
