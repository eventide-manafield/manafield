#!/usr/bin/env python3
"""Read-only Docker inventory for native Noctemblith Resource integration.
Never persists raw inspect output, secret values or container commands.
"""
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path("/opt/manafield/ai-workspace")
OUTPUT = ROOT / "work/noctemblith-native-connect-20261019/docker-inventory.json"
SAFE_ENV = {
    "MANAFIELD_CORE_URL", "MANAFIELD_RESOURCE_ID", "MANAFIELD_MODULE_ID",
    "MANAFIELD_POSTGRES_RESOURCE_ID", "MANAFIELD_MODULES_NETWORK", "PORT",
    "POSTGRES_DB", "POSTGRES_USER", "POSTGRES_HOST", "POSTGRES_PORT",
    "MANAFIELD_BINDING_STATE_ENDPOINT_HOST", "MANAFIELD_BINDING_STATE_ENDPOINT_PORT",
    "MANAFIELD_BINDING_STATE_CONFIG_DATABASE", "MANAFIELD_BINDING_STATE_CONFIG_SCHEMA",
    "MANAFIELD_BINDING_STATE_CONFIG_USERNAME", "NOCTEMBLITH_MANAFIELD_SSO_ENABLED",
    "NOCTEMBLITH_MANAFIELD_ACCOUNT_ORIGIN", "NOCTEMBLITH_MANAFIELD_OAUTH_CLIENT_ID",
    "NOCTEMBLITH_MANAFIELD_OAUTH_CALLBACK_URL",
}
SAFE_LABELS = {
    "com.docker.compose.project", "com.docker.compose.service",
    "com.docker.compose.project.config_files",
    "com.docker.compose.project.working_dir",
}


def summarize(item):
    config = item.get("Config") or {}
    labels = config.get("Labels") or {}
    env = {}
    for entry in config.get("Env") or []:
        key, separator, value = entry.partition("=")
        if separator and key in SAFE_ENV:
            # URLs must not carry userinfo/query credentials even in allowed fields.
            if key.endswith(("URL", "ORIGIN")):
                from urllib.parse import urlsplit
                parsed = urlsplit(value)
                if parsed.username or parsed.password or parsed.query or parsed.fragment:
                    env[key] = "<redacted URL with credentials or parameters>"
                    continue
            env[key] = value
    state = item.get("State") or {}
    return {
        "name": item.get("Name", "").lstrip("/"),
        "image_ref": config.get("Image"),
        "image_id": item.get("Image"),
        "user": config.get("User"),
        "state": state.get("Status"),
        "health": (state.get("Health") or {}).get("Status"),
        "compose": {key: value for key, value in labels.items() if key in SAFE_LABELS},
        "networks": {
            name: {"aliases": data.get("Aliases", []), "ip": data.get("IPAddress")}
            for name, data in (item.get("NetworkSettings", {}).get("Networks") or {}).items()
        },
        "mounts": [
            {"type": mount.get("Type"), "name": mount.get("Name"),
             "source": mount.get("Source"), "target": mount.get("Destination"),
             "read_write": mount.get("RW")}
            for mount in item.get("Mounts") or []
        ],
        "environment_keys": sorted(entry.partition("=")[0] for entry in config.get("Env") or []),
        "safe_environment": env,
    }


def self_test():
    sample = {"Config": {"Env": [
        "POSTGRES_PASSWORD=DO_NOT_EXPORT_PASSWORD",
        "MANAFIELD_ACCOUNT_TOKEN=DO_NOT_EXPORT_TOKEN",
        "MANAFIELD_CORE_URL=https://user:DO_NOT_EXPORT_URL_PASSWORD@example.test/?token=NO",
        "MANAFIELD_MODULE_ID=noctemblith",
    ], "Labels": {"unknown": "DO_NOT_EXPORT_LABEL"}, "Cmd": ["DO_NOT_EXPORT_COMMAND"]}}
    result = json.dumps(summarize(sample))
    assert all(x not in result for x in [
        "DO_NOT_EXPORT_PASSWORD", "DO_NOT_EXPORT_TOKEN", "DO_NOT_EXPORT_URL_PASSWORD",
        "DO_NOT_EXPORT_LABEL", "DO_NOT_EXPORT_COMMAND",
    ])
    assert "noctemblith" in result
    print("PASS: inventory does not export secret values, commands or unapproved labels")


def main():
    if sys.argv[1:] == ["--self-test"]:
        self_test()
        return
    if sys.argv[1:]:
        raise SystemExit("usage: inspect-native-connection.py [--self-test]")
    try:
        ids = subprocess.run(["docker", "ps", "-aq"], check=True, text=True,
                             capture_output=True).stdout.split()
        containers = []
        for start in range(0, len(ids), 30):
            result = subprocess.run(["docker", "inspect", *ids[start:start + 30]],
                                    check=True, capture_output=True, text=True)
            for item in json.loads(result.stdout):
                project = (item.get("Config", {}).get("Labels") or {}).get("com.docker.compose.project", "")
                name = item.get("Name", "")
                if any(word in (project + " " + name).lower() for word in ["manafield", "noctemblith", "traefik", "jenkins"]):
                    containers.append(summarize(item))
        data = {"containers": containers}
        OUTPUT.parent.mkdir(exist_ok=True)
        # The directory is group-shared with manafield-agent. No secret values
        # are present in the report, but unrelated local users cannot read it.
        temp = OUTPUT.with_suffix(".tmp")
        with open(temp, "w", encoding="utf-8") as stream:
            json.dump(data, stream, indent=2)
            stream.write("\n")
        temp.chmod(0o640)
        os.replace(temp, OUTPUT)
        print("PASS: read-only Docker inventory saved")
        print(OUTPUT)
        for container in containers:
            print(container["name"], container["state"], container["health"] or "-")
        print("No services, credentials, networks or volumes were modified.")
    except subprocess.CalledProcessError:
        raise SystemExit("Docker inventory failed. Run as the Docker-enabled SSH account; raw inspect data is not printed.")


if __name__ == "__main__":
    main()
