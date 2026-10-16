"""Observe an S3-compatible bucket and register a generic Manafield Resource.

No object data passes through Manafield Core. This process performs only a
bounded, read-only ListObjectsV2 readiness check and Resource registration.
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

LOG = logging.getLogger("manafield.s3.provider")
CAPABILITY_ID = "storage.object"
CAPABILITY_VERSION = "1.0.0"


def required(env: dict[str, str], key: str) -> str:
    value = env.get(key, "").strip()
    if not value:
        raise ValueError(f"Missing required S3 provider setting: {key}")
    return value


def read_secret(filename: str) -> str:
    if not filename.startswith("/"):
        raise ValueError("S3 credential file path must be absolute")
    # File contents are intentionally never displayed in exceptions or logs.
    try:
        raw = Path(filename).read_bytes()
    except OSError:
        raise ValueError("Cannot read configured S3 credential file") from None
    if not raw or len(raw) > 4096:
        raise ValueError("S3 credential file is empty or too large")
    try:
        value = raw.decode("utf-8").removesuffix("\n").removesuffix("\r")
    except UnicodeDecodeError:
        raise ValueError("S3 credential file must be UTF-8") from None
    if not value or "\r" in value or "\n" in value or "\x00" in value:
        raise ValueError("S3 credential file has invalid contents")
    return value


@dataclass(frozen=True)
class Settings:
    resource_id: str
    resource_name: str
    core_url: str
    endpoint: str
    bucket: str
    region: str
    force_path_style: bool
    access_key_id_file: str
    secret_access_key_file: str
    check_interval: int

    @classmethod
    def from_env(cls, env: dict[str, str]) -> Settings:
        resource_id = required(env, "MANAFIELD_RESOURCE_ID")
        if not re.fullmatch(r"[a-zA-Z0-9][a-zA-Z0-9_.-]{0,62}", resource_id):
            raise ValueError("Invalid S3 Resource Instance ID")

        resource_name = env.get("MANAFIELD_RESOURCE_NAME", "Manafield S3 Object Storage").strip()
        if not resource_name or len(resource_name) > 200:
            raise ValueError("Invalid S3 Resource name")
        core_url = env.get("MANAFIELD_CORE_URL", "http://core:8080").rstrip("/")
        if urlsplit(core_url).scheme not in ("http", "https"):
            raise ValueError("Invalid Manafield Core URL")

        endpoint = required(env, "MANAFIELD_S3_ENDPOINT").rstrip("/")
        parsed = urlsplit(endpoint)
        if (
            parsed.scheme != "https" or not parsed.hostname
            or parsed.username is not None or parsed.password is not None
            or parsed.path or parsed.query or parsed.fragment
        ):
            raise ValueError("S3 endpoint must be a root HTTPS URL without credentials")
        bucket = required(env, "MANAFIELD_S3_BUCKET")
        if (not 3 <= len(bucket) <= 63 or not re.fullmatch(r"[a-z0-9][a-z0-9.-]*[a-z0-9]", bucket)
                or ".." in bucket):
            raise ValueError("Invalid S3 bucket name")
        region = env.get("MANAFIELD_S3_REGION", "auto").strip()
        if not re.fullmatch(r"[a-z0-9-]{1,40}", region):
            raise ValueError("Invalid S3 region")
        style = env.get("MANAFIELD_S3_FORCE_PATH_STYLE", "true").strip().lower()
        if style not in ("true", "false"):
            raise ValueError("MANAFIELD_S3_FORCE_PATH_STYLE must be true or false")
        interval_text = env.get("MANAFIELD_RESOURCE_CHECK_INTERVAL", "15").strip()
        if not interval_text.isdecimal() or not 5 <= int(interval_text) <= 3600:
            raise ValueError("MANAFIELD_RESOURCE_CHECK_INTERVAL must be 5..3600 seconds")
        return cls(
            resource_id, resource_name, core_url, endpoint, bucket, region,
            style == "true", required(env, "MANAFIELD_S3_ACCESS_KEY_ID_FILE"),
            required(env, "MANAFIELD_S3_SECRET_ACCESS_KEY_FILE"), int(interval_text),
        )


def metadata(settings: Settings) -> bytes:
    """Generic Resource metadata: no endpoints, keys or secret references."""
    return json.dumps({
        "id": settings.resource_id,
        "name": settings.resource_name,
        "type": "s3",
        "description": "S3-compatible bucket observed by Manafield S3 Resource Provider.",
        "provides": {"capabilities": [
            {"id": CAPABILITY_ID, "version": CAPABILITY_VERSION}
        ]},
    }, separators=(",", ":")).encode("utf-8")


def make_s3_client(settings: Settings):
    """Load credentials only into the provider process, never into the Registry."""
    import boto3
    from botocore.config import Config

    return boto3.client(
        "s3",
        endpoint_url=settings.endpoint,
        region_name=settings.region,
        aws_access_key_id=read_secret(settings.access_key_id_file),
        aws_secret_access_key=read_secret(settings.secret_access_key_file),
        config=Config(
            signature_version="s3v4",
            s3={"addressing_style": "path" if settings.force_path_style else "virtual"},
            connect_timeout=5,
            read_timeout=10,
            retries={"max_attempts": 2, "mode": "standard"},
        ),
    )


def storage_ready(client, settings: Settings) -> bool:
    try:
        # Readiness MUST be checked with an authenticated bucket-scoped API.
        # Do not create buckets, write probe objects or delete content.
        client.list_objects_v2(Bucket=settings.bucket, MaxKeys=1)
        return True
    except Exception as exc:
        LOG.warning("S3 bucket probe failed (error type: %s)", type(exc).__name__)
        return False


def _resource_url(settings: Settings) -> str:
    return settings.core_url + "/resources/" + quote(settings.resource_id, safe="")


def registered(settings: Settings, opener=urlopen) -> bool:
    request = Request(_resource_url(settings), method="GET")
    try:
        with opener(request, timeout=8) as response:
            if response.status != 200:
                return False
            data = json.load(response)
    except (HTTPError, URLError, OSError, ValueError):
        return False
    # Do not accept a name collision with an unrelated Resource Instance.
    provided = data.get("provides", {}).get("capabilities", [])
    return (
        data.get("id") == settings.resource_id
        and data.get("type") == "s3"
        and any(item.get("id") == CAPABILITY_ID and item.get("version") == CAPABILITY_VERSION
                for item in provided)
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
                LOG.info("Registered S3 Resource %s", settings.resource_id)
                return True
    except HTTPError as exc:
        if exc.code == 409:
            if registered(settings, opener):
                return True
            LOG.error("S3 Resource ID collision or incompatible registration: %s", settings.resource_id)
            return False
        LOG.warning("Resource registration refused (HTTP %d)", exc.code)
        return False
    except (URLError, OSError) as exc:
        LOG.warning("Resource registration unavailable (error type: %s)", type(exc).__name__)
        return False
    return False


def check_once(settings: Settings, s3_client, opener=urlopen) -> bool:
    if not storage_ready(s3_client, settings):
        return False
    if registered(settings, opener):
        return True
    return register(settings, opener)


def main() -> int:
    logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(message)s")
    try:
        settings = Settings.from_env(dict(os.environ))
        client = make_s3_client(settings)
    except (ValueError, Exception) as exc:
        # No exception content may contain credential values or file contents.
        LOG.error("S3 provider configuration failed (%s)", type(exc).__name__)
        return 1

    LOG.info("S3 Resource observer started: %s", settings.resource_id)
    while True:
        check_once(settings, client)
        time.sleep(settings.check_interval)


if __name__ == "__main__":
    raise SystemExit(main())
