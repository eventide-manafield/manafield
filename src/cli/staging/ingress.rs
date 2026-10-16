//! Declarative Traefik route staging for direct Deploy v0.
//! Never publishes routes automatically: existing traffic must not be moved
//! before the new instance passes readiness checks.
use serde_json::{Map, Value, json};

use super::{CliError, fail};

fn safe_hostname(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && !host.contains("..")
        && host
            .split('.')
            .all(|part| !part.is_empty() && !part.starts_with('-') && !part.ends_with('-'))
        && host
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'-'))
}

fn safe_prefix(prefix: &str) -> bool {
    prefix.starts_with('/')
        && !prefix.contains(['`', '"', '\'', '\\', '\n', '\r'])
        && prefix
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.'))
        && !prefix.split('/').any(|part| part == ".." || part == ".")
}

pub(super) struct Exposure {
    pub(super) host: String,
    pub(super) port: u16,
}

pub(super) fn inspect(module: &Value) -> Result<Option<Exposure>, CliError> {
    let Some(exposure) = module.get("exposure").filter(|v| !v.is_null()) else {
        return Ok(None);
    };
    let host = exposure["host"]
        .as_str()
        .ok_or_else(|| fail("Module exposure host missing"))?;
    if !safe_hostname(host) {
        return Err(fail("Module exposure host must be an ASCII DNS hostname"));
    }
    let port = exposure["targetPort"]
        .as_u64()
        .and_then(|x| u16::try_from(x).ok())
        .filter(|x| *x > 0)
        .ok_or_else(|| fail("Module exposure targetPort must be a valid TCP port"))?;
    match exposure["type"].as_str() {
        Some("host") => (),
        Some("prefix") if exposure["prefix"].as_str().is_some_and(safe_prefix) => (),
        _ => return Err(fail("Unsupported or unsafe Module exposure type/prefix")),
    }
    Ok(Some(Exposure {
        host: host.to_owned(),
        port,
    }))
}

pub(super) fn render(modules: &[Value]) -> Result<Option<String>, CliError> {
    let mut routers = Map::new();
    let mut services = Map::new();
    for module in modules {
        let Some(exposure) = inspect(module)? else {
            continue;
        };
        let id = module["id"]
            .as_str()
            .ok_or_else(|| fail("Module ID missing for route"))?;
        if id.is_empty()
            || !id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_'))
        {
            return Err(fail("Unsafe Module ID for public ingress"));
        }
        let name = format!("manafield-{id}");
        let rule = match module["exposure"]["type"].as_str() {
            Some("host") => format!("Host(`{}`)", exposure.host),
            Some("prefix") => {
                let prefix = module["exposure"]["prefix"]
                    .as_str()
                    .ok_or_else(|| fail("Prefix missing"))?;
                if prefix == "/" {
                    format!("Host(`{}`) && PathPrefix(`/`)", exposure.host)
                } else {
                    format!(
                        "Host(`{}`) && (Path(`{}`) || PathPrefix(`{}/`))",
                        exposure.host, prefix, prefix
                    )
                }
            }
            _ => return Err(fail("Invalid exposure type")),
        };
        let priority = if module["exposure"]["type"] == "prefix" {
            1000 + module["exposure"]["prefix"].as_str().unwrap_or("/").len()
        } else {
            1
        };
        routers.insert(
            name.clone(),
            json!({
                "rule": rule, "entryPoints":["websecure"], "service": name,
                "priority":priority, "tls": {},
            }),
        );
        services.insert(name.clone(), json!({
            "loadBalancer": {"servers":[{"url":format!("http://module-{id}:{}",exposure.port)}]},
        }));
    }
    if routers.is_empty() {
        return Ok(None);
    }
    let value = json!({"http":{"routers":routers,"services":services}});
    Ok(Some(
        serde_yaml_ng::to_string(&value).map_err(|e| fail(e.to_string()))?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn host_exposure_produces_staged_only_traefik_route() {
        let yaml = render(&[json!({
            "id":"noctemblith", "exposure":{
                "type":"host", "host":"noctemblith-dev.manafield.studio", "targetPort":3000
            }
        })])
        .unwrap()
        .unwrap();
        assert!(yaml.contains("noctemblith-dev.manafield.studio"));
        assert!(yaml.contains("http://module-noctemblith:3000"));
    }
    #[test]
    fn rejects_unsafe_host_and_prefix() {
        assert!(
            inspect(&json!({"exposure":{"type":"host","host":"x`evil","targetPort":3000}}))
                .is_err()
        );
        assert!(inspect(&json!({"exposure":{"type":"prefix","host":"manafield.studio","prefix":"/x`y","targetPort":3000}})).is_err());
    }
    #[test]
    fn no_exposure_means_no_ingress_change() {
        assert!(render(&[json!({"id":"private-module"})]).unwrap().is_none());
    }
}
