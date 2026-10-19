# Exclusive Redis Resource — 2026-10-19 handoff

Status: stage B source implemented; isolated Redis runtime PASSED (operator report 2026-10-19 09:04). Native SNS connection preparation is in progress.
User approved option A (generic Resource exclusive consumer contract).
User subsequently requested local commits and operator SSH commands.
Local commits continue from the existing Draft PR commits; no push, merge,
deployment, ingress publication, SNS retirement or live credential/volume
change is performed by this source preparation.

## Source changes

- Instance Resource field: exclusiveTo: { module, slot }. Required for enabled
  Redis, optional for other Resource types. Exact owner Module+slot validation,
  including forbidden disabled consumers and Core references.
- Policy retained in Build Plan JSON and CI metadata. Jenkins refuses exclusive
  Resources until materialization supports the policy.
- Redis injection rechecks the owner before environment/secret mounts.
- Secrets generated independently per Resource ID. New consumer files use
  explicit 0444 permissions behind private 0700 parent despite umask; existing
  incompatible Redis file permissions are refused without automatic repair.
- AOF everysec, noeviction, private network/no published Redis host ports.
- Direct-staged data volume name includes Instance Compose project and Redis
  Resource ID. Existing data is never renamed/deleted automatically.
- Fixed pre-existing shell command splitting: Redis startup is now passed as
  one argument to /bin/sh -ec. Offline tests execute the rendered startup with
  fake Redis, disposable credentials/config and no daemon.
- Provider guide, generic build-plan ADR (KR/EN), and native Noctemblith guide
  updated. No new backend behavior change was needed.
- Only inactive Instance candidate was changed to noctemblith-redis and owner
  noctemblith.cache. Active instance.yaml and credentials were not edited.

## Checks

PASS: Rust library suite 104/104 before final permission regression addition.
PASS: Redis staging tests 6/6 including that added permission regression.
PASS: Rust suite under umask 077; fmt; build-plan binary build.
PASS: Provider/Compose Python tests 10/10.
PASS: Compose Redis+S3 config; readonly mounts/no published Redis port checks.
PASS: actual active Instance and candidate resolve using the new binary.
PASS: invalid owner slot/extra consumer slot/missing policy are rejected
      before output JSON or CI directory creation.
PASS: stock example/bootstrap definitions resolve.
PASS: native Noctemblith shipping checks (changed-file lint, SPDX, locale).
PASS: git diff --check for both worktrees.
No real Redis PING, BullMQ/PubSub, container UID/GID or persistence test was run.

## Workspace state

Existing worktrees retain the prior uncommitted WIP plus these modifications:
- /opt/manafield/ai-workspace/manafield-redis-native-20261016
- /opt/manafield/ai-workspace/noctemblith-native-redis-20261016
Both worktrees are aligned to their already available PR heads without
changing working contents: Manafield 45372ad, Noctemblith 6ff3de3.
The new local commits are descendants of those PR commits. No remote branch
is updated until a later push. Use git log/status to inspect exact local tips.

Inactive candidate:
 /opt/manafield/ai-workspace/instance/instance.candidate-noctemblith-native-20261016.yaml

## Remaining boundary

manafield-agent has no Docker socket access. Use the existing operator account
for isolated, nonproduction runtime verification; do not change socket ownership
or copy credentials into chat. Before any existing Instance update choose the
executor path explicitly: Jenkins still fails closed for S3/Redis, while direct
CLI would prepare a separate Core/Postgres project. Do not run direct deploy
against the active Instance merely because the candidate now validates.

Runtime checks required: actual nonroot Redis/Provider/SNS secret reads,
authenticated PING and rejected unauthenticated access, Pub/Sub, BullMQ restart
recovery, data volume ownership and resource registration. Then stage C native
PG18+R2+Redis/SSO/media E2E. Cutover, legacy SNS/volume retirement remain later
separate approvals. Public PostgreSQL/R2/Account resources are not cleanup targets.

## Operator runtime probe prepared

Run from the Docker-enabled SSH account:
python3 /opt/manafield/ai-workspace/manafield-redis-native-20261016/providers/redis/check-runtime.py

The --prepare-only form passed (Compose generation, no ports, Node syntax).
The actual Docker run remains for the operator. This probe uses only a random
new test project, fresh credentials, test registry and AOF volume. It exercises
authenticated/unauthenticated PING, Provider registration, Pub/Sub and real
BullMQ Worker recovery after Redis restart. Full SNS image UID compatibility
and native PG18/R2/SSO E2E remain later checks.
User now permits operational service changes; that does not require choosing
a separate-Core direct CLI deployment or deleting unrelated resources.

## Isolated runtime result and native connection inventory

Operator ran check-runtime.py successfully and reported UID 991 secret read,
authenticated PING, NOAUTH, Pub/Sub, Provider registration against the test
registry, AOF/BullMQ restoration after restart and actual Worker completion.
Probe containers, network and volume were removed. This does not establish
registration against the live Core or a native SNS/SSO E2E result.

The archived instance/current Compose is an old example-postgres/reference
layout and does not match the current Instance definition with Account modules.
Use actual Docker Compose labels and network membership before selecting
the incremental deployment path; do not use that archived Compose blindly.
Read-only operator command:
python3 /opt/manafield/ai-workspace/manafield-redis-native-20261016/providers/redis/inspect-native-connection.py

Sanitized result path:
/opt/manafield/ai-workspace/work/noctemblith-native-connect-20261019/docker-inventory.json
Only allowlisted nonsecret env metadata, paths, networks and image identities
are saved. No live service changes are performed by this inspection.
