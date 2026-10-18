#!/usr/bin/env python3
"""Operator-only Docker probe. Uses fresh test credentials/resources, no live Instance."""
import argparse
import copy
import json
import os
from pathlib import Path
import secrets
import shutil
import subprocess
import sys
import tempfile
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]
REGISTRY = """
import json
from http.server import BaseHTTPRequestHandler, HTTPServer
resources = {}
class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args): pass
    def reply(self, status, data):
        body = json.dumps(data).encode()
        self.send_response(status)
        self.send_header('Content-Type', 'application/json')
        self.end_headers()
        self.wfile.write(body)
    def do_GET(self):
        if self.path == '/health': return self.reply(200, {})
        identifier = self.path.removeprefix('/resources/')
        self.reply(200 if identifier in resources else 404, resources.get(identifier, {}))
    def do_POST(self):
        if self.path != '/resources': return self.reply(404, {})
        data = json.loads(self.rfile.read(int(self.headers.get('Content-Length', '0'))))
        resources[data['id']] = data
        self.reply(201, {})
HTTPServer(('0.0.0.0', 8080), Handler).serve_forever()
"""
PROBE = """
import assert from 'node:assert/strict';
import fs from 'node:fs';
import Redis from 'ioredis';
import { Queue, Worker, QueueEvents } from 'bullmq';
const mode = process.argv[2];
assert.equal(process.getuid(), 991);
const password = fs.readFileSync('/run/manafield/redis/password', 'utf8').trim();
assert.match(password, /^[0-9a-f]{48}$/);
const connection = { host: 'cache', port: 6379, password, maxRetriesPerRequest: null,
  retryStrategy: n => n < 20 ? 200 : null };
const client = new Redis(connection);
client.on('error', () => {});
const queue = new Queue('restart-probe', { connection, prefix: 'mf_probe' });
queue.on('error', () => {});
try {
  assert.equal(await client.ping(), 'PONG');
  if (mode === 'prepare') {
    const denied = new Redis({ host: 'cache', port: 6379, lazyConnect: true,
      enableReadyCheck: false, retryStrategy: () => null, maxRetriesPerRequest: 0 });
    denied.on('error', () => {});
    try {
      await denied.connect();
      await assert.rejects(denied.ping(), /NOAUTH/);
    } finally { denied.disconnect(); }
    const sub = new Redis(connection);
    sub.on('error', () => {});
    try {
      const received = new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error('Pub/Sub timeout')), 5000);
        sub.once('message', (channel, message) => { clearTimeout(timer); resolve(message); });
      });
      await sub.subscribe('mf_probe:channel');
      await client.publish('mf_probe:channel', 'delivered');
      assert.equal(await received, 'delivered');
    } finally { sub.disconnect(); }
    await client.set('mf_probe:persistent', 'survived');
    await queue.add('persisted-job', { answer: 42 }, { jobId: 'survive' });
    assert.equal(await (await queue.getJob('survive')).getState(), 'waiting');
    let registered = false;
    for (let i = 0; i < 40; i++) {
      try {
        const response = await fetch('http://registry:8080/resources/runtime-probe-redis');
        if (response.ok) {
          const data = await response.json();
          assert.equal(data.id, 'runtime-probe-redis');
          assert.equal(data.type, 'redis');
          assert.ok(data.provides.capabilities.some(c => c.id === 'cache.redis' && c.version === '1.0.0'));
          registered = true;
          break;
        }
      } catch {}
      await new Promise(resolve => setTimeout(resolve, 1000));
    }
    assert.ok(registered, 'Provider did not register cache.redis');
    console.log('PASS: UID 991 secret read, authenticated PING, NOAUTH, Pub/Sub, Provider registration, queued job');
  } else if (mode === 'verify') {
    assert.equal(await client.get('mf_probe:persistent'), 'survived');
    const job = await queue.getJob('survive');
    assert.ok(job, 'BullMQ job missing after Redis restart');
    const events = new QueueEvents('restart-probe', { connection, prefix: 'mf_probe' });
    events.on('error', () => {});
    const worker = new Worker('restart-probe', async item => item.data.answer,
      { connection, prefix: 'mf_probe' });
    worker.on('error', () => {});
    try {
      await events.waitUntilReady();
      const result = await job.waitUntilFinished(events, 15000);
      assert.equal(result, 42);
    } finally { await worker.close(); await events.close(); }
    console.log('PASS: AOF data and BullMQ job survived restart; real Worker completed restored job');
  } else { throw new Error('Unknown probe mode'); }
} catch {
  console.error('FAIL: Redis runtime assertion; credentials are not printed');
  process.exitCode = 1;
} finally {
  await queue.close();
  client.disconnect();
}
"""


