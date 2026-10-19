#!/usr/bin/env python3
"""Operator-only recovery of the first local admin in the empty native test instance.

Does not enable the public signup API or modify the running image, SSO, Resource
bindings, central Account or standalone instance. Password is entered without
echo and passed only via stdin. No password or login token is printed or saved.
Existing ordinary users/root cause refusal; verified internal system accounts are retained. Runtime verification requires SSH Docker access.
"""
import argparse
import getpass
import json
import re
import subprocess
import sys
import time
from pathlib import Path

APP = "manafield-module-noctemblith-1"
ORIGIN = "https://noctemblith-dev.manafield.studio"


def run(args, **kwargs):
    return subprocess.run(args, text=True, check=True, capture_output=True, **kwargs)


def verify_app():
    app = json.loads(run(["docker", "inspect", APP]).stdout)[0]
    labels = app["Config"].get("Labels") or {}
    if labels.get("com.docker.compose.project") != "manafield" or labels.get("com.docker.compose.service") != "module-noctemblith":
        raise RuntimeError("Unexpected application ownership")
    if not app["State"].get("Running") or app["State"].get("Health", {}).get("Status") != "healthy":
        raise RuntimeError("Native application is not healthy")
    release = Path(labels["com.docker.compose.project.config_files"].split(",")[0]).resolve().parent
    if release.parent != Path("/opt/manafield/ai-workspace/instance/releases"):
        raise RuntimeError("Unexpected Release")
    recipe = json.loads((release / "native-connection-recipe.json").read_text())
    build = json.loads(run(["docker", "exec", APP, "curl", "-fsS", "--max-time", "10",
                            "http://127.0.0.1:8080/noctemblith-build.json"]).stdout)
    if build.get("sha") != recipe.get("native_revision"):
        raise RuntimeError("Running revision differs from reviewed Release")
    status = json.loads(run(["docker", "exec", APP, "curl", "-fsS", "--max-time", "10",
                             "http://127.0.0.1:8080/auth/manafield/status"]).stdout)
    if status.get("enabled") is not True:
        raise RuntimeError("SSO must remain enabled")
    return app["Id"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--username", default="noctadmin")
    parser.add_argument("--preflight", action="store_true")
    args = parser.parse_args()
    if not re.fullmatch(r"[a-z0-9_]{1,20}", args.username):
        raise RuntimeError("Username must match [a-z0-9_]{1,20}")
    original_id = verify_app()
    guard = Path(__file__).with_name("noctemblith-native-bootstrap-guard.mjs").read_text()
    code = guard + "\n" + Path(__file__).with_suffix(".mjs").read_text()
    command = ["docker", "exec", "-i", "-e", "NODE_ENV=production", APP,
               "node", "--input-type=module", "-e", code]
    print(run(command).stdout.strip(), flush=True)
    if args.preflight:
        return
    if not sys.stdin.isatty():
        raise RuntimeError("Use an interactive SSH terminal for password entry")
    print("Creating initial LOCAL administrator: " + args.username, flush=True)
    print("This is the administrator login password, not a setup-screen password.", flush=True)
    password = getpass.getpass("New administrator password (at least 10 characters): ")
    again = getpass.getpass("Confirm password: ")
    if password != again or len(password) < 10 or len(password.encode("utf-8")) > 72:
        raise RuntimeError("Passwords must match; minimum 10 characters, maximum 72 UTF-8 bytes")
    if verify_app() != original_id:
        raise RuntimeError("Container changed while entering credentials")
    result = run(command + ["--", "--apply"],
                 input=json.dumps({"username": args.username, "password": password}))
    password = again = ""
    print(result.stdout.strip(), flush=True)
    print("Refreshing only the native app to load the committed root administrator.", flush=True)
    run(["docker", "restart", "--timeout", "20", APP])
    for _ in range(30):
        try:
            verify_app()
            break
        except (RuntimeError, subprocess.CalledProcessError):
            time.sleep(2)
    else:
        raise RuntimeError("Administrator was created, but app restart health has not recovered")
    print("PASS: native app healthy; SSO enabled; initial local administrator " + args.username)
    print("Open: " + ORIGIN + " (refresh the browser)")
    print("Central Account and standalone accounts are unchanged.")


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as error:
        # Node helper intentionally emits only sanitized diagnostics.
        detail = next((s for s in (error.stderr or "").splitlines()
                       if s.startswith("Stopped:")), "Operation failed; no credentials printed")
        print(detail, file=sys.stderr)
        sys.exit(1)
    except (RuntimeError, OSError, ValueError, KeyError) as error:
        message = str(error) if isinstance(error, RuntimeError) else "Could not inspect expected deployment files"
        print("Stopped: " + message, file=sys.stderr)
        sys.exit(1)
