# ADR-0014 — Instance Working Copy, Immutable Releases, Persistent Resources

- Status: Accepted (architecture decision; CLI syntax and implementation are planned)
- Date: 2026-10-08
- Related: [ADR-0006](0006-instance-build-plan.md), [ADR-0010](0010-capability-dependency-resolution.md), [ADR-0013](0013-docker-v0-cli-execution.md)

## Problem

The current Jenkins pipeline combines Instance configuration, Module source builds, Resource preparation, release artifact staging and deployment. This mixes platform builds with Instance operations and encourages a `manafield build` command to build unrelated third-party Modules.

Operators instead need to reuse existing Resources, edit Module/Binding topology, and name a fixed configuration before deploying it.

## Decision: four separate concerns

1. **Platform Build** builds Manafield itself (CLI, Core, official execution components/Providers); independently published Modules are not part of `build all`.
2. **Instance Definition / Working Copy** is mutable YAML describing Module/Resource Instances and Requirement-slot Bindings.
3. **Release** is an **immutable YAML configuration snapshot**. It is not a new Resource instance, copy of data, or mandatory Module rebuild.
4. **Runtime State** is the actual running services, volumes, databases, and their lifecycle; it is not the same as a Release file.

CLI and Core server become **separate binaries shipped in the same distribution**. The CLI dispatches to shared management/deployment code; Docker privileges remain outside the Core server process.

## Binding is a logical reference

```text
manafield module bind <consumer-id> <requirement-slot> <target-instance-id>
manafield module bind echo-prod state main-postgres
```

This declares `echo-prod.requires.state -> main-postgres` in the working copy, pointing at a concrete Module or Resource Instance ID, without merging programs or cloning resources. Validation and diagnostics are separate from selecting a target. Do not auto-bind compatible candidates; preserve ADR-0010's warning-on-version-mismatch behavior.

Binding edits do **not** mutate the live runtime.

## Select, edit, deploy

Proposed operator workflow:

```text
manafield use A
manafield module bind echo-prod state main-postgres
manafield deploy B
```

- `use A` loads an immutable Release YAML into a **mutable private temp working copy**, recording A as its base. It does not deploy A. **An explicit `use A` intentionally replaces the previous temp copy, discarding unsaved edits without confirmation.**
- `module bind` and future editing commands modify only that working copy. When no working copy exists, choose an existing temp copy, else the active Release, else the initial `instance.yaml`. If none exists, report a missing-initialization error rather than create an empty YAML. Read-only commands do not create a working copy. Manual YAML editing must target the same source of truth.
- When B is a **new Release ID**, `deploy B` validates and snapshots the working copy as immutable B **before** attempting deployment. Preserve B even if deployment fails.
- When B **already exists**, deploy the saved B verbatim. Dirty working-copy changes must cause a conflict, not be silently dropped or written over B.
- An argument-free `deploy` may generate a new Release ID and print it. Re-deploy an existing Release explicitly using `deploy A`.
- `use` changes the selected editing baseline, not the live deployment. Record the last selected base Release in `manafield/temp/context.json` and the successful live Release separately in `manafield/state/active-release.json`. CLI restarts do not discard temp edits.

These commands are a **target design**, not currently implemented syntax. A separate `release create` is not required initially; snapshotting belongs to the new-release deploy transaction. A save-without-deploy operation may be added later.

## Paths and ID

Planned private per-instance layout:

```text
<instance-root>/
└── manafield/
    ├── release/
    │   ├── v1_20261008T070000Z.yaml
    │   └── v2_20261008T080000Z.yaml
    ├── temp/
    │   ├── working.yaml
    │   └── context.json
    └── state/
        └── active-release.json
```

- Default ID: `v<positive integer>_<UTC YYYYMMDDTHHMMSSZ>`; the integer monotonically increases per Instance. Never overwrite or reuse an existing ID.
- The private working copy may persist across invocations. Invalid or missing Release IDs must not discard existing edits even when `use` was invoked. Do not store raw secrets in the working copy or Release YAML.
- Pin concrete source/version references and binding target Instance IDs in the snapshot. Image digests/manifests may be referenced in separate immutable metadata when needed for reproducibility. **Release IDs are not Platform or Module versions.**
- Write immutable files atomically. Store deployment success/failure history separately rather than editing the Release YAML.

