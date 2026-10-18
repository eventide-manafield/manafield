use super::*;

fn definition_with_modules(modules: &str) -> InstanceDefinition {
    serde_yaml_ng::from_str(&format!(
        r#"
version: 0
instance:
  id: test
core:
  source:
    repository: https://example.invalid/core.git
    ref: main
modules:
{modules}
deployment:
  modulesNetwork: manafield-modules
  edgeNetwork: manafield-edge
  ingress:
    provider: traefik
    output: /tmp/manafield.yml
"#
    ))
    .unwrap()
}

#[test]
fn core_logging_binding_requires_enabled_postgresql_resource() {
    let basic = r#"
version: 0
instance:
  id: test
core:
  source:
    repository: https://example.invalid/core.git
    ref: main
  bindings:
    loggingState: postgres
resources:
  - id: postgres
    provider: postgresql
deployment:
  modulesNetwork: modules
  edgeNetwork: edge
"#;
    validate_yaml(basic).unwrap();
    let value: InstanceDefinition = serde_yaml_ng::from_str(basic).unwrap();
    let plan = BuildPlan::from(value);
    assert_eq!(plan.core.bindings.get("loggingState").unwrap(), "postgres");
    assert!(validate_yaml(&basic.replace("provider: postgresql", "provider: redis")).is_err());
    assert!(validate_yaml(&basic.replace("loggingState", "unknownSlot")).is_err());
    assert!(
        validate_yaml(&basic.replace(
            "provider: postgresql",
            "provider: postgresql\n    enabled: false"
        ))
        .is_err()
    );
}

#[test]
fn accepts_nested_prefixes_on_same_host() {
    let definition = definition_with_modules(
        r#"  - id: home
    source:
      type: dir
    build:
      type: docker
      context: .
      dockerfile: Dockerfile
    exposure:
      type: prefix
      host: manafield.studio
      prefix: /
      targetPort: 8080
  - id: social
    source:
      type: dir
    build:
      type: docker
      context: .
      dockerfile: Dockerfile
    exposure:
      type: prefix
      host: manafield.studio
      prefix: /social
      targetPort: 8080"#,
    );

    assert_eq!(validate_definition(&definition), Ok(()));
}

#[test]
fn rejects_duplicate_prefix_on_same_host() {
    let definition = definition_with_modules(
        r#"  - id: home-a
    source:
      type: dir
    build:
      type: docker
      context: .
      dockerfile: Dockerfile
    exposure:
      type: prefix
      host: manafield.studio
      prefix: /
      targetPort: 8080
  - id: home-b
    source:
      type: dir
    build:
      type: docker
      context: .
      dockerfile: Dockerfile
    exposure:
      type: prefix
      host: manafield.studio
      prefix: /
      targetPort: 8080"#,
    );

    let error = validate_definition(&definition).unwrap_err();
    assert!(error.contains("duplicate prefix exposure"));
}

#[test]
fn accepts_git_module_subdir() {
    let definition = definition_with_modules(
        r#"  - id: account-core
    source:
      type: git
      repository: https://example.invalid/manafield-account.git
      ref: main
      subdir: modules/account-core
    build:
      type: docker
      context: .
      dockerfile: Dockerfile"#,
    );

    assert_eq!(validate_definition(&definition), Ok(()));

    let plan = BuildPlan::from(definition);
    let ModuleSourceDefinition::Git { subdir, .. } = &plan.modules[0].source else {
        panic!("expected git source");
    };
    assert_eq!(subdir.as_deref(), Some("modules/account-core"));
}

#[test]
fn rejects_git_module_subdir_traversal() {
    let definition = definition_with_modules(
        r#"  - id: account-core
    source:
      type: git
      repository: https://example.invalid/manafield-account.git
      ref: main
      subdir: ../account-core
    build:
      type: docker
      context: .
      dockerfile: Dockerfile"#,
    );

    let error = validate_definition(&definition).unwrap_err();
    assert!(error.contains("canonical relative path"));
}

#[test]
fn accepts_binding_to_enabled_resource() {
    let definition: InstanceDefinition = serde_yaml_ng::from_str(
        r#"
version: 0
instance:
  id: test
core:
  source:
    repository: https://example.invalid/core.git
    ref: main
modules:
  - id: account
    source:
      type: dir
    build:
      type: docker
      context: .
      dockerfile: Dockerfile
    bindings:
      state: manafield-postgres
resources:
  - id: manafield-postgres
    provider: postgresql
deployment:
  modulesNetwork: manafield-modules
  edgeNetwork: manafield-edge
"#,
    )
    .unwrap();

    assert_eq!(validate_definition(&definition), Ok(()));
}

