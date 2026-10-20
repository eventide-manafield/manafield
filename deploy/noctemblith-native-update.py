#!/usr/bin/env python3
"""Replace only the published native app image; retain its bindings and data."""
import argparse
import copy
import fcntl
import importlib.util
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import time
import uuid

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("cutover", Path(__file__).with_name("noctemblith-native-cutover.py"))
cutover = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cutover)
shared = cutover.shared
APP = cutover.APP
SERVICE = "module-noctemblith"
REVISION = "905c625dad0926b17d9754efbb0d4d9f7f173409"
IMAGE = "manafield-module-noctemblith:905c625dad09"
# All three digests belong to the exact build output supplied by the operator.
# Classic and containerd image stores can expose different levels as image Id.
EXPECTED_IMAGE_DIGESTS = {
    "sha256:b5060c576e1c07ec20db5127641595b33a0dfdab933dc0388749055839ad997a": "config",
    "sha256:ab0e9f4005b8bf6591ac1ba1b14611c32141c81d51208bdf4d30251fdec8168c": "manifest",
    "sha256:b70b1d4c45929fb052c2b7f70d9715e6fbc739bbaecb77ee63ee0ab91232c0e9": "index",
}
ENV = {"PATH": "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin", "LANG": "C.UTF-8"}
PROBE = r"""
const fs=require('node:fs'),crypto=require('node:crypto');
const dir='/misskey/packages/backend/migration';
const migrations=Object.fromEntries(fs.readdirSync(dir).sort().filter(n=>/\.(js|sql)$/.test(n))
 .map(n=>[n,crypto.createHash('sha256').update(fs.readFileSync(dir+'/'+n)).digest('hex')]));
console.log(JSON.stringify({build:JSON.parse(fs.readFileSync('/misskey/packages/backend/assets/noctemblith-build.json','utf8')),migrations}));
"""


def run(args, *, cwd=None, env=None):
    return subprocess.run([str(x) for x in args], cwd=cwd, env=env or ENV,
                          text=True, check=True, capture_output=True, timeout=180).stdout


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def validate_image_identity(target):
    image_id = target.get("Id")
    kind = EXPECTED_IMAGE_DIGESTS.get(image_id)
    require(kind is not None,
            "Image digest differs from all three supplied build digests; actual Id: " + str(image_id))
    print("PASS: built image identity (" + kind + "): " + image_id, flush=True)
    return image_id


def inspect(name):
    return json.loads(run(["docker", "inspect", name]))[0]


def endpoint(path):
    return run(["docker", "exec", APP, "curl", "-fsS", "--max-time", "10",
                "http://127.0.0.1:8080" + path])


def public(path):
    return run(["curl", "-fsS", "--max-time", "15", "-H", "Cache-Control: no-cache",
                "https://" + cutover.HOST + path + "?native-update=" + uuid.uuid4().hex])


def verify(revision, public_check=True):
    require(inspect(APP)["State"].get("Health", {}).get("Status") == "healthy",
            "App container is not healthy")
    for get in ([endpoint, public] if public_check else [endpoint]):
        get("/healthz")
        require(json.loads(get("/noctemblith-build.json")).get("sha") == revision,
                "Endpoint serves a different build")
        require(json.loads(get("/auth/manafield/status")).get("enabled") is True,
                "SSO-enabled status check failed")


def wait_healthy(revision):
    deadline = time.monotonic() + 240
    while True:
        try:
            verify(revision)
            return
        except (RuntimeError, ValueError, subprocess.SubprocessError):
            if time.monotonic() >= deadline:
                raise RuntimeError("App/public health or build check timed out")
            time.sleep(3)


def persisted_write(path, data, metadata):
    cutover.atomic(path, data, stat.S_IMODE(metadata.st_mode))
    os.chown(path, metadata.st_uid, metadata.st_gid)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--preflight", action="store_true", help="validate without replacing the live app")
    args = parser.parse_args()
    require(os.geteuid() == 0, "Run this operator helper with sudo (Docker and release.env access required)")
    app = inspect(APP)
    labels = app["Config"].get("Labels") or {}
    require(labels.get("com.docker.compose.project") == "manafield"
            and labels.get("com.docker.compose.service") == SERVICE, "Unexpected app ownership")
    files = [Path(p).resolve() for p in labels["com.docker.compose.project.config_files"].split(",")]
    release = files[0].parent
    require(release.is_relative_to((shared.INSTANCE / "releases").resolve()), "Unexpected release location")
    require(files == [release / "compose.yml", release / "modules.compose.yml"],
            "Unexpected Compose files; inspect before updating")
    with (release / ".noctemblith-update.lock").open("a") as lock:
        os.chmod(lock.name, 0o600)
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        return apply(args, app, release, files)