Today's Jenkins `<instance-root>/releases/<BUILD_NUMBER>/` stores staged Compose artifacts, **not** the planned `manafield/release/<id>.yaml` snapshots. No automatic migration is implied.

## Reconciliation and persistent Resources

Deploying a Release applies its **desired configuration** to the current Runtime State.

- Reuse an existing PostgreSQL Resource, named volume, schema, role, and database content for the same Resource Instance ID. Do not recreate or wipe it because the Release changed.
- Create/register only genuinely missing Resources, subject to checks. Third-party Module artifacts are built/published independently; `build all` does not rebuild them.
- A general-purpose Release-to-Release diff display/engine is **not required for v0**. The deployment engine still needs minimum desired-versus-actual reconciliation, stable identity, and idempotence.
- Never implicitly perform destructive cleanup. Existing Jenkins `docker compose up --remove-orphans` behavior must be reviewed against this policy.
- Major database engine upgrades, schema migrations, restarts, backups, and product-specific recovery are outside this ADR; persistent data and explicit destructive-operation approval remain mandatory boundaries.

## Failure and recovery

An immutable YAML snapshot allows retrying or re-applying an earlier desired configuration. It does **not** automatically restore partially modified runtime state, changed external systems, or database content. Transaction logs, health checks, backups, and explicit recovery procedures remain separate concerns. No automatic data rollback is promised in v0.

## Migration work

- Split the `manafield` CLI binary from `manafield-core`, shipping both together, with shared management/deployment logic.
- Redesign the `build` command family for **Manafield platform artifacts only**. The current Jenkins-compatibility `manafield build WORKSPACE REVISION CORE_IMAGE` also builds external Modules, which conflicts with this target.
- Working-copy `use` and `module bind` are implemented in CLI v0. Immutable Release YAML creation, `deploy <release-id>`, and persistent Resource reuse remain future work.
- Maintain existing Jenkins/CLI `plan`, legacy `build`, and `deploy RELEASE_DIR` during the migration; do not represent them as already following this new contract.

## CLI implementation status (2026-10-08)

`manafield module bind A B C` updates only temp working YAML. If missing, the working copy is initialized from the last deployed Release (`release_id` in `manafield/state/active-release.json`) or initial `instance.yaml`. Immutable Release YAML and live runtime are untouched. YAML serialization may not preserve comments/formatting; full Capability compatibility validation remains future work.

## CLI Release deployment bridge (2026-10-08)

`manafield deploy [release-id]` now validates a working copy and atomically commits a new immutable YAML snapshot (or applies an existing one unless dirty working edits conflict). Without an ID it allocates `vN_UTCtimestamp`; `--snapshot-only` stops after saving.

Runtime application currently requires `--staged-dir DIR` with pre-created Compose artifacts and `build-plan.json`. The staged Build Plan must match the Release or Docker execution is rejected. Success currently means **Compose application and Core HTTP health**, not full Module/Resource health or ingress verification.

The new path avoids `--remove-orphans` and records the last attempt and active Release separately. The old Jenkins-compatible `deploy RELEASE_DIR` stays unchanged. Immutable YAML alone does not pin all image digests, mutable external source refs or runtime data.

## Direct Instance v0 deployment limitations (2026-10-08)

Without `--staged-dir`, `deploy` prepares source/build/Compose artifacts from local Manafield Core source (`--source`, default cwd) for non-exposed Git/dir Modules and at most one PostgreSQL Resource. Git refs are fetched and checked out, and image tags include resolved commit IDs; `--modules-root` locates local Module directories. Other Resource providers and ingress/exposure fail explicitly.

Direct mode currently records active Release after Core HTTP health, not full Module/Resource verification or DB migration. Existing PostgreSQL volumes and secrets are not deleted, but credential migration from Jenkins-managed installations is not automatic. The fixed Compose project `manafield` limits multi-Instance operation. Real Docker integration remains necessary before calling it production-ready.
