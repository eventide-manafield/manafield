"""Offline tests of the operator recipe; Docker operations are simulated."""
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("native_connect", HERE / "noctemblith-native-connect.py")
recipe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(recipe)
REAL_RUN = recipe.run
CLI = HERE.parent / "target/debug/manafield"
YAML_LIBRARY = recipe.YAML_LIBRARY


class RecipeTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source = self.root / "source"
        self.native = self.root / "native"
        self.instance = self.root / "instance"
        self.old = self.instance / "releases/58"
        for directory in [self.source / "deploy", self.native / "packages/backend/migration",
                          self.old / "modules/noctemblith", self.old / "modules/account"]:
            directory.mkdir(parents=True)
        shutil.copy(HERE / "compose.yml", self.source / "deploy/compose.yml")
        (self.native / "Dockerfile").write_text("ARG NOCTEMBLITH_BUILD_SHA=unknown\n")
        (self.native / "packages/backend/migration/1792170000000-NoctemblithBaseline.js").touch()
        manifest = {"id": "noctemblith", "requires": {"capabilities": {
            "state": {"id": "database.postgresql", "version": "^1.0.0"},
            "media": {"id": "storage.object", "version": "^1.0.0"},
            "cache": {"id": "cache.redis", "version": "^1.0.0"}}}}
        (self.native / "manafield.module.json").write_text(json.dumps(manifest))
        (self.old / "modules/account/manafield.module.json").write_text('{"id":"account"}')
        base = {"name":"manafield","services":{"core":{"image":"old-core:tag","volumes":[
            {"type":"bind","source":"${MANAFIELD_MODULES_PATH}","target":"/var/lib/manafield/modules","read_only":True}]},
            "manafield-postgres":{"image":"postgres:18-alpine"},
            "manafield-postgres-provider":{"image":"old-provider:tag"}},
            "volumes":{"manafield-postgres-data":{}}, "networks":{"modules":{"name":"manafield-modules"}}}
        (self.old / "compose.yml").write_text(json.dumps(base))
        (self.old / "modules.compose.yml").write_text(json.dumps({"services":{"module-account":{"image":"old-account:tag"}}}))
        (self.old / "release.env").write_text("MANAFIELD_POSTGRES_PASSWORD=fake-private\n")
        (self.old / "compose-profiles.txt").write_text("postgresql\n")
        (self.old / "postgresql-allocations.tsv").write_text("pg\tmanafield\tcore_log\tcore_log\t/private/core.password\n")
        secrets = self.instance / "secrets/s3/manafield-r2"
        secrets.mkdir(parents=True)
        (secrets / "access-key-id").write_text("test-id")
        (secrets / "secret-access-key").write_text("test-secret")
        definition = {"version":0,"instance":{"id":"fixture"},
            "core":{"source":{"repository":"https://example.test/core.git","ref":"main"}},
            "modules":[{"id":"noctemblith","source":{"type":"dir"},
                "build":{"type":"docker","context":".","dockerfile":"Dockerfile"},
                "bindings":{"state":"manafield-postgres","media":"manafield-r2","cache":"noctemblith-redis"},
                "exposure":{"type":"host","host":"native.example.test","targetPort":8080}},
                {"id":"account","source":{"type":"dir"},"build":{"type":"docker","context":".","dockerfile":"Dockerfile"}}],
            "resources":[{"id":"manafield-postgres","provider":"postgresql"},
                {"id":"manafield-r2","provider":"s3","config":{"endpoint":"https://account.r2.cloudflarestorage.com","bucket":"fixture-media"}},
                {"id":"noctemblith-redis","provider":"redis","exclusiveTo":{"module":"noctemblith","slot":"cache"}}],
            "deployment":{"modulesNetwork":"manafield-modules","edgeNetwork":"edge",
                          "ingress":{"provider":"traefik","output":"/tmp/candidate.yml"}}}
        self.candidate = self.instance / "candidate.yaml"
        self.candidate.write_text(json.dumps(definition))
        sso = ["NOCTEMBLITH_MANAFIELD_SSO_ENABLED=true","NOCTEMBLITH_MANAFIELD_ACCOUNT_ORIGIN=https://manafield.studio",
               "NOCTEMBLITH_MANAFIELD_OAUTH_CLIENT_ID=test-native",
               "NOCTEMBLITH_MANAFIELD_OAUTH_CALLBACK_URL=https://native.example.test/auth/manafield/callback"]
        def item(service, image, env=None):
            return {"Config":{"Labels":{"com.docker.compose.project":"manafield","com.docker.compose.service":service},
                              "Env":env or []},"Image":image}
        core = item("core","sha256:core-pinned")
        core["Config"]["Labels"]["com.docker.compose.project.config_files"] = str(self.old / "compose.yml")+","+str(self.old / "modules.compose.yml")
        core["Mounts"] = [{"Source":str(self.old / "modules"),"Destination":"/var/lib/manafield/modules"}]
        self.live = {"manafield-core-1":core,
            "manafield-manafield-postgres-1":item("manafield-postgres","sha256:pg-pinned",["POSTGRES_DB=manafield","POSTGRES_USER=manafield"]),
            "manafield-manafield-postgres-provider-1":item("manafield-postgres-provider","sha256:provider-pinned"),
            "manafield-module-account-1":item("module-account","sha256:account-public-handle"),
            "noctemblith-dev-noctemblith-1":item("old-native","sha256:old-native",sso)}
        self.calls = []

    def fake_run(self, args, **kwargs):
        args = [str(v) for v in args]
        self.calls.append(args)
        if args[0] == "node" or (args[0] == str(CLI) and args[1] in ["plan","stage-resources"]):
            return REAL_RUN(args, **kwargs)
        if args[0] == "git":
            return subprocess.CompletedProcess(args,0,stdout="a"*40+"\n" if "rev-parse" in args else "")
        if args[0] == str(CLI) and args[1] == "build":
            (Path(args[2]) / "resolved-images.env").write_text(
                "resource.manafield-r2.provider.image=s3:pinned\nresource.noctemblith-redis.provider.image=observer:pinned\nresource.noctemblith-redis.server.image=redis:pinned\n")
        if args[0] == "docker" and args[-2:] == ["config","--quiet"]:
            return REAL_RUN(args, **kwargs)
        if "http://core:8080/resources" in args:
            return subprocess.CompletedProcess(args,0,stdout=json.dumps([
                {"id":resource,"provides":{"capabilities":[{"id":cap,"version":"1.0.0"}]}}
                for resource,cap in [("manafield-postgres","database.postgresql"),
                    ("manafield-r2","storage.object"),("noctemblith-redis","cache.redis")]]))
        output = '{"enabled":true}' if any("/auth/manafield/status" in v for v in args) else ""
        return subprocess.CompletedProcess(args,0,stdout=output)

    def invoke(self, argv, count=0):
        with patch.multiple(recipe, SOURCE=self.source, NATIVE=self.native, INSTANCE=self.instance,
                            CANDIDATE=self.candidate, CLI=CLI, YAML_LIBRARY=YAML_LIBRARY), \
             patch.object(recipe, "docker_inventory", return_value=self.live), \
             patch.object(recipe, "pg_count", return_value=count), \
             patch.object(recipe, "run", side_effect=self.fake_run), \
             patch.object(recipe, "snapshot", side_effect=lambda a,b:shutil.copytree(a,b)), \
             patch.object(sys,"argv",["connect",*argv]):
            return recipe.main()

    def test_full_recipe_preserves_pins_and_applies_only_selected_services(self):
        self.assertEqual(self.invoke([]),0)
        release = next(p for p in (self.instance / "releases").iterdir() if p.name.startswith("native-"))
        modules = recipe.load_yaml(release / "modules.compose.yml")
        base = recipe.load_yaml(release / "compose.yml")
        self.assertEqual(base["services"]["core"]["image"],"sha256:core-pinned")
        self.assertEqual(base["services"]["manafield-postgres"]["image"],"sha256:pg-pinned")
        self.assertEqual(modules["services"]["module-account"]["image"],"sha256:account-public-handle")
        self.assertEqual(modules["services"]["module-noctemblith"]["networks"],{"modules":{}})
        applies = [c for c in self.calls if c[0]==str(CLI) and c[1]=="apply"]
        self.assertEqual(len(applies),2)
        targets = [c[i+1] for c in applies for i,v in enumerate(c) if v=="--service"]
        self.assertEqual(targets,["core","manafield-postgres-provider","manafield-s3-provider",
                                  "manafield-redis","manafield-redis-provider","module-noctemblith"])
        result = json.loads((release / "native-health-result.json").read_text())
        self.assertFalse(result["ingress_published"])
        self.assertIn("core_log",(release / "postgresql-allocations.tsv").read_text())
        for c in self.calls:
            self.assertNotIn("--remove-orphans",c)
            self.assertNotIn("down",c)

    def test_readonly_preflight_creates_no_release(self):
        self.assertEqual(self.invoke(["--preflight"]),0)
        self.assertEqual([p.name for p in (self.instance / "releases").iterdir()],["58"])
        self.assertFalse(any(c[0]==str(CLI) for c in self.calls))

    def test_populated_target_is_refused_before_changes(self):
        with self.assertRaisesRegex(RuntimeError,"already contains tables"):
            self.invoke([],count=1)
        self.assertEqual([p.name for p in (self.instance / "releases").iterdir()],["58"])


if __name__ == "__main__":
    unittest.main()
