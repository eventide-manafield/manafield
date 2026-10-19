use super::*;
use std::path::PathBuf;
struct Fixture {
    root: PathBuf,
    release: PathBuf,
    definition: PathBuf,
    plan: Value,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn fixture() -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "mf-shared-stage-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let release = root.join("release");
    let pg_secret = root.join("secrets/postgresql/pg/mf_app_state.password");
    super::super::secrets::create_secret(&pg_secret).unwrap();
    fs::create_dir_all(release.join("modules/app")).unwrap();
    let storage = root.join("secrets/s3/r2");
    fs::create_dir_all(&storage).unwrap();
    fs::write(storage.join("access-key-id"), "test-r2-id-unique-value\n").unwrap();
    fs::write(
        storage.join("secret-access-key"),
        "test-r2-secret-unique-value\n",
    )
    .unwrap();
    let definition = root.join("instance.yaml");
    let input = json!({"version":0,"instance":{"id":"demo"},
  "core":{"source":{"repository":"https://example.test/core.git","ref":"main"}},
  "modules":[{"id":"app","source":{"type":"dir"},"build":{"type":"docker","context":".","dockerfile":"Dockerfile"},
   "bindings":{"state":"pg","media":"r2","cache":"app-redis"},
   "exposure":{"type":"host","host":"app.example.test","targetPort":8080}}],
  "resources":[{"id":"pg","provider":"postgresql"},{"id":"r2","provider":"s3",
   "config":{"endpoint":"https://example.r2.cloudflarestorage.com","bucket":"test-media","region":"auto","forcePathStyle":true,"publicBaseUrl":"https://media.example.test"}},
   {"id":"app-redis","provider":"redis","exclusiveTo":{"module":"app","slot":"cache"}}],
  "deployment":{"modulesNetwork":"modules","edgeNetwork":"edge","ingress":{"provider":"traefik","output":"/tmp/candidate.yml"}}});
    fs::write(&definition, serde_json::to_vec(&input).unwrap()).unwrap();
    let plan = crate::build_plan::plan_value(&definition).unwrap();
    fs::write(
        release.join("build-plan.json"),
        serde_json::to_vec(&plan).unwrap(),
    )
    .unwrap();
    fs::write(release.join("modules/app/manafield.module.json"),serde_json::to_vec(&json!({
  "id":"app","requires":{"capabilities":{"state":{"id":"database.postgresql","version":"^1.0.0"},
   "media":{"id":"storage.object","version":"^1.0.0"},"cache":{"id":"cache.redis","version":"^1.0.0"}}}
 })).unwrap()).unwrap();
    fs::write(
        release.join("compose.yml"),
        include_str!("../../../../deploy/compose.yml"),
    )
    .unwrap();
    fs::write(release.join("modules.compose.yml"),serde_json::to_vec(&json!({"services":{"module-app":{
  "image":"native-app:pinned","environment":{"PRESERVED":"value","MANAFIELD_BINDING_STATE_CONFIG_SCHEMA":"mf_app_state"},
  "volumes":[{"type":"bind","source":pg_secret,"target":"/run/manafield/bindings/state/password","read_only":true}],
  "depends_on":{"core":{"condition":"service_healthy"}}}}})).unwrap()).unwrap();
    fs::write(
        release.join("release.env"),
        "MANAFIELD_CORE_IMAGE=core:pinned\nUNCHANGED=value\n",
    )
    .unwrap();
    fs::write(release.join("compose-profiles.txt"), "postgresql\n").unwrap();
    fs::write(release.join("resolved-images.env"),"resource.r2.provider.image=s3:pinned\nresource.app-redis.provider.image=redis-provider:pinned\nresource.app-redis.server.image=redis-server:pinned\n").unwrap();
    Fixture {
        root,
        release,
        definition,
        plan,
    }
}
#[test]
fn shared_stage_preserves_existing_settings_and_has_no_secret_values() {
    let f = fixture();
    run(
        f.definition.to_str().unwrap(),
        f.root.to_str().unwrap(),
        f.release.to_str().unwrap(),
    )
    .unwrap();
    let modules = yaml_json(&f.release.join("modules.compose.yml")).unwrap();
    let service = &modules["services"]["module-app"];
    assert_eq!(service["image"], "native-app:pinned");
    assert_eq!(service["environment"]["PRESERVED"], "value");
    assert_eq!(
        service["environment"]["MANAFIELD_BINDING_STATE_CONFIG_SCHEMA"],
        "mf_app_state"
    );
    assert_eq!(
        service["environment"]["MANAFIELD_BINDING_CACHE_TARGET"],
        "app-redis"
    );
    assert_eq!(
        service["environment"]["MANAFIELD_BINDING_MEDIA_TARGET"],
        "r2"
    );
    assert_eq!(
        service["environment"]["MANAFIELD_MODULE_PUBLIC_URL"],
        "https://app.example.test"
    );
    assert_eq!(service["volumes"].as_array().unwrap().len(), 4);
    assert_eq!(
        service["depends_on"]["core"]["condition"],
        "service_healthy"
    );
    let compose = yaml_json(&f.release.join("compose.yml")).unwrap();
    assert_eq!(compose["name"], "manafield");
    assert_eq!(
        compose["volumes"]["manafield-redis-data"]["name"],
        "manafield-redis-app-redis"
    );
    assert_eq!(
        fs::read_to_string(f.release.join("compose-profiles.txt")).unwrap(),
        "postgresql,s3,redis\n"
    );
    for name in [
        "release.env",
        "modules.compose.yml",
        "resource-materialization.json",
    ] {
        let text = fs::read_to_string(f.release.join(name)).unwrap();
        assert!(
            !text.contains("test-r2-id-unique-value")
                && !text.contains("test-r2-secret-unique-value")
        );
    }
    verify(&f.plan, &f.release).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(f.release.join("release.env"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(f.root.join("secrets/s3/r2/runtime/access-key-id"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o444
        );
    }
}
#[test]
fn preparation_is_idempotent_and_direct_image_metadata_works() {
    let f = fixture();
    augment(&f.plan, &f.root, &f.release).unwrap();
    let first = fs::read(f.release.join("modules.compose.yml")).unwrap();
    fs::remove_file(f.release.join("resolved-images.env")).unwrap();
    augment(&f.plan, &f.root, &f.release).unwrap();
    assert_eq!(
        first,
        fs::read(f.release.join("modules.compose.yml")).unwrap()
    );
    assert_eq!(
        fs::read_to_string(f.release.join("compose-profiles.txt")).unwrap(),
        "postgresql,s3,redis\n"
    );
}
#[test]
fn wrong_type_fails_before_secret_allocation_or_output_change() {
    let f = fixture();
    fs::write(f.release.join("modules/app/manafield.module.json"),
  r#"{"id":"app","requires":{"capabilities":{"cache":{"id":"storage.object","version":"^1.0.0"}}}}"#).unwrap();
    let before = fs::read(f.release.join("modules.compose.yml")).unwrap();
    assert!(augment(&f.plan, &f.root, &f.release).is_err());
    assert_eq!(
        before,
        fs::read(f.release.join("modules.compose.yml")).unwrap()
    );
    assert!(!f.root.join("secrets/redis").exists());
    assert!(!f.root.join("secrets/s3/r2/runtime").exists());
    assert!(!f.release.join("resource-materialization.json").exists());
}
#[test]
fn changed_definition_and_incomplete_release_fail_closed() {
    let f = fixture();
    assert!(verify(&f.plan, &f.release).is_err());
    let text = fs::read_to_string(&f.definition)
        .unwrap()
        .replace("app.example.test", "other.example.test");
    fs::write(&f.definition, text).unwrap();
    assert!(
        run(
            f.definition.to_str().unwrap(),
            f.root.to_str().unwrap(),
            f.release.to_str().unwrap()
        )
        .is_err()
    );
    assert!(!f.root.join("secrets/redis").exists());
    verify(
        &json!({"resources":[{"id":"pg","provider":"postgresql"}]}),
        &f.release,
    )
    .unwrap();
}
