"""Offline Redis Resource Provider unit tests; no Docker daemon or Redis needed."""
import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path
from urllib.error import HTTPError

SPEC = importlib.util.spec_from_file_location("manafield_redis_provider", Path(__file__).with_name("provider.py"))
provider = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = provider
SPEC.loader.exec_module(provider)


def env():
    return {
        "MANAFIELD_RESOURCE_ID": "manafield-redis",
        "MANAFIELD_REDIS_HOST": "manafield-redis",
        "MANAFIELD_REDIS_PORT": "6379",
        "MANAFIELD_REDIS_PASSWORD_FILE": "/run/manafield/redis/password",
    }


class RedisClient:
    def __init__(self, result=True, error=None):
        self.result = result
        self.error = error
        self.calls = 0

    def ping(self):
        self.calls += 1
        if self.error is not None:
            raise self.error
        return self.result


class Reply:
    def __init__(self, code, data):
        self.status = code
        self.body = json.dumps(data).encode()

    def read(self, *_args):
        return self.body

    def __enter__(self):
        return self

    def __exit__(self, *_args):
        return False


class Transport:
    def __init__(self, data=None, post=201):
        self.data = data
        self.post = post
        self.calls = []

    def __call__(self, req, timeout=8):
        self.calls.append(req.get_method())
        if req.get_method() == "GET":
            if self.data is None:
                raise HTTPError(req.full_url, 404, "missing", {}, None)
            return Reply(200, self.data)
        if self.post == 409:
            raise HTTPError(req.full_url, 409, "collision", {}, None)
        return Reply(self.post, {})


class RedisProviderTests(unittest.TestCase):
    def test_metadata_is_generic_and_credentials_never_appear(self):
        settings = provider.Settings.from_env(env())
        data = json.loads(provider.metadata(settings))
        self.assertEqual(data["provides"]["capabilities"], [{"id": "cache.redis", "version": "1.0.0"}])
        self.assertNotIn(settings.password_file, json.dumps(data))
        self.assertNotIn("MANAFIELD_REDIS_PASSWORD_FILE", json.dumps(data))

    def test_rejects_bad_resource_ids_ports_and_paths(self):
        for change in (
            {"MANAFIELD_RESOURCE_ID": "../broken"},
            {"MANAFIELD_REDIS_PORT": "0"},
            {"MANAFIELD_REDIS_PORT": "65536"},
            {"MANAFIELD_REDIS_PORT": "abc"},
        ):
            with self.assertRaises(ValueError):
                provider.Settings.from_env({**env(), **change})

    def test_password_file_validation(self):
        with tempfile.TemporaryDirectory() as dirname:
            secret = Path(dirname) / "password"
            secret.write_text("a" * 48 + "\n")
            self.assertEqual(provider.password_from_file(str(secret)), "a" * 48)
            for bad in ("", "unsafe-key", "a" * 48 + "\nother"):
                secret.write_text(bad)
                with self.assertRaises(ValueError):
                    provider.password_from_file(str(secret))

    def test_authenticated_ping_is_read_only(self):
        s = provider.Settings.from_env(env())
        transport = Transport()
        redis = RedisClient()
        self.assertTrue(provider.check_once(s, redis, transport))
        self.assertEqual(redis.calls, 1)
        self.assertEqual(transport.calls, ["GET", "POST"])

    def test_redis_unavailable_prevents_registration(self):
        s = provider.Settings.from_env(env())
        transport = Transport()
        self.assertFalse(provider.check_once(s, RedisClient(error=OSError("private token")), transport))
        self.assertEqual(transport.calls, [])

    def test_accepts_idempotent_registration(self):
        s = provider.Settings.from_env(env())
        transport = Transport(data=json.loads(provider.metadata(s)), post=409)
        self.assertTrue(provider.register(s, transport))

    def test_rejects_wrong_existing_capability(self):
        s = provider.Settings.from_env(env())
        data = json.loads(provider.metadata(s))
        data["provides"]["capabilities"][0]["id"] = "database.postgresql"
        transport = Transport(data=data, post=409)
        self.assertFalse(provider.register(s, transport))


if __name__ == "__main__":
    unittest.main()