#[test]
fn rejects_binding_to_unknown_instance() {
    let definition: InstanceDefinition = serde_yaml_ng::from_str(
        r#"
version: 0
instance:
  id: test
core:
  source:
    repository: https://example.invalid/core.git
    ref: main
modules:
  - id: account
    source:
      type: dir
    build:
      type: docker
      context: .
      dockerfile: Dockerfile
    bindings:
      state: missing-postgres
deployment:
  modulesNetwork: manafield-modules
  edgeNetwork: manafield-edge
"#,
    )
    .unwrap();

    let error = validate_definition(&definition).unwrap_err();
    assert!(error.contains("targets unknown or disabled instance"));
}

#[test]
fn accepts_s3_resource_and_preserves_safe_instance_config() {
    let input = r#"
version: 0
instance:
  id: storage-test
core:
  source:
    repository: https://example.invalid/core.git
    ref: main
runtimeProviders:
  - id: docker
resources:
  - id: manafield-r2
    provider: s3
    config:
      endpoint: https://example.r2.cloudflarestorage.com
      bucket: manafield-media
      region: auto
      forcePathStyle: true
deployment:
  modulesNetwork: modules
  edgeNetwork: edge
"#;
    validate_yaml(input).unwrap();
    let value: InstanceDefinition = serde_yaml_ng::from_str(input).unwrap();
    let plan = BuildPlan::from(value);
    let config = plan.resources[0].config.as_ref().unwrap();
    assert_eq!(config["bucket"], "manafield-media");
    assert_eq!(config["region"], "auto");
    assert!(config.get("secretAccessKey").is_none());
}

#[test]
fn rejects_missing_and_credential_bearing_s3_configuration() {
    let definition = r#"
version: 0
instance:
  id: storage-test
core:
  source:
    repository: https://example.invalid/core.git
    ref: main
resources:
  - id: manafield-r2
    provider: s3
    config:
      endpoint: https://example.r2.cloudflarestorage.com
      bucket: media-test
deployment:
  modulesNetwork: modules
  edgeNetwork: edge
"#;
    assert!(validate_yaml(definition).is_ok());
    assert!(validate_yaml(&definition.replace("      bucket: media-test\n", "")).is_err());
    assert!(
        validate_yaml(&definition.replace(
            "      bucket: media-test",
            "      bucket: media-test\n      secretAccessKey: never-store-secrets-in-YAML"
        ))
        .is_err()
    );
    assert!(validate_yaml(&definition.replace("provider: s3", "provider: postgresql")).is_err());
}

