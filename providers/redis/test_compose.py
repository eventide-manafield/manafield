"""Offline Compose/startup checks: Docker CLI only, no daemon/container/network."""
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


@unittest.skipUnless(shutil.which("docker"), "Docker Compose CLI is required")
class RedisComposeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        result = subprocess.run(
            ["docker", "compose", "--profile", "redis", "--profile", "s3",
             "-f", "deploy/compose.yml", "config", "--format", "json"],
            cwd=ROOT, capture_output=True, text=True, check=True,
        )
        cls.compose = json.loads(result.stdout)

    def test_startup_is_one_shell_program_with_valid_syntax(self):
        service = self.compose["services"]["manafield-redis"]
        self.assertEqual(service["entrypoint"], ["/bin/sh", "-ec"])
        self.assertEqual(len(service["command"]), 1)
        program = service["command"][0].replace("$$", "$")
        result = subprocess.run(["/bin/sh", "-n"], input=program, text=True, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_readonly_secrets_and_no_published_redis_ports(self):
        for name in ["manafield-redis", "manafield-redis-provider"]:
            service = self.compose["services"][name]
            self.assertFalse(service.get("ports"))
            self.assertTrue(service["read_only"])
            mounts = [m for m in service["volumes"]
                      if m.get("target") == "/run/manafield/redis/password"]
            self.assertEqual(len(mounts), 1)
            self.assertTrue(mounts[0]["read_only"])

    def test_startup_writes_private_aof_queue_policy_without_password_argv(self):
        # Execute only the rendered shell startup, replacing ALL secret/config
        # paths and the Redis binary with disposable offline test files.
        with tempfile.TemporaryDirectory(prefix="manafield-redis-command-") as dirname:
            root = Path(dirname)
            secret = root / "password"
            config = root / "redis.conf"
            secret.write_text("a" * 48 + "\n")
            fake = root / "redis-server"
            fake.write_text("#!/bin/sh\n[ \"$#\" -eq 1 ] && [ -r \"$1\" ]\n")
            fake.chmod(0o700)
            program = self.compose["services"]["manafield-redis"]["command"][0]
            program = program.replace("$$", "$").replace(
                "/run/manafield/redis/password", str(secret)
            ).replace("/tmp/redis.conf", str(config)).replace(
                "exec redis-server ", f"exec {fake} "
            )
            result = subprocess.run(["/bin/sh", "-ec", program], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, "Redis shell startup did not complete")
            self.assertNotIn("a" * 48, result.stdout + result.stderr)
            self.assertEqual(config.stat().st_mode & 0o777, 0o600)
            self.assertEqual(config.read_text().splitlines(), [
                "requirepass " + "a" * 48,
                "appendonly yes", "appendfsync everysec",
                "maxmemory-policy noeviction", "dir /data",
            ])


if __name__ == "__main__":
    unittest.main()
