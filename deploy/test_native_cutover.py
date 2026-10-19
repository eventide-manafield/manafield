"""Offline cutover/rollback tests; no live services or routes are changed."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("native_cutover", Path(__file__).with_name("noctemblith-native-cutover.py"))
cutover = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cutover)


class CutoverTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.release = self.root / "releases/native-fixture"
        self.release.mkdir(parents=True)
        self.route = self.root / "router.yml"
        self.modules = self.release / "modules.compose.yml"
        self.revision = "a" * 40
        def write(path, value):
            path.write_text(json.dumps(value))
        write(self.route,{"http":{"routers":{"noctemblith-dev":{"rule":"Host(`"+cutover.HOST+"`)" }},
              "services":{"noctemblith-dev":{"loadBalancer":{"servers":[{"url":"http://noctemblith-dev:3000"}]}}}}})
        write(self.modules,{"services":{"module-noctemblith":{"image":"native:pinned","networks":{"modules":{}}}}})
        write(self.release/"compose.yml",{"networks":{"edge":{"name":cutover.EDGE}}})
        write(self.release/"native-connection-recipe.json",{"native_revision":self.revision})
        write(self.release/"native-health-result.json",{"health":"passed","resources_registered":True})
        (self.release/"compose-profiles.txt").write_text("postgresql,s3,redis")
        self.original_route = self.route.read_bytes()
        self.original_modules = self.modules.read_bytes()
        def item(project,service):
            return {"Id":project+service,"Config":{"Labels":{"com.docker.compose.project":project,
                    "com.docker.compose.service":service},"Env":[]},
                    "NetworkSettings":{"Networks":{}}}
        app = item("manafield","module-noctemblith")
        app["Config"]["Env"] = ["MANAFIELD_MODULE_PUBLIC_URL=https://"+cutover.HOST]
        app["Config"]["Labels"]["com.docker.compose.project.config_files"] = str(self.release/"compose.yml")+","+str(self.modules)
        traefik = item("traefik","traefik")
        traefik["NetworkSettings"]["Networks"][cutover.EDGE] = {}
        self.live = {cutover.APP:app,"traefik":traefik}
        for name,service in [("noctemblith","noctemblith"),("db","db"),("redis","redis")]:
            self.live["noctemblith-dev-"+name+"-1"] = item("noctemblith-dev",service)
        self.calls=[]

    def fake_run(self, args, **kwargs):
        args = [str(v) for v in args]
        self.calls.append(args)
        output = '[{"Containers":{}}]' if args[:3]==["docker","network","inspect"] else ""
        return subprocess.CompletedProcess(args,0,stdout=output)

    def app_get(self,path):
        if path=="/noctemblith-build.json":
            return json.dumps({"sha":self.revision})
        return '{"enabled":true}'

    def public_get(self,path):
        return self.app_get(path)

    def invoke(self, preflight=False, public_failure=False):
        def public_get(path):
            if public_failure:
                raise RuntimeError("simulated public failure")
            return self.public_get(path)
        with patch.object(cutover,"ROUTE",self.route), \
             patch.object(cutover.shared,"INSTANCE",self.root), \
             patch.object(cutover.shared,"docker_inventory",return_value=self.live), \
             patch.object(cutover.shared,"load_yaml",side_effect=lambda p:json.loads(p.read_text())), \
             patch.object(cutover.shared,"run",side_effect=self.fake_run), \
             patch.object(cutover,"app_get",side_effect=self.app_get), \
             patch.object(cutover,"public_get",side_effect=public_get), \
             patch.object(cutover.time,"sleep"), \
             patch.object(sys,"argv",["cutover"]+(["--preflight"] if preflight else [])), \
             contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            return cutover.main()

    def test_publish_then_stop_only_old_dev_services(self):
        self.assertEqual(self.invoke(),0)
        route=json.loads(self.route.read_text())
        self.assertEqual(route["http"]["services"]["noctemblith-dev"]["loadBalancer"]["servers"],
                         [{"url":"http://noctemblith-native:8080"}])
        self.assertEqual(json.loads(self.modules.read_text())["services"]["module-noctemblith"]["networks"]["edge"],
                         {"aliases":["noctemblith-native"]})
        stops=[c for c in self.calls if c[:2]==["docker","stop"]]
        self.assertEqual(stops,[["docker","stop","--time","20","noctemblith-dev-noctemblith-1"],
                               ["docker","stop","--time","20","noctemblith-dev-db-1","noctemblith-dev-redis-1"]])
        result=json.loads(next(self.release.glob("cutover-*/result.json")).read_text())
        self.assertTrue(result["legacy_stop_complete"])
        self.assertTrue(result["data_and_volumes_retained"])
        self.assertFalse(any("down" in c or "prune" in c for c in self.calls))

    def test_public_failure_restores_files_and_does_not_stop_old_services(self):
        with self.assertRaisesRegex(RuntimeError,"Public native checks failed"):
            self.invoke(public_failure=True)
        self.assertEqual(self.route.read_bytes(),self.original_route)
        self.assertEqual(self.modules.read_bytes(),self.original_modules)
        self.assertFalse(any(c[:2]==["docker","stop"] for c in self.calls))
        self.assertIn(["docker","network","disconnect",cutover.EDGE,cutover.APP],self.calls)

    def test_preflight_is_readonly(self):
        self.assertEqual(self.invoke(preflight=True),0)
        self.assertEqual(self.route.read_bytes(),self.original_route)
        self.assertEqual(self.modules.read_bytes(),self.original_modules)
        self.assertFalse(list(self.release.glob("cutover-*")))
        self.assertFalse(any(c[:3] in [["docker","network","connect"],["docker","network","disconnect"]]
                             or c[:2]==["docker","stop"] for c in self.calls))


if __name__=="__main__":
    unittest.main()