def run(args, *, env=None, capture=False):
    return subprocess.run(args, env=env, check=True, text=True,
                          stdout=subprocess.PIPE if capture else None,
                          stderr=subprocess.PIPE if capture else None)


def build_manifest(project, work, template, server_image, provider_image, probe_image):
    cache = copy.deepcopy(template["services"]["manafield-redis"])
    cache.update(image=server_image, restart="no", networks=["probe"])
    cache.pop("profiles", None)
    cache["volumes"] = [
        {"type": "volume", "source": "data", "target": "/data"},
        {"type": "bind", "source": str(work / "password"),
         "target": "/run/manafield/redis/password", "read_only": True},
    ]
    observer = copy.deepcopy(template["services"]["manafield-redis-provider"])
    observer.pop("profiles", None)
    observer.update(image=provider_image, restart="no", networks=["probe"],
                    depends_on={"cache": {"condition": "service_healthy"},
                                "registry": {"condition": "service_started"}})
    observer["environment"].update(
        MANAFIELD_CORE_URL="http://registry:8080",
        MANAFIELD_RESOURCE_ID="runtime-probe-redis",
        MANAFIELD_REDIS_HOST="cache",
        MANAFIELD_RESOURCE_CHECK_INTERVAL="5",
    )
    observer["volumes"] = [cache["volumes"][1]]
    return {
        "name": project,
        "services": {
            "cache": cache,
            "observer": observer,
            "registry": {
                "image": provider_image, "entrypoint": ["python", "-c", REGISTRY],
                "user": "65532:65532", "read_only": True, "cap_drop": ["ALL"],
                "security_opt": ["no-new-privileges:true"], "networks": ["probe"],
            },
            "probe": {
                "image": probe_image, "user": "991:991", "read_only": True,
                "tmpfs": ["/tmp"], "cap_drop": ["ALL"],
                "security_opt": ["no-new-privileges:true"], "networks": ["probe"],
                "volumes": [cache["volumes"][1]],
                "environment": {"NODE_ENV": "production"},
            },
        },
        "volumes": {"data": {"labels": {"manafield.runtime-probe": project}}},
        "networks": {"probe": {"internal": True}},
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prepare-only", action="store_true",
                        help="Generate/validate probe artifacts without Docker daemon access")
    args = parser.parse_args()
    if not shutil.which("docker"):
        raise SystemExit("Docker CLI is required")
    project = "manafield-redis-probe-" + uuid.uuid4().hex[:12]
    work = Path(tempfile.mkdtemp(prefix=project + "-"))
    work.chmod(0o700)
    password = work / "password"
    password.write_text(secrets.token_hex(24) + "\n")
    password.chmod(0o444)
    server_image = project + "-server:local"
    provider_image = project + "-provider:local"
    probe_image = project + "-client:local"
    env = dict(os.environ, MANAFIELD_REDIS_PASSWORD_HOST=str(password),
               MANAFIELD_REDIS_RESOURCE_ID="runtime-probe-redis")
    template = json.loads(run(
        ["docker", "compose", "--profile", "redis", "-f", str(ROOT / "deploy/compose.yml"),
         "config", "--format", "json"], env=env, capture=True).stdout)
    manifest = build_manifest(project, work, template, server_image, provider_image, probe_image)
    config = work / "compose.json"
    config.write_text(json.dumps(manifest))
    config.chmod(0o600)
    (work / "probe.mjs").write_text(PROBE)
    # Exact library versions used by the native Noctemblith PR.
    (work / "Dockerfile").write_text(
        "FROM node:26-alpine\nWORKDIR /probe\n"
        "RUN npm init -y >/dev/null && npm install --ignore-scripts --no-audit --no-fund "
        "--omit=dev bullmq@6.3.2 ioredis@5.11.1\n"
        "COPY probe.mjs /probe/probe.mjs\nUSER 991:991\n"
        'ENTRYPOINT ["node", "/probe/probe.mjs"]\n')
    # Build context must never contain the test password or Compose secret path.
    (work / ".dockerignore").write_text("*\n!Dockerfile\n!probe.mjs\n")
    compose = ["docker", "compose", "--project-name", project, "-f", str(config)]
    run(compose + ["config", "--quiet"], capture=True)
    assert not any(service.get("ports") for service in manifest["services"].values())
    assert len(manifest["services"]["cache"]["command"]) == 1
    if args.prepare_only:
        # Parse JS without resolving imports or starting a process/container.
        if shutil.which("node"):
            run(["node", "--check", str(work / "probe.mjs")], capture=True)
        shutil.rmtree(work)
        print("PASS: isolated manifest and Node probe syntax; no Docker daemon used")
        return 0
    print("Isolated Redis runtime probe:", project, flush=True)
    print("Builds fresh test images; uses no live credentials, hosts or service volumes.", flush=True)
    started = False
    success = False
    try:
        run(["docker", "info", "--format", "{{.ServerVersion}}"], capture=True)
        run(["docker", "build", "-f", str(ROOT / "providers/redis/Dockerfile.server"),
             "-t", server_image, str(ROOT / "providers/redis")])
        run(["docker", "build", "-f", str(ROOT / "providers/redis/Dockerfile"),
             "-t", provider_image, str(ROOT / "providers/redis")])
        run(["docker", "build", "-t", probe_image, str(work)])
        started = True
        run(compose + ["up", "-d", "cache", "registry", "observer"])
        run(compose + ["run", "--rm", "--no-deps", "probe", "prepare"])
        run(compose + ["restart", "cache"])
        for _ in range(40):
            try:
                # Verify readiness as the actual nonroot Redis image user.
                run(compose + ["exec", "-T", "cache", "/bin/sh", "-ec",
                    'REDISCLI_AUTH="$(cat /run/manafield/redis/password)" redis-cli ping | grep -qx PONG'],
                    capture=True)
                break
            except subprocess.CalledProcessError:
                time.sleep(0.5)
        else:
            raise RuntimeError("Redis did not become ready after restart")
        run(compose + ["run", "--rm", "--no-deps", "probe", "verify"])
        success = True
        print("PASS: all isolated Redis runtime checks", flush=True)
    except (subprocess.CalledProcessError, RuntimeError, KeyboardInterrupt):
        print("FAIL: runtime probe stopped; no credential values are printed.", file=sys.stderr)
    finally:
        if started:
            # This randomly named Compose project contains ONLY disposable probe
            # services and its own labeled data volume. Never target live projects.
            try:
                run(compose + ["down", "--volumes", "--remove-orphans", "--timeout", "10"])
            except subprocess.CalledProcessError:
                print("Probe cleanup needs retry:", " ".join(compose + ["down", "--volumes"]), file=sys.stderr)
                success = False
        # Images have exact unique probe tags; no prune and no shared image deletion.
        run(["docker", "image", "rm", server_image, provider_image, probe_image], capture=True) if success else None
        if success or not started:
            shutil.rmtree(work)
        else:
            print("Probe-only recovery files retained:", work, file=sys.stderr)
    return 0 if success else 1


if __name__ == "__main__":
    sys.exit(main())
