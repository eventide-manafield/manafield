"""Run offline: python -m unittest discover -s providers/s3 -p 'test_*.py'."""

import importlib.util
import io
import json
import tempfile
import unittest
from pathlib import Path
from urllib.error import HTTPError


SPEC = importlib.util.spec_from_file_location(
    "manafield_s3_provider", Path(__file__).with_name("provider.py")
)
provider = importlib.util.module_from_spec(SPEC)
import sys
sys.modules[SPEC.name] = provider
SPEC.loader.exec_module(provider)


def env():
    return {
        "MANAFIELD_RESOURCE_ID": "manafield-r2",
        "MANAFIELD_RESOURCE_NAME": "Shared media",
        "MANAFIELD_S3_ENDPOINT": "https://account.r2.cloudflarestorage.com",
        "MANAFIELD_S3_BUCKET": "app-media",
        "MANAFIELD_S3_REGION": "auto",
        "MANAFIELD_S3_FORCE_PATH_STYLE": "true",
        "MANAFIELD_S3_ACCESS_KEY_ID_FILE": "/run/manafield/s3/access-key-id",
        "MANAFIELD_S3_SECRET_ACCESS_KEY_FILE": "/run/manafield/s3/secret-access-key",
    }


class FakeResponse:
    def __init__(self, status, body):
        self.status, self.body = status, body

    def __enter__(self):
        return self

    def __exit__(self, *_args):
        return False

    def read(self, _size=-1):
        return json.dumps(self.body).encode("utf-8")


class FakeOpener:
    def __init__(self, get=None, post=201):
        self.get, self.post = get, post
        self.calls = []

    def __call__(self, request, timeout=8):
        self.calls.append((request.get_method(), request.full_url, timeout))
        if request.get_method() == "GET":
            if self.get is None:
                raise HTTPError(request.full_url, 404, "missing", {}, None)
            return FakeResponse(200, self.get)
        if self.post == 409:
            raise HTTPError(request.full_url, 409, "exists", {}, None)
        return FakeResponse(self.post, {})


class FakeS3:
    def __init__(self, failure=None):
        self.failure = failure
        self.calls = []

    def list_objects_v2(self, **kwargs):
        self.calls.append(kwargs)
        if self.failure:
            raise self.failure
        return {"KeyCount": 0}


class S3ResourceProviderTests(unittest.TestCase):
    def test_requires_explicit_endpoint_bucket_and_secret_paths(self):
        data = env()
        for key in ("MANAFIELD_S3_ENDPOINT", "MANAFIELD_S3_BUCKET", "MANAFIELD_S3_SECRET_ACCESS_KEY_FILE"):
            x = dict(data)
            x.pop(key)
            with self.assertRaises(ValueError):
                provider.Settings.from_env(x)

    def test_rejects_http_or_url_embedded_credentials(self):
        data = env()
        for endpoint in (
            "http://example.invalid", "https://alice:token@example.invalid",
            "https://example.invalid/api", "https://example.invalid/?key=secret",
        ):
            with self.assertRaises(ValueError):
                provider.Settings.from_env({**data, "MANAFIELD_S3_ENDPOINT": endpoint})

    def test_r2_settings_and_metadata_do_not_include_secrets(self):
        settings = provider.Settings.from_env(env())
        result = json.loads(provider.metadata(settings))
        self.assertEqual(settings.region, "auto")
        self.assertTrue(settings.force_path_style)
        self.assertEqual(result["provides"]["capabilities"], [{"id": "storage.object", "version": "1.0.0"}])
        self.assertNotIn("access", json.dumps(result).lower())
        self.assertNotIn("secret", json.dumps(result).lower())
        self.assertNotIn(settings.bucket, json.dumps(result))
        self.assertNotIn(settings.endpoint, json.dumps(result))

    def test_secret_file_is_read_only_and_rejects_malformed(self):
        with tempfile.TemporaryDirectory() as root:
            p = Path(root) / "test.secret"
            p.write_bytes(b"example-key\n")
            self.assertEqual(provider.read_secret(str(p)), "example-key")
            for data in (b"", b"\n", b"one\ntwo", b"x" * 4097, b"\xff"):
                p.write_bytes(data)
                with self.assertRaises(ValueError):
                    provider.read_secret(str(p))

    def test_storage_probe_does_not_mutate_objects(self):
        settings = provider.Settings.from_env(env())
        s3 = FakeS3()
        self.assertTrue(provider.storage_ready(s3, settings))
        self.assertEqual(s3.calls, [{"Bucket": "app-media", "MaxKeys": 1}])
        self.assertFalse(provider.storage_ready(FakeS3(PermissionError()), settings))

    def test_register_after_s3_ready(self):
        settings = provider.Settings.from_env(env())
        opener = FakeOpener()
        self.assertTrue(provider.check_once(settings, FakeS3(), opener))
        self.assertEqual([name for name, _, _ in opener.calls], ["GET", "POST"])

    def test_fail_closed_when_s3_unavailable(self):
        settings = provider.Settings.from_env(env())
        opener = FakeOpener()
        self.assertFalse(provider.check_once(settings, FakeS3(PermissionError()), opener))
        self.assertEqual(opener.calls, [])

    def test_rejects_resource_collision_with_wrong_capability(self):
        settings = provider.Settings.from_env(env())
        different = json.loads(provider.metadata(settings))
        different["provides"]["capabilities"][0]["id"] = "database.postgresql"
        opener = FakeOpener(get=different, post=409)
        self.assertFalse(provider.register(settings, opener))

    def test_accepts_idempotent_registered_resource(self):
        settings = provider.Settings.from_env(env())
        opener = FakeOpener(get=json.loads(provider.metadata(settings)), post=409)
        self.assertTrue(provider.register(settings, opener))

    def test_rejects_malformed_name_and_bucket(self):
        data = env()
        for bucket in ("../secrets", "MyBucket", "ab", "a..b"):
            with self.assertRaises(ValueError):
                provider.Settings.from_env({**data, "MANAFIELD_S3_BUCKET": bucket})
        with self.assertRaises(ValueError):
            provider.Settings.from_env({**data, "MANAFIELD_RESOURCE_ID": "../unsafe"})


if __name__ == "__main__":
    unittest.main()
