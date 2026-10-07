use std::collections::BTreeMap;
use std::env;
use std::fs;

use serde::{Deserialize, Serialize};

fn main() {
    if let Err(error) = run() {
        eprintln!("manafield-ingress-traefik: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let input = args
        .next()
        .ok_or("usage: manafield-ingress-traefik <build-plan.json> <output.yml>")?;
    let output = args
        .next()
        .ok_or("usage: manafield-ingress-traefik <build-plan.json> <output.yml>")?;

    if args.next().is_some() {
        return Err("usage: manafield-ingress-traefik <build-plan.json> <output.yml>".into());
    }

    let plan: BuildPlan = serde_json::from_str(&fs::read_to_string(input)?)?;
    let config = TraefikConfig::from_plan(plan);
    let yaml = serde_yaml_ng::to_string(&config)?;

    fs::write(output, yaml)?;
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BuildPlan {
    modules: Vec<ModulePlan>,
}

#[derive(Debug, Deserialize)]
struct ModulePlan {
    id: String,
    #[serde(default)]
    exposure: Option<ExposurePlan>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum ExposurePlan {
    Host {
        host: String,
        #[serde(rename = "targetPort")]
        target_port: u16,
    },
}

#[derive(Debug, Serialize)]
struct TraefikConfig {
    http: HttpConfig,
}

#[derive(Debug, Serialize)]
struct HttpConfig {
    routers: BTreeMap<String, Router>,
    services: BTreeMap<String, Service>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Router {
    rule: String,
    entry_points: Vec<String>,
    service: String,
    tls: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Service {
    load_balancer: LoadBalancer,
}

#[derive(Debug, Serialize)]
struct LoadBalancer {
    servers: Vec<Server>,
}

#[derive(Debug, Serialize)]
struct Server {
    url: String,
}

impl TraefikConfig {
    fn from_plan(plan: BuildPlan) -> Self {
        let mut routers = BTreeMap::new();
        let mut services = BTreeMap::new();

        for module in plan.modules {
            let Some(exposure) = module.exposure else {
                continue;
            };

            match exposure {
                ExposurePlan::Host { host, target_port } => {
                    let component = safe_component_id(&module.id);
                    let name = format!("manafield-{component}");

                    routers.insert(
                        name.clone(),
                        Router {
                            rule: format!("Host(`{host}`)"),
                            entry_points: vec!["websecure".to_string()],
                            service: name.clone(),
                            tls: BTreeMap::new(),
                        },
                    );

                    services.insert(
                        name,
                        Service {
                            load_balancer: LoadBalancer {
                                servers: vec![Server {
                                    url: format!("http://{}:{target_port}", module.id),
                                }],
                            },
                        },
                    );
                }
            }
        }

        Self {
            http: HttpConfig { routers, services },
        }
    }
}

fn safe_component_id(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '-' | '_' | '.')
            {
                character
            } else if character.is_ascii_uppercase() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}
