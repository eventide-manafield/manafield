use super::CliError;
use serde::{Deserialize, de::DeserializeOwned};
use std::env;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

pub(super) const DEFAULT_CORE_URL: &str = "http://127.0.0.1:8080";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Deserialize)]
pub(super) struct HealthResponse {
    pub(super) status: String,
    pub(super) service: String,
    pub(super) version: String,
}

pub(super) struct CoreClient {
    endpoint: HttpEndpoint,
}

impl CoreClient {
    pub(super) fn from_env() -> Result<Self, CliError> {
        let base_url =
            env::var("MANAFIELD_CORE_URL").unwrap_or_else(|_| DEFAULT_CORE_URL.to_owned());

        Ok(Self {
            endpoint: HttpEndpoint::parse(&base_url)?,
        })
    }

    pub(super) fn get<T>(&self, path: &str) -> Result<T, CliError>
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

pub(super) struct HttpEndpoint {
    pub(super) authority: String,
    pub(super) address: String,
    pub(super) base_path: String,
}

impl HttpEndpoint {
    pub(super) fn parse(base_url: &str) -> Result<Self, CliError> {
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

    pub(super) fn target(&self, path: &str) -> String {
        format!("{}{}", self.base_path, path)
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

pub(super) fn decode_chunked(input: &[u8]) -> Result<Vec<u8>, String> {
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