def apply(args, app, release, files):
    path = files[1]
    original = path.read_bytes()
    metadata = path.stat()
    modules = json.loads(original)
    service = modules["services"][SERVICE]
    env = service["environment"]
    expected = {
        "MANAFIELD_MODULE_PUBLIC_URL": "https://" + cutover.HOST,
        "NOCTEMBLITH_MANAFIELD_SSO_ENABLED": "true",
        "MANAFIELD_BINDING_STATE_TARGET": "manafield-postgres",
        "MANAFIELD_BINDING_MEDIA_TARGET": "manafield-r2",
        "MANAFIELD_BINDING_CACHE_TARGET": "noctemblith-redis",
    }
    live_env = dict(value.split("=", 1) for value in app["Config"]["Env"] if "=" in value)
    require(all(env.get(k) == v and live_env.get(k) == v for k, v in expected.items()),
            "Live/persisted origin, SSO or resource bindings differ")
    require(inspect(service["image"])["Id"] == app["Image"], "Persisted image differs from the live app")
    require(cutover.ALIAS in service.get("networks", {}).get("edge", {}).get("aliases", []),
            "Published edge alias is missing from Compose")
    route_before = cutover.ROUTE.read_bytes()
    route = shared.load_yaml(cutover.ROUTE)
    require(route["http"]["services"]["noctemblith-dev"]["loadBalancer"]["servers"]
            == [{"url": "http://" + cutover.ALIAS + ":8080"}], "Public route no longer targets native app")
    target = inspect(IMAGE)
    validate_image_identity(target)
    candidate = json.loads(run(["docker", "run", "--rm", "--network", "none", "--read-only",
                                "--cap-drop", "ALL", "--security-opt", "no-new-privileges:true",
                                "--entrypoint", "node", target["Id"], "-e", PROBE]))
    previous = json.loads(run(["docker", "exec", APP, "node", "-e", PROBE]))
    require(candidate["build"].get("sha") == REVISION
            and candidate["build"].get("channel") == "noctemblith/dev", "Image build metadata differs")
    require(candidate["migrations"] and candidate["migrations"] == previous["migrations"],
            "Migration files differ; this app-only helper does not approve schema changes")
    previous_revision = previous["build"]["sha"]
    verify(previous_revision)
    profiles = (release / "compose-profiles.txt").read_text().strip()
    compose_env = dict(ENV, COMPOSE_PROFILES=profiles)
    compose = ["docker", "compose", "-p", "manafield", "--env-file", release / "release.env",
               "-f", files[0], "-f", files[1]]
    def dc(*args):
        return run(compose + list(args), cwd=release, env=compose_env)
    dc("config", "--quiet")
    print("PASS: live release, public route, bindings, exact image and unchanged migrations", flush=True)
    if args.preflight:
        return 0
    if app["Image"] == target["Id"]:
        print("PASS: intended image is already running; public health/build/SSO verified")
        return 0

    backup = release / ("image-update-" + time.strftime("%Y%m%dT%H%M%SZ", time.gmtime()) + "-" + uuid.uuid4().hex[:6])
    backup.mkdir(mode=0o700)
    cutover.atomic(backup / "modules.before.yml", original, 0o600)
    result = {"previous_revision": previous_revision, "previous_image": app["Image"],
              "revision": REVISION, "image": IMAGE, "image_id": target["Id"],
              "service": SERVICE, "release": str(release), "status": "prepared"}
    def receipt(status):
        result["status"] = status
        cutover.atomic(backup / "result.json", cutover.json_bytes(result), 0o600)
    receipt("prepared")
    print("Image-update recovery directory:", backup, flush=True)
    changed = copy.deepcopy(modules)
    changed["services"][SERVICE]["image"] = target["Id"]
    rollback = copy.deepcopy(modules)
    rollback["services"][SERVICE]["image"] = app["Image"]
    require(path.read_bytes() == original and inspect(APP)["Id"] == app["Id"],
            "Live deployment changed during preflight")
    try:
        persisted_write(path, cutover.json_bytes(changed), metadata)
        dc("config", "--quiet")
        # Only the named app may be recreated. Dependencies, volumes and routes are retained.
        print("Replacing module-noctemblith; waiting for public verification...", flush=True)
        dc("up", "-d", "--no-deps", "--no-build", "--pull", "never", SERVICE)
        wait_healthy(REVISION)
        current = inspect(APP)
        require(current["Image"] == target["Id"], "Running container uses a different image")
        after_env = dict(v.split("=", 1) for v in current["Config"]["Env"] if "=" in v)
        require(all(after_env.get(k) == live_env.get(k) for k in env), "App environment changed")
        def mounts(item):
            return sorted((m["Type"], m.get("Source", ""), m["Destination"], m["RW"]) for m in item["Mounts"])
        require(mounts(current) == mounts(app), "App mounts changed")
        require(cutover.ROUTE.read_bytes() == route_before, "Public route file changed concurrently")
        receipt("passed")
    except (Exception, KeyboardInterrupt):
        print("Update failed; restoring the previous app image...", flush=True)
        try:
            persisted_write(path, cutover.json_bytes(rollback), metadata)
            dc("up", "-d", "--no-deps", "--no-build", "--pull", "never", SERVICE)
            wait_healthy(previous_revision)
            receipt("rolled_back")
            print("PASS: previous image and public health/build/SSO restored", flush=True)
        except (Exception, KeyboardInterrupt):
            receipt("rollback_incomplete")
            print("RECOVERY INCOMPLETE: inspect the recovery directory before further deployment", file=sys.stderr)
        raise RuntimeError("Requested update did not complete; see recovery/result")
    print("PASS: native app image updated; public HTTPS health, exact build and SSO-enabled status")
    print("Revision:", REVISION)
    print("URL: https://" + cutover.HOST)
    print("Recovery/result:", backup)
    print("Only module-noctemblith was targeted; Browser SSO/media E2E and Jenkins promotion remain separate.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except RuntimeError as error:
        print("Stopped:", str(error), file=sys.stderr)
        sys.exit(1)
    except (OSError, ValueError, KeyError, subprocess.SubprocessError):
        print("Stopped: operator/configuration check failed; no credentials are printed.", file=sys.stderr)
        sys.exit(1)
