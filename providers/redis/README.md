# Manafield Redis Resource Provider v0

This provider represents one *concrete* Redis Resource Instance. It checks an
authenticated PING and advertises `cache.redis@1.0.0` through the generic
Manafield Resource registry. No application cache bytes pass through Core.

## Layout

- `manafield-redis`: Redis 7 with AOF persistence in its own named volume.
- `manafield-redis-provider`: nonroot observer and resource registrar.
- `src/cli/staging/redis.rs`: private credential file and consumer binding.
- One Redis Resource per v0 Instance; multiple trusted first-party Modules can
  bind it. It is an additive service, not a replacement for PostgreSQL or S3.

The direct CLI creates a stable password in the private Instance root at
`secrets/redis/<resource-id>/password` if absent, then mounts it read-only into
the Redis service, the Provider and each explicitly bound consumer. No raw
password is placed in the Build Plan or Compose environment. Container startup
creates an ephemeral Redis configuration file under `/tmp`; passwords are
**not** written to process argv.

Consumers with Requirement slot `cache` and Capability `cache.redis@^1.0.0`
receive these binding fields:

```text
MANAFIELD_BINDING_CACHE_TARGET
MANAFIELD_BINDING_CACHE_ENDPOINT_HOST
MANAFIELD_BINDING_CACHE_ENDPOINT_PORT
MANAFIELD_BINDING_CACHE_CONFIG_PREFIX
MANAFIELD_BINDING_CACHE_SECRET_PASSWORD_FILE
```

Redis is not published on a host port; traffic stays within the managed
Module network. The per-Module prefix is for key namespacing **only**, not
permission isolation. All consumers holding this Redis password can access
other keys/channels. Do not bind untrusted Modules without proper Redis ACLs.

## Verification and deployment scope

```sh
python3 -m unittest discover -s providers/redis -p 'test_*.py' -q
cargo test --locked --lib
docker compose --profile redis -f deploy/compose.yml config --quiet
```

The tests use fake Redis and HTTP clients. A real Redis Docker runtime smoke
test and Noctemblith integration boot are still required. Jenkins will reject
Redis/S3 resources until its private binding and profile wiring is implemented;
the supported initial staging path is the direct CLI. Do not delete any
previous SNS database/Redis volumes until the replacement app has booted.
