# Manafield exclusive Redis Resource Provider v0

The generic Provider registers one concrete Redis Resource as
`cache.redis@1.0.0` after an authenticated PING. Core does not proxy Redis data.
The capability includes cache, BullMQ jobs and Pub/Sub: it does not mean that
all stored data is disposable.

## Resource and consumer contract

```yaml
resources:
  - id: noctemblith-redis
    provider: redis
    exclusiveTo:
      module: noctemblith
      slot: cache
# The native Module must be enabled and bind cache: noctemblith-redis.
```

`exclusiveTo` is a generic optional Resource field with exactly `module` and
`slot` (unknown nested fields are rejected). Enabled Redis Resources require
it; existing PostgreSQL/S3 configurations without it retain their behavior.
The owner must be an enabled Module whose exact slot binds the Resource.
Every other Module/slot binding is rejected, including dormant declarations,
and Core cannot bind a Module-exclusive Resource. The policy is preserved in
Build Plan JSON and `resource-exclusive-consumers.tsv`. Jenkins refuses
exclusive Resources until its materializer can enforce that contract.

These are deployment/credential-allocation checks, not a Redis ACL or a
sandbox for a compromised host/Module. Prefixes are only namespacing.
The direct CLI supports one Redis Resource per Manafield Instance for now.
Provider code is reusable; multiple Redis instances are a separate change.

## Runtime and secret ownership

- Internal service `manafield-redis` is independent of Resource ID.
- Redis 7 server and nonroot observer run on the modules network without host
  ports. No other Module receives the password.
- Password: `secrets/redis/<resource-id>/password`, independently generated per
  Resource and stable on restart. It is not shared with legacy SNS Redis.
- Host parent directory is 0700; the consumer file is 0444 behind that parent,
  explicitly set despite restrictive umask. It is mounted as one read-only
  file for Redis, its Provider and the one declared consumer. Incompatible
  existing Redis file permissions fail closed rather than being silently
  relaxed. Validate actual container UID/GID access before deployment.
- No password values in Git, Build Plan, Compose environment or argv.
  Redis creates its private configuration in `/tmp`.
- AOF is enabled with `appendfsync everysec` and explicit
  `maxmemory-policy noeviction`. Memory exhaustion may reject writes; memory
  limits/alerts and backup intervals still require measurements and review.
  AOF does not guarantee zero job loss.
- Direct staging names the data volume by Compose project AND Resource ID.
  Replacing an image/Release with the same IDs preserves data. Changing the
  Resource ID selects a different volume; it does not migrate or delete old
  data. Do not rename an already deployed Resource without operator planning.
- No automatic volume cleanup or legacy SNS retirement.

Consumers receive `MANAFIELD_BINDING_CACHE_TARGET`, `ENDPOINT_HOST`,
`ENDPOINT_PORT`, `CONFIG_PREFIX`, `SECRET_PASSWORD_FILE` (the latter four also
have the `MANAFIELD_BINDING_CACHE_` prefix). Binding injection rechecks the
exclusive owner before adding environment or mounts.

## Validation and deployment boundary

```sh
cargo fmt --all -- --check
cargo test --locked --lib
python3 -m unittest discover -s providers/redis -p 'test_*.py' -q
docker compose --profile redis -f deploy/compose.yml config --quiet
```

These are source/offline checks, not runtime proof. Before any rollout an
authorized Docker operator must verify authenticated PING, rejection without
credentials, read access from each actual nonroot UID, Pub/Sub, a BullMQ job
surviving Redis restart, and stable AOF volume ownership. Pin image
versions/digests then. Full native Noctemblith and SSO E2E is a later stage.

Jenkins still refuses Redis/S3; direct CLI prepares a separate project with
Core/PostgreSQL and is not an approved way to augment the running Jenkins
Instance. Do not publish staged Traefik routes or stop existing SNS instances.

## Operator SSH command

From the Docker-enabled operator account:

```sh
python3 /opt/manafield/ai-workspace/manafield-redis-native-20261016/providers/redis/check-runtime.py
```

The script builds Redis/server Provider images from this checkout and a small
Node test image using Noctemblith's BullMQ/ioredis versions. It runs a uniquely
named disposable Compose project with no host ports or edge network, fresh
test credentials and a new AOF volume. A minimal test registry replaces Core;
no PostgreSQL, R2, Account or existing SNS configuration is used.

It checks real Provider registration, Redis nonroot startup, UID 991 secret
read, rejected unauthenticated PING, Pub/Sub and a real BullMQ job/Worker across
Redis restart. UID/GID matching to a future full SNS image and SNS E2E remain
separate. Success prints PASS, removes this probe's containers/volume/images
and exits 0. On failure it exits nonzero and reports any probe-only recovery
path. It never prunes or deletes existing service volumes.

For syntax/manifest validation without Docker daemon access, add
`--prepare-only`. Base image tags here are for disposable testing; production
image digests must be reviewed/pinned before deployment.
