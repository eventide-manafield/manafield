#!/usr/bin/env python3
"""Connect native Noctemblith using the shared CLI; no Jenkins installation required.
Run as the authorized Docker/Instance-secret operator. No ingress publication or retirement.
"""
import argparse
import copy
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import time

SOURCE = Path(__file__).resolve().parents[1]
WORKSPACE = SOURCE.parent
INSTANCE = WORKSPACE / "instance"
NATIVE = WORKSPACE / "noctemblith-native-redis-20261016"
CANDIDATE = INSTANCE / "instance.candidate-noctemblith-native-20261016.yaml"
YAML_LIBRARY = NATIVE / "packages/backend/node_modules/js-yaml"
CLI = SOURCE / "target/debug/manafield"


def run(args, *, cwd=None, capture=False, env=None, input=None):
    return subprocess.run([str(x) for x in args], cwd=cwd, env=env, input=input, text=True,
                          check=True, stdout=subprocess.PIPE if capture else None,
                          stderr=subprocess.PIPE if capture else None)


def load_yaml(path):
    code = "const fs=require('fs'),y=require(process.argv[1]);process.stdout.write(JSON.stringify(y.load(fs.readFileSync(process.argv[2],'utf8'))));"
    return json.loads(run(["node", "-e", code, YAML_LIBRARY, path], capture=True).stdout)


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")
    path.chmod(0o600)


def snapshot(repo, destination):
    data = subprocess.run(["git", "-c", "safe.directory=" + str(repo), "-C", str(repo), "archive", "HEAD"],
                          capture_output=True, check=True).stdout
    destination.mkdir()
    with tarfile.open(fileobj=io.BytesIO(data)) as archive:
        archive.extractall(destination, filter="data")


def docker_inventory():
    ids = run(["docker", "ps", "-q"], capture=True).stdout.split()
    if not ids:
        raise RuntimeError("No running Docker services")
    items = json.loads(run(["docker", "inspect", *ids], capture=True).stdout)
    # Never write or print raw inspect output/environment values.
    return {item["Name"].lstrip("/"): item for item in items}


