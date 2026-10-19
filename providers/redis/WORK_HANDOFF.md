# Exclusive Redis / native connection handoff — 2026-10-19

## Decision and current state

- Redis Resource `noctemblith-redis` is exclusive to `noctemblith.cache`.
  Generic `exclusiveTo: {module, slot}` validation applies before credentials.
- Jenkins is the recommended runner. Shared CLI builds, Resource staging and
  selected apply also work without Jenkins. Docker remains the current backend.
- Operator reported isolated runtime PASS: UID 991 secret read, authenticated
  PING, NOAUTH, Pub/Sub, test-registry Provider registration, AOF/BullMQ restart
  restoration and real Worker completion. Probe resources were cleaned up.
- Native PG18/R2/Redis connection, browser SSO and app media E2E are not yet
  established by that probe. No rollout has been performed by the agent.
- User authorized implementation, local commits and operational changes.
  This recipe stops before public route publication and legacy retirement.

## Shared implementation

`stage-resources --definition FILE --instance-root DIR --release-dir DIR`
validates the staged plan/manifests and prepares PostgreSQL Module allocations,
S3 and exclusive Redis bindings/secret-file mounts. Jenkins Stage Release and
direct staging call it. Existing Core logging/Account preparation in Jenkins
still contains transitional duplication. Limit: one Resource per provider.
A completion receipt matching the Build Plan gates S3/Redis apply.

`apply RELEASE_DIR --service NAME [--service NAME ...]` applies only selected
services with no dependency startup, build or orphan removal. Preparation does
not deploy. Apply does not promote active Instance/Release metadata.

Redis uses AOF everysec, noeviction, no host ports, stable Resource-specific
credentials and a project/Resource-specific volume. Consumer secret files are
0444 behind private 0700 host directories and individually mounted read-only.
Existing incompatible Redis permissions fail closed. S3 runtime copies receive
explicit 0444 permissions, including matching copies created under restrictive
umask; master credentials are not modified or rotated.

## Existing deployment evidence

Actual Core is project `manafield`, release 58. PG18/data volume are already in
that project; Account Core/Profile use newer image overrides. Archived
`instance/current` is stale and must not be used as the active graph.
Read-only sanitized inventory:
`/opt/manafield/ai-workspace/work/noctemblith-native-connect-20261019/docker-inventory.json`.

Source worktrees:
- `/opt/manafield/ai-workspace/manafield-redis-native-20261016`
- `/opt/manafield/ai-workspace/noctemblith-native-redis-20261016`

Candidate:
`/opt/manafield/ai-workspace/instance/instance.candidate-noctemblith-native-20261016.yaml`.
Only this inactive candidate was edited; active Instance configuration remains
unchanged. Local commits descend from PR #30/#27; no push or merge.

## SSH execution

Run as the authorized operator:

```sh
sudo python3 /opt/manafield/ai-workspace/manafield-redis-native-20261016/deploy/noctemblith-native-connect.py
```

Optional read-only checks: append `--preflight`.
The committed recipe archives exact Git revisions and builds through the shared
CLI. It takes base/modules configuration from live Docker labels, pins existing
project services to actual image IDs, preserves Account overrides and PG volume,
then stages PG18/R2/Redis bindings in a new private release directory.

It recreates Core (new manifest directory) and the PG Provider (new allocations),
starts S3/Redis services, checks bound-file reads/PG login from the actual app
image, runs the reviewed baseline migration in `mf_noctemblith_state`, then
starts the native app internally and checks health, SSO-enabled metadata and
unchanged public table count, plus live Core registration of PG/S3/Redis. Core can briefly be unavailable during recreation.

A pre-existing populated target schema is refused; failures never reset it.
A partial migration needs operator inspection before retry. Generated artifacts
and recipe/result JSON remain in the printed release directory. An unsuccessful
command exits nonzero; keep that directory for diagnosis.

The public route still targets old SNS. Active Instance/current metadata is not
promoted. After internal success, inspect Resource registration and run browser
SSO/media E2E before explicitly preparing public cutover. Old SNS shutdown and
volume retirement are separate; public PG/R2/Account are not cleanup targets.

## Validation boundaries

Rust library/shared staging, Provider/Compose offline suites, CLI compilation,
Python syntax, Jenkins shell syntax and native shipping checks cover source
behavior. See operator output for actual Docker runtime results. The remote
agent has no Docker socket/root-secret access; do not change socket ownership
or expose credentials to bypass that boundary.

Source validation for this update: Rust 111/111; native recipe offline 3/3
(Docker execution simulated, real CLI plan/stage and Compose config); Redis
Provider/Compose 10/10; S3 Provider 10/10; Python syntax; 14 Jenkins shell
blocks; native shipping SPDX/locale PASS (changed-file lint SKIPPED: docs only).
Runtime Docker deployment remains unexecuted by the agent.
