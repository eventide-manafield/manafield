#!/usr/bin/env python3
"""Publish the already-running native app; retain all old data/volumes."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import stat
import sys
import time
import uuid
from urllib.parse import parse_qs, urlsplit

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("native_connect", Path(__file__).with_name("noctemblith-native-connect.py"))
shared = importlib.util.module_from_spec(spec)
spec.loader.exec_module(shared)
ROUTE = Path("/home/nayuz/infra/traefik/dynamic/noctemblith-dev.yml")
HOST = "noctemblith-dev.manafield.studio"
APP = "manafield-module-noctemblith-1"
ALIAS = "noctemblith-native"
EDGE = "manafield-edge"


def atomic(path, data, mode):
    temporary = path.with_name("." + path.name + "." + uuid.uuid4().hex + ".tmp")
    try:
        fd = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL, mode)
        with os.fdopen(fd, "wb") as file:
            file.write(data)
            file.flush()
            os.fsync(file.fileno())
        os.chmod(temporary, mode)
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def json_bytes(value):
    return (json.dumps(value, indent=2) + "\n").encode()


def app_get(path):
    return shared.run(["docker", "exec", APP, "curl", "--fail", "--silent",
                       "--show-error", "--max-time", "15", "http://127.0.0.1:8080" + path],
                      capture=True).stdout


def public_get(path):
    return shared.run(["curl", "--fail", "--silent", "--show-error", "--max-time", "15",
                       "-H", "Cache-Control: no-cache", "https://" + HOST + path +
                       "?native-cutover=" + uuid.uuid4().hex], capture=True).stdout


def verify_public(revision):
    public_get("/healthz")
    if json.loads(public_get("/noctemblith-build.json")).get("sha") != revision:
        raise RuntimeError("Public endpoint does not serve the intended native revision")
    if json.loads(public_get("/auth/manafield/status")).get("enabled") is not True:
        raise RuntimeError("Public native SSO is not enabled")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--preflight", action="store_true", help="read-only checks")
    args = parser.parse_args()
    live = shared.docker_inventory()
    if APP not in live or "traefik" not in live:
        raise RuntimeError("Expected native app or Traefik is not running")
    app = live[APP]
    labels = app["Config"]["Labels"]
    if labels.get("com.docker.compose.project") != "manafield" or labels.get("com.docker.compose.service") != "module-noctemblith":
        raise RuntimeError("Unexpected native app ownership")
    release = Path(labels["com.docker.compose.project.config_files"].split(",")[0]).resolve().parent
    if not release.is_relative_to((shared.INSTANCE / "releases").resolve()):
        raise RuntimeError("Native Release is outside the Instance")
    recipe = json.loads((release / "native-connection-recipe.json").read_text())
    health = json.loads((release / "native-health-result.json").read_text())
    if health.get("health") != "passed" or health.get("resources_registered") is not True:
        raise RuntimeError("Native connection has no successful Resource/health receipt")
    revision = recipe["native_revision"]
    if json.loads(app_get("/noctemblith-build.json")).get("sha") != revision:
        raise RuntimeError("Running native revision differs from the connection recipe")
    app_get("/healthz")
    if json.loads(app_get("/auth/manafield/status")).get("enabled") is not True:
        raise RuntimeError("Native SSO is not enabled")
    env_values = dict(v.split("=", 1) for v in app["Config"]["Env"] if "=" in v)
    if env_values.get("MANAFIELD_MODULE_PUBLIC_URL") != "https://" + HOST:
        raise RuntimeError("Native public origin differs from the existing route")
    if EDGE not in live["traefik"]["NetworkSettings"]["Networks"]:
        raise RuntimeError("Traefik is not on the expected edge network")
    route = shared.load_yaml(ROUTE)
    if route["http"]["routers"]["noctemblith-dev"]["rule"] != "Host(`" + HOST + "`)":
        raise RuntimeError("Public router definition changed")
    servers = route["http"]["services"]["noctemblith-dev"]["loadBalancer"]["servers"]
    if servers not in [[{"url":"http://noctemblith-dev:3000"}], [{"url":"http://" + ALIAS + ":8080"}]]:
        raise RuntimeError("Unexpected public route backend; inspect before replacing")
    modules_path = release / "modules.compose.yml"
    modules = shared.load_yaml(modules_path)
    base = shared.load_yaml(release / "compose.yml")
    if base["networks"]["edge"]["name"] not in [EDGE, "${MANAFIELD_EDGE_NETWORK:-manafield-edge}"]:
        raise RuntimeError("Unexpected persisted edge network definition")
    network = json.loads(shared.run(["docker", "network", "inspect", EDGE], capture=True).stdout)[0]
    # A unique alias prevents old/new backends from sharing DNS round-robin.
    for identifier in network.get("Containers", {}):
        other = json.loads(shared.run(["docker", "inspect", identifier], capture=True).stdout)[0]
        aliases = other["NetworkSettings"]["Networks"][EDGE].get("Aliases") or []
        if ALIAS in aliases and other["Id"] != app["Id"]:
            raise RuntimeError("Native edge alias is already owned by another container")
    targets = []
    for name, service_name in [("noctemblith-dev-noctemblith-1","noctemblith"),
                               ("noctemblith-dev-db-1","db"),("noctemblith-dev-redis-1","redis")]:
        if name in live:
            old_labels = live[name]["Config"].get("Labels") or {}
            if old_labels.get("com.docker.compose.project") != "noctemblith-dev" or old_labels.get("com.docker.compose.service") != service_name:
                raise RuntimeError("Legacy ownership differs; no cutover performed")
            targets.append(name)
    print("PASS: native revision, health, SSO, public origin and route ownership", flush=True)
    if args.preflight:
        return 0

    backup = release / ("cutover-" + time.strftime("%Y%m%dT%H%M%SZ", time.gmtime()) + "-" + uuid.uuid4().hex[:6])
    backup.mkdir(mode=0o700)
    route_before, modules_before = ROUTE.read_bytes(), modules_path.read_bytes()
    route_mode, modules_mode = stat.S_IMODE(ROUTE.stat().st_mode), stat.S_IMODE(modules_path.stat().st_mode)
    atomic(backup / "route.before.yml", route_before, 0o600)
    atomic(backup / "modules.before.yml", modules_before, 0o600)
    connected = False
    changed = False
    print("Cutover recovery directory:", backup, flush=True)
    try:
        if EDGE not in app["NetworkSettings"]["Networks"]:
            shared.run(["docker", "network", "connect", "--alias", ALIAS,
                        "--alias", "module-noctemblith", EDGE, APP])
            connected = True
        elif ALIAS not in (app["NetworkSettings"]["Networks"][EDGE].get("Aliases") or []):
            raise RuntimeError("Native app already has a different edge attachment; no disconnect performed")
        service = modules["services"]["module-noctemblith"]
        service.setdefault("networks", {})["edge"] = {"aliases":[ALIAS]}
        changed = True
        atomic(modules_path, json_bytes(modules), modules_mode)
        profiles = (release / "compose-profiles.txt").read_text().strip()
        compose = ["docker", "compose", "--env-file", release / "release.env",
                   "-f", release / "compose.yml", "-f", modules_path]
        shared.run(compose + ["config", "--quiet"], env=dict(os.environ, COMPOSE_PROFILES=profiles), capture=True)
        servers[0]["url"] = "http://" + ALIAS + ":8080"
        atomic(ROUTE, json_bytes(route), route_mode)
        for attempt in range(12):
            try:
                verify_public(revision)
                break
            except (RuntimeError, ValueError, shared.subprocess.CalledProcessError):
                if attempt == 11:
                    raise RuntimeError("Public native checks failed")
                time.sleep(2)
        print("PASS: public HTTPS health, exact native build and SSO-enabled status", flush=True)
    except Exception:
        rollback_failed = False
        if changed:
            for path, data, mode in [(ROUTE, route_before, route_mode),
                                      (modules_path, modules_before, modules_mode)]:
                try:
                    atomic(path, data, mode)
                except OSError:
                    rollback_failed = True
        if connected:
            try:
                shared.run(["docker", "network", "disconnect", EDGE, APP], capture=True)
            except (OSError, shared.subprocess.CalledProcessError):
                rollback_failed = True
        if rollback_failed:
            print("Cutover failed and recovery is incomplete; inspect the recovery directory. Old SNS was not stopped.", file=sys.stderr)
        else:
            print("Public cutover failed; original route/config restored; old SNS was not stopped.", file=sys.stderr)
        raise

    result = {"public_origin":"https://" + HOST, "native_revision":revision,
              "native_release":str(release),"public_checks":"passed",
              "data_and_volumes_retained":True,"standalone_untouched":True,
              "legacy_stop_complete":False, "legacy_targets":targets}
    atomic(backup / "result.json", json_bytes(result), 0o600)
    # The new route is already proven. Stop the old app before its own DB/Redis.
    if targets:
        shared.run(["docker", "stop", "--time", "20", targets[0]])
        if len(targets) > 1:
            shared.run(["docker", "stop", "--time", "20", *targets[1:]])
    result["legacy_stop_complete"] = True
    atomic(backup / "result.json", json_bytes(result), 0o600)
    try:
        verify_public(revision)
    except (RuntimeError, ValueError, shared.subprocess.CalledProcessError):
        raise RuntimeError("Public cutover passed, but post-stop verification failed; inspect native app and recovery directory")
    print("PASS: public native cutover complete; old dev SNS stopped; all old data/volumes retained")
    print("URL: https://" + HOST)
    print("Recovery/result:", backup)
    print("Browser SSO and app media E2E remain unverified; active Instance/Jenkins promotion remains separate.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except RuntimeError as error:
        print("Stopped: " + str(error), file=sys.stderr)
        sys.exit(1)
    except (OSError, ValueError, KeyError, shared.subprocess.CalledProcessError):
        print("Cutover command stopped. Inspect the printed recovery directory; no credentials are printed.", file=sys.stderr)
        sys.exit(1)