def pg_count(container, schema, database, username):
    sql = "SELECT count(*) FROM information_schema.tables WHERE table_schema='" + schema + "'"
    return int(run(["docker", "exec", container, "psql", "-U", username, "-d", database,
                    "-tAc", sql], capture=True).stdout.strip())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--preflight", action="store_true", help="read-only Docker/configuration checks")
    args = parser.parse_args()
    if not CLI.is_file():
        raise RuntimeError("Build the shared CLI binary first")
    live = docker_inventory()
    required = ["manafield-core-1", "manafield-manafield-postgres-1",
                "manafield-manafield-postgres-provider-1",
                "noctemblith-dev-noctemblith-1"]
    if any(name not in live for name in required):
        raise RuntimeError("Expected existing deployment differs; refresh inventory before proceeding")
    core = live["manafield-core-1"]
    labels = core["Config"]["Labels"]
    if labels.get("com.docker.compose.project") != "manafield":
        raise RuntimeError("Unexpected Core Compose project")
    core_files = labels["com.docker.compose.project.config_files"].split(",")
    base_path = Path(core_files[0])
    modules_path = Path(core_files[1])
    current = base_path.parent
    if not (current / "release.env").is_file():
        raise RuntimeError("Existing Core Release environment is unavailable")
    if run(["git", "-c", "safe.directory=" + str(SOURCE), "-C", str(SOURCE), "status", "--porcelain"], capture=True).stdout.strip():
        raise RuntimeError("Commit the shared CLI source before generating build provenance")
    if run(["git", "-c", "safe.directory=" + str(NATIVE), "-C", str(NATIVE), "status", "--porcelain"], capture=True).stdout.strip():
        raise RuntimeError("Native source has uncommitted changes")

    definition = load_yaml(CANDIDATE)
    resource = next(r for r in definition["resources"] if r["id"] == "noctemblith-redis")
    if resource.get("exclusiveTo") != {"module": "noctemblith", "slot": "cache"}:
        raise RuntimeError("Native Redis exclusive consumer does not match")
    native_module = next(m for m in definition["modules"] if m["id"] == "noctemblith")
    if native_module["bindings"] != {"state": "manafield-postgres", "media": "manafield-r2", "cache": "noctemblith-redis"}:
        raise RuntimeError("Native Resource bindings do not match")
    pg = live["manafield-manafield-postgres-1"]
    pg_env = dict(v.split("=", 1) for v in pg["Config"]["Env"] if "=" in v)
    database, username = pg_env["POSTGRES_DB"], pg_env["POSTGRES_USER"]
    if database != "manafield" or username != "manafield":
        raise RuntimeError("Shared PostgreSQL database differs from the binding contract")
    for name in ["access-key-id", "secret-access-key"]:
        secret = INSTANCE / "secrets/s3/manafield-r2" / name
        if secret.is_symlink() or not secret.is_file() or not os.access(secret, os.R_OK):
            raise RuntimeError("Run with permission to read the existing private Instance R2 files")
    sso_keys = ["NOCTEMBLITH_MANAFIELD_SSO_ENABLED", "NOCTEMBLITH_MANAFIELD_ACCOUNT_ORIGIN",
                "NOCTEMBLITH_MANAFIELD_OAUTH_CLIENT_ID", "NOCTEMBLITH_MANAFIELD_OAUTH_CALLBACK_URL"]
    old_env = dict(v.split("=", 1) for v in live["noctemblith-dev-noctemblith-1"]["Config"]["Env"] if "=" in v)
    sso = {key: old_env[key] for key in sso_keys if key in old_env}
    if len(sso) != 4 or sso[sso_keys[0]] != "true":
        raise RuntimeError("Existing native SSO configuration is incomplete")
    if sso[sso_keys[3]] != "https://" + native_module["exposure"]["host"] + "/auth/manafield/callback":
        raise RuntimeError("Existing OAuth callback and proposed native public origin differ")
    public_before = pg_count("manafield-manafield-postgres-1", "public", database, username)
    native_tables = pg_count("manafield-manafield-postgres-1", "mf_noctemblith_state", database, username)
    migrations = sorted((NATIVE / "packages/backend/migration").glob("*.js"))
    if [m.name for m in migrations] != ["1792170000000-NoctemblithBaseline.js"]:
        raise RuntimeError("Unexpected native migration chain")
    if native_tables:
        raise RuntimeError("Target native schema already contains tables; no reset or automatic migration-history conversion is allowed")
    print("PASS: existing Core/PG, explicit bindings, SSO metadata and empty native schema", flush=True)
    if args.preflight:
        return 0

    stamp = time.strftime("%Y%m%dT%H%M%SZ", time.gmtime())
    release = INSTANCE / "releases" / ("native-" + stamp)
    release.mkdir(mode=0o750)
    print("Preparing native connection Release:", release, flush=True)
    source_sha = run(["git", "-c", "safe.directory=" + str(SOURCE), "-C", SOURCE, "rev-parse", "HEAD"], capture=True).stdout.strip()
    native_sha = run(["git", "-c", "safe.directory=" + str(NATIVE), "-C", NATIVE, "rev-parse", "HEAD"], capture=True).stdout.strip()
    build = release / "build-workspace"
    native_source = release / "native-source"
    snapshot(SOURCE, build)
    snapshot(NATIVE, native_source)
    # Supply the app's documented image metadata input in the generated build recipe.
    dockerfile = native_source / "Dockerfile"
    text = dockerfile.read_text()
    if text.count("ARG NOCTEMBLITH_BUILD_SHA=unknown") != 1:
        raise RuntimeError("Native build metadata contract changed")
    dockerfile.write_text(text.replace("ARG NOCTEMBLITH_BUILD_SHA=unknown",
                                      "ARG NOCTEMBLITH_BUILD_SHA=" + native_sha))
    app_image = "manafield-module-noctemblith:" + native_sha[:12]
    (build / "ci-plan").mkdir(exist_ok=True)
    (build / "ci-plan/resources.tsv").write_text("manafield-r2\ts3\nnoctemblith-redis\tredis\n")
    (build / "module-sources.tsv").write_text(
        "\t".join(["noctemblith", "noctemblith", str(native_source), native_sha[:12],
                   app_image, "docker", ".", "Dockerfile"]) + "\n")
    print("Building through the shared CLI (this can take several minutes).", flush=True)
    run([CLI, "build", build, source_sha[:12], "manafield-core:native-builder-" + source_sha[:12]])
    shutil.copy(build / "resolved-images.env", release / "resolved-images.env")

    # Preserve the existing project's configuration and actual pinned images.
    base = load_yaml(base_path)
    modules = load_yaml(modules_path)
    template = load_yaml(SOURCE / "deploy/compose.yml")
    for name in ["manafield-s3-provider", "manafield-redis", "manafield-redis-provider"]:
        base["services"][name] = template["services"][name]
    base.setdefault("volumes", {})["manafield-redis-data"] = {}
    for item in live.values():
        lab = item["Config"].get("Labels") or {}
        if lab.get("com.docker.compose.project") == "manafield":
            name = lab.get("com.docker.compose.service")
            target = base["services"].get(name) or modules["services"].get(name)
            if target is not None:
                target["image"] = item["Image"]
    base["name"] = "manafield"
    mounts = core.get("Mounts", [])
    existing_modules = next(Path(m["Source"]) for m in mounts if m["Destination"] == "/var/lib/manafield/modules")
    shutil.copytree(existing_modules, release / "modules")
    (release / "modules/noctemblith").mkdir(exist_ok=True)
    shutil.copy(native_source / "manafield.module.json", release / "modules/noctemblith/manafield.module.json")
    modules["services"]["module-noctemblith"] = {
        "image": app_image, "restart": "unless-stopped", "init": True,
        "environment": dict(sso, PORT="8080", MANAFIELD_MODULE_ID="noctemblith",
                            MANAFIELD_CORE_URL="http://core:8080", MANAFIELD_WEB_BASE_PATH="/"),
        "depends_on": {"core": {"condition": "service_healthy"}},
        "read_only": True, "tmpfs": ["/tmp"], "cap_drop": ["ALL"],
        "security_opt": ["no-new-privileges:true"], "networks": {"modules": {}},
    }
    write_json(release / "compose.yml", base)
    write_json(release / "modules.compose.yml", modules)
    shutil.copy(current / "postgresql-allocations.tsv", release / "postgresql-allocations.tsv")
    shutil.copy(current / "release.env", release / "release.env")
    (release / "release.env").chmod(0o600)
    with (release / "release.env").open("a") as envfile:
        envfile.write("\nMANAFIELD_MODULES_PATH=" + str(release / "modules") + "\n")
        envfile.write("MANAFIELD_POSTGRES_ALLOCATIONS_FILE_HOST=" + str(release / "postgresql-allocations.tsv") + "\n")
    shutil.copy(current / "compose-profiles.txt", release / "compose-profiles.txt")
    shutil.copy(CANDIDATE, release / "instance.yaml")
    run([CLI, "plan", release / "instance.yaml", "--output", release / "build-plan.json"])
    run([CLI, "stage-resources", "--definition", release / "instance.yaml",
         "--instance-root", INSTANCE, "--release-dir", release])
    profiles = (release / "compose-profiles.txt").read_text().strip()
    env = dict(os.environ, COMPOSE_PROFILES=profiles)
    compose = ["docker", "compose", "--env-file", release / "release.env",
               "-f", release / "compose.yml", "-f", release / "modules.compose.yml"]
    run(compose + ["config", "--quiet"], env=env, capture=True)
    write_json(release / "native-connection-recipe.json", {
        "source_revision": source_sha, "native_revision": native_sha,
        "preserved_core_image": core["Image"], "original_core_files": core_files,
        "targets_only": ["core", "manafield-postgres-provider", "manafield-s3-provider",
                         "manafield-redis", "manafield-redis-provider", "module-noctemblith"],
        "ingress_published": False,
    })
    # Shared apply executes only named services, without deps or orphan removal.
    services = ["core", "manafield-postgres-provider", "manafield-s3-provider",
                "manafield-redis", "manafield-redis-provider"]
    command = [CLI, "apply", release]
    for service in services:
        command.extend(["--service", service])
    run(command)
    check = """const {loadConfig}=await import('/misskey/packages/backend/built/config.js');
const {default:pg}=await import('/misskey/packages/backend/node_modules/pg/lib/index.js');
try { const c=loadConfig(); const db=new pg.Client({host:c.db.host,port:c.db.port,
 database:c.db.db,user:c.db.user,password:c.db.pass}); await db.connect();
 await db.query('SELECT 1');await db.end();console.log('PASS: native binding files and PostgreSQL login');}
catch {process.exit(41);}"""
    for _ in range(12):
        try:
            run(compose + ["run", "--rm", "--no-deps", "--entrypoint", "node",
                          "module-noctemblith", "--input-type=module", "-e", check], env=env, capture=True)
            break
        except subprocess.CalledProcessError:
            time.sleep(2)
    else:
        raise RuntimeError("Native bound credentials/PG allocation check failed; do not publish ingress")
    # The archived source must contain exactly the reviewed initial migration.
    migrations = sorted((native_source / "packages/backend/migration").glob("*.js"))
    if [m.name for m in migrations] != ["1792170000000-NoctemblithBaseline.js"]:
        raise RuntimeError("Unexpected native migration chain")
    run(compose + ["run", "--rm", "--no-deps", "module-noctemblith", "pnpm", "migrate"], env=env)
    run([CLI, "apply", release, "--service", "module-noctemblith"])
    for _ in range(60):
        try:
            run(compose + ["exec", "-T", "module-noctemblith", "curl", "--fail", "--silent",
                           "http://127.0.0.1:8080/healthz"], env=env, capture=True)
            break
        except subprocess.CalledProcessError:
            time.sleep(2)
    else:
        raise RuntimeError("Native SNS health did not pass; old public route remains in place")
    if pg_count("manafield-manafield-postgres-1", "public", database, username) != public_before:
        raise RuntimeError("Unexpected public-schema table count change; inspect before proceeding")
    status = json.loads(run(compose + ["exec", "-T", "module-noctemblith", "curl", "--fail", "--silent",
                                     "http://127.0.0.1:8080/auth/manafield/status"], env=env, capture=True).stdout)
    if status.get("enabled") is not True:
        raise RuntimeError("Native SSO is not enabled; do not publish ingress")
    expected_resources = {
        "manafield-postgres": "database.postgresql",
        "manafield-r2": "storage.object",
        "noctemblith-redis": "cache.redis",
    }
    for attempt in range(20):
        try:
            registered = json.loads(run(compose + ["exec", "-T", "module-noctemblith",
                "curl", "--fail", "--silent", "http://core:8080/resources"],
                env=env, capture=True).stdout)
            by_id = {r["id"]: r for r in registered}
            ready = all(any(c.get("id") == capability and c.get("version") == "1.0.0"
                            for c in by_id.get(resource, {}).get("provides", {}).get("capabilities", []))
                        for resource, capability in expected_resources.items())
            if ready:
                break
        except (subprocess.CalledProcessError, ValueError, KeyError, TypeError):
            pass
        time.sleep(2)
    else:
        raise RuntimeError("Live Core Resource registration incomplete; do not publish ingress")
    print("PASS: live Core registered PostgreSQL, S3 and exclusive Redis Resources", flush=True)
    write_json(release / "native-health-result.json",
               {"health":"passed","sso_enabled":True,"resources_registered":True,"public_tables_unchanged":True,"ingress_published":False})
    print("PASS: native Noctemblith started with PG18 + R2 + exclusive Redis; SSO enabled", flush=True)
    print("Release:", release)
    print("Public route still targets the old SNS. Browser SSO/media E2E and cutover remain separate.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except RuntimeError as error:
        print("Native connection stopped: " + str(error), file=sys.stderr)
        sys.exit(1)
    except (OSError, subprocess.CalledProcessError, KeyError, StopIteration):
        print("Native connection stopped. No secret values are printed. Keep existing data/volumes and do not publish ingress on failure.", file=sys.stderr)
        sys.exit(1)
