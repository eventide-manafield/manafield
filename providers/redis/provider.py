"""Read-only Redis Resource observer: cache.redis@1.0.0.

Never proxies cache data, logs passwords, or changes Redis keys. Registers
exactly one Resource Instance in Manafield Core when authenticated PING works.
"""
from __future__ import annotations

import json
import logging
import os
import re
import time
from dataclasses import dataclass
from pathlib import Path
from urllib.error import HTTPError, URLError
from urllib.parse import quote, urlsplit
from urllib.request import Request, urlopen

LOG = logging.getLogger("manafield.redis.provider")


def mandatory(env: dict[str, str], name: str) -> str:
    value = env.get(name, "").strip()
    if not value:
        raise ValueError(f"Missing required Redis Provider setting: {name}")
    return value


def password_from_file(filename: str) -> str:
    if not filename.startswith("/"):
        raise ValueError("Redis password file path must be absolute")
    try:
        raw = Path(filename).read_bytes()
    except OSError:
        raise ValueError("Cannot read Redis password file") from None
    if not raw or len(raw) > 4096:
        raise ValueError("Invalid Redis password file length")
    try:
        value = raw.decode("utf-8").removesuffix("\n").removesuffix("\r")
    except UnicodeDecodeError:
        raise ValueError("Invalid Redis password file encoding") from None
    if not re.fullmatch(r"[0-9a-f]{48}", value):
        raise ValueError("Redis password must be a 48-digit lowercase hex secret")
    return value


@dataclass(frozen=True)
class Settings:
    resource_id: str
    resource_name: str
    core_url: str
    redis_host: str
    redis_port: int
    password_file: str
    interval: int

    @classmethod
    def from_env(cls, env: dict[str, str]) -> Settings:
        resource_id = mandatory(env, "MANAFIELD_RESOURCE_ID")
        if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]{0,62}", resource_id) or ".." in resource_id:
            raise ValueError("Invalid Redis Resource ID")
        name = env.get("MANAFIELD_RESOURCE_NAME", "Manafield Redis Cache").strip()
        if not name or len(name) > 200:
            raise ValueError("Invalid Redis Resource name")
        core_url = env.get("MANAFIELD_CORE_URL", "http://core:8080").rstrip("/")
        parsed = urlsplit(core_url)
        if (
            parsed.scheme not in ("http", "https") or not parsed.hostname
            or parsed.username or parsed.password or parsed.query or parsed.fragment
        ):
            raise ValueError("Invalid Manafield Core URL")
        host = mandatory(env, "MANAFIELD_REDIS_HOST")
        if not re.fullmatch(r"[a-zA-Z0-9.-]{1,253}", host):
            raise ValueError("Invalid Redis host")
        port_text = env.get("MANAFIELD_REDIS_PORT", "6379")
        if not port_text.isdecimal() or not 1 <= int(port_text) <= 65535:
            raise ValueError("Invalid Redis port")
        seconds = env.get("MANAFIELD_RESOURCE_CHECK_INTERVAL", "15")
        if not seconds.isdecimal() or not 5 <= int(seconds) <= 3600:
            raise ValueError("Invalid Redis interval")
        return cls(
            resource_id, name, core_url, host, int(port_text),
            mandatory(env, "MANAFIELD_REDIS_PASSWORD_FILE"), int(seconds),
        )


def metadata(settings: Settings) -> bytes:
    return json.dumps({
        "id": settings.resource_id,
        "name": settings.resource_name,
        "type": "redis",
        "description": "Redis cache observed by Manafield Redis Resource Provider.",
        "provides": {"capabilities": [{"id": "cache.redis", "version": "1.0.0"}]},
    }, separators=(",", ":")).encode("utf-8")


def redis_ready(client) -> bool:
    try:
        return client.ping() is True
    except Exception as exc:
        LOG.warning("Redis PING not ready (%s)", type(exc).__name__)
        return False


def registered(settings: Settings, opener=urlopen) -> bool:
    request = Request(settings.core_url + "/resources/" + quote(settings.resource_id, safe=""), method="GET")
    try:
        with opener(request, timeout=8) as response:
            if response.status != 200:
                return False
            value = json.load(response)
    except (HTTPError, URLError, OSError, ValueError):
        return False
    caps = value.get("provides", {}).get("capabilities", [])
    return (
        value.get("id") == settings.resource_id
        and value.get("type") == "redis"
        and any(cap.get("id") == "cache.redis" and cap.get("version") == "1.0.0" for cap in caps)
    )


def register(settings: Settings, opener=urlopen) -> bool:
    request = Request(
        settings.core_url + "/resources",
        data=metadata(settings),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with opener(request, timeout=8) as response:
            if response.status == 201:
                LOG.info("Registered Redis Resource %s", settings.resource_id)
                return True
    except HTTPError as exc:
        if exc.code == 409:
            if registered(settings, opener):
                return True
            LOG.error("Redis Resource ID collision: %s", settings.resource_id)
            return False
        LOG.warning("Redis Resource registration unavailable (HTTP %d)", exc.code)
        return False
    except (URLError, OSError) as exc:
        LOG.warning("Redis Resource registration unavailable (%s)", type(exc).__name__)
        return False
    return False


def check_once(settings: Settings, client, opener=urlopen) -> bool:
    if not redis_ready(client):
        return False
    return registered(settings, opener) or register(settings, opener)


def main() -> int:
    logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(message)s")
    try:
        import redis
        settings = Settings.from_env(dict(os.environ))
        connection = redis.Redis(
            host=settings.redis_host,
            port=settings.redis_port,
            password=password_from_file(settings.password_file),
            socket_connect_timeout=5,
            socket_timeout=8,
            decode_responses=True,
        )
    except Exception as exc:
        LOG.error("Redis observer startup failed (%s)", type(exc).__name__)
        return 1
    LOG.info("Redis Resource observer started: %s", settings.resource_id)
    while True:
        check_once(settings, connection)
        time.sleep(settings.interval)


if __name__ == "__main__":
    raise SystemExit(main())
