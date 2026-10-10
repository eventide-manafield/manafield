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
