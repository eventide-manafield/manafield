//! Executor-side S3 Resource configuration. This is NOT a Core Capability
//! contract: it describes one concrete Resource Instance's connection metadata.
//! Credentials belong only in private Instance-root files, never in this value.

use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct S3ResourceConfig {
    pub endpoint: String,
    pub bucket: String,
    pub region: String,
    pub force_path_style: bool,
    pub public_base_url: Option<String>,
}

impl S3ResourceConfig {
    pub fn parse(value: &Value) -> Result<Self, String> {
        let obj = value
            .as_object()
            .ok_or_else(|| "S3 Resource config must be a mapping".to_owned())?;
        for key in obj.keys() {
            if !matches!(
                key.as_str(),
                "endpoint" | "bucket" | "region" | "forcePathStyle" | "publicBaseUrl"
            ) {
                return Err(format!(
                    "unsupported S3 Resource config key '{key}' (credentials must be in secret files)"
                ));
            }
        }
        let required = |key: &str| -> Result<&str, String> {
            obj.get(key)
                .and_then(Value::as_str)
                .filter(|v| !v.is_empty())
                .ok_or_else(|| format!("S3 Resource config.{key} must be a nonempty string"))
        };

        let endpoint = required("endpoint")?;
        if !valid_root_https_endpoint(endpoint) {
            return Err("S3 Resource config.endpoint must be a root HTTPS URL".into());
        }
        let bucket = required("bucket")?;
        if !valid_bucket(bucket) {
            return Err("S3 Resource config.bucket must be a valid S3 bucket name".into());
        }
        let region = obj
            .get("region")
            .map(|v| v.as_str().ok_or("S3 Resource config.region must be a string"))
            .transpose()?
            .unwrap_or("auto");
        if region.is_empty()
            || region.len() > 40
            || !region.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err("S3 Resource config.region must be a simple region name".into());
        }
        let force_path_style = obj
            .get("forcePathStyle")
            .map(|v| v.as_bool().ok_or("S3 Resource config.forcePathStyle must be boolean"))
            .transpose()?
            .unwrap_or(true);
        let public_base_url = obj
            .get("publicBaseUrl")
            .map(|v| {
                let value = v
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .ok_or("S3 Resource config.publicBaseUrl must be a URL string")?;
                if !valid_public_https_url(value) {
                    return Err("S3 Resource config.publicBaseUrl must be HTTPS without credentials or query");
                }
                Ok(value.to_owned())
            })
            .transpose()?;

        Ok(Self {
            endpoint: endpoint.trim_end_matches('/').to_owned(),
            bucket: bucket.to_owned(),
            region: region.to_owned(),
            force_path_style,
            public_base_url,
        })
    }
}

// URL is an operator-controlled storage endpoint, not a user-supplied redirect.
// v0 accepts only a hostname with optional numeric port and no embedded path,
// query, fragment or credentials. Local HTTP is intentionally excluded.
fn valid_root_https_endpoint(value: &str) -> bool {
    let Some(authority) = value.strip_prefix("https://") else { return false };
    let authority = authority.strip_suffix('/').unwrap_or(authority);
    if authority.is_empty() || authority.contains("..") {
        return false;
    }
    let (host, port) = match authority.split_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (authority, None),
    };
    !host.is_empty()
        && host.split('.').all(|part| !part.is_empty() && !part.starts_with('-') && !part.ends_with('-'))
        && host.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'))
        && port.is_none_or(|p| {
            !p.is_empty()
                && p.bytes().all(|b| b.is_ascii_digit())
                && p.parse::<u16>().is_ok_and(|n| n > 0)
        })
}

fn valid_public_https_url(value: &str) -> bool {
    let Some(host_path) = value.strip_prefix("https://") else { return false };
    let host = host_path.split('/').next().unwrap_or("");
    valid_root_https_endpoint(&format!("https://{host}"))
        && host_path.bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'~' | b':' | b'/' | b'%')
        })
}

fn valid_bucket(name: &str) -> bool {
    name.len() >= 3
        && name.len() <= 63
        && name.bytes().next().is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        && name.bytes().last().is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        && !name.contains("..")
        && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_cloudflare_r2_as_s3_compatible_resource() {
        let cfg = S3ResourceConfig::parse(&json!({
            "endpoint": "https://account.r2.cloudflarestorage.com",
            "bucket": "manafield-uploads",
            "region": "auto",
            "forcePathStyle": true,
            "publicBaseUrl": "https://cdn.example.test/media"
        })).unwrap();
        assert_eq!(cfg.region, "auto");
        assert!(cfg.force_path_style);
        assert_eq!(cfg.public_base_url.as_deref(), Some("https://cdn.example.test/media"));
    }

    #[test]
    fn rejects_config_credentials_and_insecure_endpoints() {
        let base = json!({
            "endpoint": "https://example.invalid",
            "bucket": "files"
        });
        assert!(S3ResourceConfig::parse(&base).is_ok());
        for bad in [
            json!({"endpoint":"http://example.invalid","bucket":"files"}),
            json!({"endpoint":"https://bob:secret@example.invalid","bucket":"files"}),
            json!({"endpoint":"https://example.invalid/path","bucket":"files"}),
            json!({"endpoint":"https://example.invalid","bucket":"../etc"}),
            json!({"endpoint":"https://example.invalid","bucket":"files","secretAccessKey":"must-not-be-here"}),
            json!({"endpoint":"https://example.invalid","bucket":"files","forcePathStyle":"true"}),
            json!({"endpoint":"https://example.invalid:abc","bucket":"files"}),
            json!({"endpoint":"https://example.invalid","bucket":"files","publicBaseUrl":"https://cdn.example.test/x\\"\nmalicious: true"}),
        ] {
            assert!(S3ResourceConfig::parse(&bad).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn defaults_to_path_style_for_s3_compatible_stores() {
        let cfg = S3ResourceConfig::parse(&json!({
            "endpoint": "https://example.invalid/",
            "bucket": "bucket"
        })).unwrap();
        assert_eq!(cfg.endpoint, "https://example.invalid");
        assert_eq!(cfg.region, "auto");
        assert!(cfg.force_path_style);
    }
}