#[test]
fn ci_plan_emits_s3_connection_metadata_without_credentials() {
    let definition: InstanceDefinition = serde_yaml_ng::from_str(
        r#"
version: 0
instance:
  id: storage-test
core:
  source:
    repository: https://example.invalid/core.git
    ref: main
resources:
  - id: manafield-r2
    provider: s3
    config:
      endpoint: https://account.r2.cloudflarestorage.com
      bucket: media-bucket
      region: auto
      forcePathStyle: true
deployment:
  modulesNetwork: modules
  edgeNetwork: edge
"#,
    )
    .unwrap();
    let plan = BuildPlan::from(definition);
    let dir = std::env::temp_dir().join(format!(
        "manafield-s3-ci-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    ci_output::write_ci_plan(&plan, &dir).unwrap();
    let metadata = fs::read_to_string(dir.join("s3-resources.tsv")).unwrap();
    assert_eq!(
        metadata,
        "manafield-r2\thttps://account.r2.cloudflarestorage.com\tmedia-bucket\tauto\ttrue\t-\n"
    );
    assert_eq!(
        fs::read_to_string(dir.join("resources.tsv")).unwrap(),
        "manafield-r2\ts3\n"
    );
    assert!(!metadata.to_lowercase().contains("secret"));
    fs::remove_dir_all(dir).unwrap();
}

fn exclusive_definition() -> InstanceDefinition {
    let mut definition = definition_with_modules(
        r#"  - id: noctemblith
    source: {type: dir}
    build: {type: docker, context: ., dockerfile: Dockerfile}
    bindings: {cache: noctemblith-redis}"#,
    );
    definition.resources.push(ResourceDefinition {
        id: "noctemblith-redis".into(),
        enabled: true,
        provider: "redis".into(),
        exclusive_to: Some(ExclusiveConsumer {
            module: "noctemblith".into(),
            slot: "cache".into(),
        }),
        config: None,
    });
    definition
}

#[test]
fn exclusive_resource_preserves_contract_in_plan_and_ci() {
    let definition = exclusive_definition();
    validate_definition(&definition).unwrap();
    let plan = BuildPlan::from(definition);
    let json = serde_json::to_value(&plan).unwrap();
    assert_eq!(json["resources"][0]["exclusiveTo"]["module"], "noctemblith");
    assert_eq!(json["resources"][0]["exclusiveTo"]["slot"], "cache");
    let dir = std::env::temp_dir().join(format!(
        "manafield-exclusive-ci-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    ci_output::write_ci_plan(&plan, &dir).unwrap();
    assert_eq!(
        fs::read_to_string(dir.join("resource-exclusive-consumers.tsv")).unwrap(),
        "noctemblith-redis\tnoctemblith\tcache\n"
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn redis_requires_explicit_exclusive_consumer() {
    let mut definition = exclusive_definition();
    definition.resources[0].exclusive_to = None;
    assert!(
        validate_definition(&definition)
            .unwrap_err()
            .contains("requires exclusiveTo")
    );
}

#[test]
fn exclusive_resource_rejects_other_module_even_when_disabled() {
    for enabled in [true, false] {
        let mut definition = exclusive_definition();
        let mut other = definition_with_modules(
            r#"  - id: other
    source: {type: dir}
    build: {type: docker, context: ., dockerfile: Dockerfile}
    bindings: {cache: noctemblith-redis}"#,
        )
        .modules
        .remove(0);
        other.enabled = enabled;
        definition.modules.push(other);
        assert!(
            validate_definition(&definition)
                .unwrap_err()
                .contains("not permitted")
        );
    }
}

#[test]
fn exclusive_resource_rejects_second_slot_and_core_binding() {
    let mut definition = exclusive_definition();
    definition.modules[0]
        .bindings
        .insert("otherCache".into(), "noctemblith-redis".into());
    assert!(
        validate_definition(&definition)
            .unwrap_err()
            .contains("not permitted")
    );
    let mut definition = exclusive_definition();
    definition
        .core
        .bindings
        .insert("loggingState".into(), "noctemblith-redis".into());
    assert!(
        validate_definition(&definition)
            .unwrap_err()
            .contains("cannot be bound by Core")
    );
}

#[test]
fn exclusive_resource_requires_enabled_owner_and_matching_binding() {
    let mut definition = exclusive_definition();
    definition.modules[0].enabled = false;
    assert!(
        validate_definition(&definition)
            .unwrap_err()
            .contains("enabled Module")
    );
    let mut definition = exclusive_definition();
    definition.modules[0].bindings.clear();
    assert!(
        validate_definition(&definition)
            .unwrap_err()
            .contains("owner must bind")
    );
    let mut definition = exclusive_definition();
    definition.resources[0]
        .exclusive_to
        .as_mut()
        .unwrap()
        .module = "missing".into();
    assert!(
        validate_definition(&definition)
            .unwrap_err()
            .contains("enabled Module")
    );
}

#[test]
fn exclusive_consumer_is_generic_and_unrestricted_postgres_stays_compatible() {
    let mut definition = exclusive_definition();
    definition.resources[0].provider = "postgresql".into();
    validate_definition(&definition).unwrap();
    definition.resources[0].exclusive_to = None;
    definition
        .core
        .bindings
        .insert("loggingState".into(), "noctemblith-redis".into());
    validate_definition(&definition).unwrap();
    let json = serde_json::to_value(BuildPlan::from(definition)).unwrap();
    assert!(json["resources"][0].get("exclusiveTo").is_none());
}

#[test]
fn malformed_exclusive_policy_is_rejected() {
    for value in [
        serde_json::json!({"module":"noctemblith"}),
        serde_json::json!({"module":"noctemblith","slot":"cache","allowOthers":true}),
        serde_json::json!({"module":"noctemblith","slot":["cache"]}),
        serde_json::json!([]),
    ] {
        assert!(serde_json::from_value::<ExclusiveConsumer>(value).is_err());
    }
    for (module, slot) in [
        ("", "cache"),
        ("../other", "cache"),
        ("noctemblith", ""),
        ("noctemblith", "bad.slot"),
    ] {
        assert!(
            ExclusiveConsumer {
                module: module.into(),
                slot: slot.into()
            }
            .validate()
            .is_err()
        );
    }
}
