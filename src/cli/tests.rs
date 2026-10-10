use super::*;

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn no_arguments_prints_help() {
    assert_eq!(parse_args(args(&[])).unwrap(), Command::Help);
}

#[test]
fn serve_command_points_to_separate_binary() {
    assert!(matches!(
        parse_args(args(&["serve"])),
        Err(CliError::Usage(_))
    ));
}

#[test]
fn parses_bindings_export() {
    assert_eq!(
        parse_args(args(&[
            "bindings",
            "export",
            "--plan",
            "build-plan.json",
            "--output",
            "bindings.json"
        ]))
        .unwrap(),
        Command::ExportBindings {
            plan: "build-plan.json".to_owned(),
            output: "bindings.json".to_owned(),
        }
    );
    for invalid in [
        vec!["bindings", "export", "--plan", "plan.json"],
        vec!["bindings", "export", "--output", "bindings.json"],
        vec![
            "bindings", "export", "--plan", "a", "--output", "b", "--plan", "c",
        ],
    ] {
        assert!(matches!(
            parse_args(args(&invalid)),
            Err(CliError::Usage(_))
        ));
    }
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
fn parses_plan_with_outputs() {
    assert_eq!(
        parse_args(args(&[
            "plan",
            "/tmp/instance.yaml",
            "--output",
            "/tmp/build-plan.json",
            "--ci-output",
            "/tmp/ci-plan",
        ]))
        .unwrap(),
        Command::Plan {
            input: "/tmp/instance.yaml".to_owned(),
            output: Some("/tmp/build-plan.json".to_owned()),
            ci_output_dir: Some("/tmp/ci-plan".to_owned()),
        }
    );
}

#[test]
fn plan_defaults_to_instance_yaml() {
    assert_eq!(
        parse_args(args(&["plan"])).unwrap(),
        Command::Plan {
            input: "instance.yaml".to_owned(),
            output: None,
            ci_output_dir: None,
        }
    );
}

#[test]
fn parses_direct_deploy_options() {
    assert_eq!(
        parse_args(args(&[
            "deploy",
            "--instance-root",
            "/instance",
            "--source",
            "/repo",
            "--modules-root",
            "/local"
        ]))
        .unwrap(),
        Command::DeployRelease {
            id: None,
            instance_root: Some("/instance".into()),
            staged: None,
            snapshot_only: false,
            source: Some("/repo".into()),
            modules_root: Some("/local".into()),
        }
    );
    assert!(parse_args(args(&["deploy", "--source"])).is_err());
    assert!(parse_args(args(&["deploy", "--modules-root"])).is_err());
    assert!(parse_args(args(&["deploy", "--source", "a", "--source", "b"])).is_err());
}

#[test]
fn parses_release_deploy_and_legacy_deploy_separately() {
    assert_eq!(
        parse_args(args(&[
            "deploy",
            "v3_20261008T070000Z",
            "--instance-root",
            "/tmp/instance",
            "--snapshot-only"
        ]))
        .unwrap(),
        Command::DeployRelease {
            id: Some("v3_20261008T070000Z".to_owned()),
            instance_root: Some("/tmp/instance".to_owned()),
            staged: None,
            snapshot_only: true,
            source: None,
            modules_root: None,
        }
    );
    assert_eq!(
        parse_args(args(&["deploy", "/tmp/staged-release"])).unwrap(),
        Command::Deploy {
            release_dir: "/tmp/staged-release".to_owned()
        }
    );
    assert!(
        parse_args(args(&[
            "deploy",
            "v3_20261008T070000Z",
            "--snapshot-only",
            "--staged-dir",
            "/tmp/staged"
        ]))
        .is_err()
    );
    assert!(parse_args(args(&["deploy", "v3_20261008T070000Z", "unexpected"])).is_err());
}

#[test]
fn parses_module_bind() {
    assert_eq!(
        parse_args(args(&[
            "module",
            "bind",
            "echo",
            "state",
            "main-postgres",
            "--instance-root",
            "/tmp/i"
        ]))
        .unwrap(),
        Command::ModuleBind {
            consumer: "echo".to_owned(),
            slot: "state".to_owned(),
            target: "main-postgres".to_owned(),
            instance_root: Some("/tmp/i".to_owned()),
        }
    );
    assert!(parse_args(args(&["module", "bind", "echo", "state"])).is_err());
    assert!(parse_args(args(&["module", "unknown"])).is_err());
    assert!(parse_args(args(&["module", "bind", "echo", "state", "db", "extra"])).is_err());
}

#[test]
fn parses_use_selection_and_status() {
    assert_eq!(
        parse_args(args(&[
            "use",
            "v1_20261008T070000Z",
            "--instance-root",
            "/tmp/i"
        ]))
        .unwrap(),
        Command::Use {
            release_id: Some("v1_20261008T070000Z".to_owned()),
            instance_root: Some("/tmp/i".to_owned())
        }
    );
    assert_eq!(
        parse_args(args(&["use"])).unwrap(),
        Command::Use {
            release_id: None,
            instance_root: None
        }
    );
    assert!(parse_args(args(&["use", "--instance-root"])).is_err());
}

#[test]
fn parses_platform_build_all() {
    assert_eq!(
        parse_args(args(&[
            "build",
            "all",
            "--source",
            "/src",
            "--output",
            "/tmp/dist",
            "--no-docker"
        ]))
        .unwrap(),
        Command::BuildAll {
            source: "/src".to_owned(),
            output: Some("/tmp/dist".to_owned()),
            no_docker: true
        }
    );
    assert_eq!(
        parse_args(args(&["build", "all"])).unwrap(),
        Command::BuildAll {
            source: ".".to_owned(),
            output: None,
            no_docker: false
        }
    );
}

#[test]
fn rejects_unknown_or_duplicate_platform_options() {
    assert!(parse_args(args(&["build", "all", "--no-cache"])).is_err());
    assert!(parse_args(args(&["build", "all", "--output"])).is_err());
    assert!(parse_args(args(&["build", "all", "--no-docker", "--no-docker"])).is_err());
}

#[test]
fn parses_build_arguments() {
    assert_eq!(
        parse_args(args(&[
            "build",
            "/tmp/workspace",
            "abcdef",
            "manafield-core:abcdef"
        ]))
        .unwrap(),
        Command::Build {
            workspace: "/tmp/workspace".to_owned(),
            revision: "abcdef".to_owned(),
            core_image: "manafield-core:abcdef".to_owned(),
        }
    );
}

#[test]
fn parses_deploy_release_directory() {
    assert_eq!(
        parse_args(args(&["deploy", "/tmp/release 123"])).unwrap(),
        Command::Deploy {
            release_dir: "/tmp/release 123".to_owned(),
        }
    );
}

#[test]
fn deploy_supports_generated_ids_but_rejects_invalid_legacy_args() {
    assert_eq!(
        parse_args(args(&["deploy"])).unwrap(),
        Command::DeployRelease {
            id: None,
            instance_root: None,
            staged: None,
            snapshot_only: false,
            source: None,
            modules_root: None,
        }
    );
    assert!(matches!(
        parse_args(args(&["deploy", "/tmp/release", "extra"])),
        Err(CliError::Usage(_))
    ));
    assert!(matches!(
        parse_args(args(&["deploy", "--unexpected"])),
        Err(CliError::Usage(_))
    ));
}

#[test]
fn validates_extension_namespace() {
    for accepted in ["account", "echo", "my-module", "v2"] {
        assert!(valid_extension_namespace(accepted), "{accepted}");
    }
    for rejected in [
        "",
        "-bad",
        "Account",
        "../bad",
        "echo/../../bin/sh",
        "abc.def",
        "📦",
    ] {
        assert!(!valid_extension_namespace(rejected), "{rejected}");
    }
}

#[test]
fn parses_log_command_without_accessing_core_http() {
    let parsed = parse_args(args(&["log", "--level", "warn", "--source", "account"])).unwrap();
    match parsed {
        Command::Log(opts) => {
            assert_eq!(opts.level.as_deref(), Some("warn"));
            assert_eq!(opts.source.as_deref(), Some("account"));
        }
        _ => panic!("log command not registered"),
    }
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
