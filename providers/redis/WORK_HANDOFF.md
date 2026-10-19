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

## Native connection confirmed / public cutover authorized (2026-10-19)

Operator reported successful internal connection at:
`instance/releases/native-20261019T005014Z`.
Actual Core registered PG/S3/exclusive Redis; native health and SSO-enabled
metadata passed. This is an operator runtime result, not browser/media E2E.

User then explicitly requested replacing the old public deployment with the
new native version. The reviewed operator command is:

```sh
sudo python3 /opt/manafield/ai-workspace/manafield-redis-native-20261016/deploy/noctemblith-native-cutover.py
```

The script discovers the running native Release from Docker labels, checks its
exact build/health/SSO receipt and unchanged public origin, joins it to
`manafield-edge` under the unique `noctemblith-native` alias, and persists that
attachment in its staged Module Compose file. It changes only the backend of
`dynamic/noctemblith-dev.yml` to port 8080. HTTPS health, exact source revision
and SSO-enabled metadata must pass before stopping the old
`noctemblith-dev` app, then its dedicated DB/Redis. Verification repeats after
the stops. Standalone `noctemblith-only`, shared PG/R2/Account and every existing
data volume remain untouched; no old DB/media migration is performed.

Before route publication it backs up route/modules configuration into a private
`cutover-<timestamp>-<suffix>` directory under the native Release. Public-check
failure restores the route/config and disconnects only an edge attachment
created by this run; the old SNS is not stopped. Failed recovery is explicitly
reported. A failure while retiring old containers leaves the proven new route
in place and records incomplete retirement in `result.json`.

Append `--preflight` for read-only validation. Cutover itself remains to be
executed by the SSH operator because the agent cannot write the Traefik
directory or access Docker. Offline cutover, rollback and read-only tests passed.
Public browser SSO and app media E2E remain outstanding. Active Instance/current
metadata is not promoted: do not run the old full Jenkins definition as though
it already described this new deployment. Promote the reviewed native source
and Instance definition separately before the next full Jenkins deployment.

## Public cutover reported / initial administrator recovery (2026-10-19)

Operator reported cutover success at 10:39 KST; recovery directory:
`native-20261019T005014Z/cutover-20261019T013901Z-fcaccd`.
Public HTTPS native build/health/SSO passed and old dev app/DB/Redis stopped.
User subsequently authorized deleting old dev data; the exact old project's
`docker compose -p noctemblith-dev -f deployments/noctemblith-dev/docker-compose.yml down --volumes`
command was provided. Actual volume deletion has not been verified by the agent.

The user reached the first-setup screen. Inspection of the Release source found:
- Managed runtime configuration does not populate `setupPassword`.
- SSO mode unconditionally rejects `admin/accounts/create`, including first setup.
- No `/auth/local-admin` route exists in this Release source. Do not claim that
  the separate 2026-10-16 login/admin-route candidate is in this native image.
  Review/reconcile that candidate separately before claiming login policy compliance.

An operator-only recovery helper is prepared (syntax checked, NOT runtime tested):

```sh
sudo python3 /opt/manafield/ai-workspace/manafield-redis-native-20261016/deploy/noctemblith-native-bootstrap-admin.py
```

It checks native ownership/revision/health/SSO and the precise managed schema,
then refuses any existing root, user or reserved username. Password is entered
interactively without echo (no credential files or printed tokens). The helper
uses the actual image's exported TypeORM entities to atomically create the
User/Profile/Keypair/UsedUsername records and assign root with registration
closed. This is a narrow empty-instance recovery procedure, not a replacement
for the application SignupService: chart/webhook side effects are not invoked.
It restarts only the native app to refresh cached metadata and checks health/SSO.
Public API guards and all central/standalone accounts remain unchanged.
`--preflight` runs only read-only validation. Both adjacent helper files are
required. Actual creation remains unexecuted because the remote agent has no
Docker access. Browser administrator login and SSO/media E2E remain outstanding.

## Bootstrap guard correction (2026-10-19)

Operator reported the first recovery attempt stopped before password entry:
`Bootstrap refused: instance is not empty`. This was a read-only preflight;
no administrator was created or existing data removed by that attempt.

Release source confirms MetaDetailed packing/NodeInfo can create system.proxy
before a human administrator exists. The previous zero-user/reservation check
incorrectly refused such a fresh installation. The helper now accepts only
actor/relay/proxy users explicitly mapped by system_account, with exact local
system.TYPE usernames, bot/locked flags and matching username reservations.
Ordinary users, existing root, orphaned/forged system mappings and unrelated
reserved usernames still cause refusal. System records remain untouched.
The apply transaction locks metadata/users/system mappings/reservations before
repeating validation. No runtime bypass or destructive reset was added.

The Python wrapper now loads adjacent noctemblith-native-bootstrap-guard.mjs
alongside the existing admin.mjs; all three helper files are required.
Eight guard regression tests and combined Node/Python syntax checks passed.
Actual Docker/database preflight after this correction remains unexecuted by
the agent; the same operator command can be retried. Output now lists verified
internal system account types before password entry, without secret values.
