# Manafield CLI / Docker v0 Execution Guide

> Baseline: Manafield v0, 2026-10-08. Current implementation is distinguished from the target.
>
> The Korean documentation takes precedence. See [ADR-0013](adr/0013-docker-v0-cli-execution.md).

## Working copy and Release CLI — implementation status

The workflow from [ADR-0014](adr/0014-instance-working-release.md) is being implemented. **`build all`, `use` and `module bind` are implemented; Release-ID `deploy` is not yet implemented.**

```bash
manafield build all
manafield use v1_20261008T070000Z
manafield module bind echo-prod state main-postgres
manafield deploy v2_20261008T080000Z
manafield deploy v1_20261008T070000Z
```

- `build all` builds the **Manafield platform only**, not independently published Modules.
- `use A` copies immutable Release A into `<instance-root>/manafield/temp/working.yaml`, discarding prior unsaved edits on explicit `use A` without changing live deployment.
- `module bind A B C` edits the temp YAML Requirement slot B of consumer A to point at Module/Resource Instance C; it neither deploys nor copies a Resource.
- `deploy B` with a new ID validates and atomically snapshots the working copy at `<instance-root>/manafield/release/B.yaml` before applying it. Existing IDs reapply saved YAML unchanged, rejecting conflicting dirty edits.
- Default IDs: `v<positive integer>_<UTC YYYYMMDDTHHMMSSZ>`, never reused or overwritten.
- Preserve existing PostgreSQL identities, volumes, schemas, roles and data across Releases. YAML retries do not restore database data.
- A full Release-to-Release diff engine is not required immediately, but runtime reconciliation and non-destructive idempotence are.

Currently, transitional `manafield build WORKSPACE REVISION CORE_IMAGE` **does build external Modules**, and `manafield deploy RELEASE_DIR` only invokes Compose on a pre-staged artifact directory. Neither implements this target contract.

## Select a Release working copy — no Docker required

```bash
manafield use v1_20261008T070000Z --instance-root /path/to/instance
manafield use --instance-root /path/to/instance
```

The Instance root comes from `--instance-root`, then `MANAFIELD_INSTANCE_ROOT`, then the current working directory. Read `manafield/release/<id>.yaml`, stage selected YAML at `manafield/temp/working.yaml`, and persist selection in `manafield/temp/context.json`.

Explicit `use A` discards prior unsaved temp edits. A missing or malformed Release must leave the previous working copy intact. Running `use` without an ID shows the selected base and whether its YAML was edited. No Docker commands or live Instance changes occur.

Without an explicit `use`, `module bind` now starts from existing temp, else the last deployed Release recorded in `manafield/state/active-release.json`, else initial `instance.yaml`; if none exists it reports an error. The planned active state JSON schema is `{"release_id":"vN_YYYYMMDDTHHMMSSZ"}`; recording it on successful deployment remains future work.

## Edit Module Bindings — no Docker required

```bash
manafield module bind echo state main-postgres --instance-root /path/to/instance
manafield use --instance-root /path/to/instance
```

`module bind A B C` updates `modules[].bindings.B: C` in the **temp working YAML only**. A must be a unique Module ID; C must be a unique enabled Module or Resource Instance ID in the Instance Definition. B matches `[A-Za-z_][A-Za-z0-9_]*`.

- Missing temp: initialize from active Release, or initial `<instance-root>/instance.yaml`; never silently create an empty YAML
- Missing/disabled target or malformed YAML: report an error **without** producing new temp state
- Repeated binds preserve earlier Binding values; no live runtime or Docker mutation
- Working YAML is `manafield/temp/working.yaml`; selection context is `manafield/temp/context.json`
- **Comments and formatting may be rewritten by the YAML serializer.** Review the result when manually editing configuration
- Full Requirement/Capability compatibility checks are outside this command's scope and will be handled by the validation/deployment flow

## Build the Manafield platform directly (without Jenkins)

From the Manafield source checkout, `build all` compiles **four Rust binaries**: CLI, Core server, Build Plan compatibility tool, and Traefik ingress adapter. By default, it also builds the Core runtime and official PostgreSQL Resource Provider Docker images.

```bash
cargo run --bin manafield -- build all
# Or invoke an already built CLI:
cargo build --locked --bin manafield
./target/debug/manafield build all
```

The default output is `dist/platform/<version>-<Git revision>/bin/` plus `manifest.json`, recording source revision, dirty state, and any Docker image tags. Docker images remain in the local Docker daemon; they are not embedded in `dist`.

```bash
manafield build all --no-docker --output ./dist/local-binaries
manafield build all --source /path/to/manafield --output /tmp/manafield-test
```

Output directories are never overwritten. `--no-docker` builds binaries without requiring daemon access; the default requires Docker CLI/daemon permissions. Instance YAML, external Modules, and database contents are not built or changed.

The old `build WORKSPACE REVISION CORE_IMAGE` command remains available temporarily for Jenkins compatibility only.

## Separate CLI and Core binaries

`manafield` is the CLI and `manafield-core` is the HTTP server; they are separate executables shipped in the same Docker Core image/distribution. Run `manafield-core` to start the API. The CLI defaults to help. Jenkins can still extract the CLI from `/usr/local/bin/manafield`.

## Prerequisites

A supported **full Manafield v0 Instance build and deployment requires Docker**:
the Docker CLI, Docker Compose plugin, and access to a Docker daemon.
Jenkins is the first optional CI/CD frontend, not a Manafield installation requirement.

Independent Core API / Registry / validation work and `manafield plan` may run without Docker.
That does not imply a full Docker-free v0 Instance deployment.

## CLI status

| Command | Status | Purpose |
| --- | --- | --- |
| `manafield-core` | Implemented | Run Core API as a separate binary |
| `manafield` (no args) | Implemented | Print CLI help |
| `manafield health` | Implemented | Query Core health |
| `manafield ps` | Implemented | Query Core, Modules, Resources |
| `manafield resource [id]` | Implemented | List or inspect Resources |
| `manafield module bind A B C [--instance-root DIR]` | Implemented | Edit Module Requirement Binding in temp YAML only |
| `manafield use [RELEASE_ID] [--instance-root DIR]` | Implemented | Select an immutable YAML into a working copy or inspect current selection; explicit switching discards unsaved edits |
| `manafield build all [--source DIR] [--output DIR] [--no-docker]` | Implemented | Produce a Manafield-only platform bundle and, by default, Core + official PostgreSQL Provider Docker images |
| `manafield plan [INSTANCE]` | Implemented | Validate and resolve an Instance Definition |
| `manafield deploy RELEASE_DIR` | Implemented | Docker Compose deployment of an already staged release |
| `manafield build WORKSPACE REVISION CORE_IMAGE` | Implemented | Build Core/Module/Resource Docker images from a prepared Jenkins workspace |
| `manafield verify` / `rebuild` | Not implemented | Future incremental Jenkins migrations |

## Resolve a Build Plan

```bash
manafield plan instance.yaml \
  --output build-plan.json \
  --ci-output ci-plan
```

## Deploy a staged release

```bash
manafield deploy /opt/manafield/instance/releases/123
```

A staged release requires `release.env`, `compose.yml`,
`modules.compose.yml`, and `compose-profiles.txt`.
The CLI applies the staged `COMPOSE_PROFILES`, runs `docker compose up -d
--no-build --remove-orphans`, then runs `docker compose ps` with the same
release files. It needs existing images and does not yet checkout sources, build
images, materialize PostgreSQL bindings, stage a release, publish ingress, or
perform post-deployment Protocol verification.

## Jenkins boundary

The Jenkins `Resolve Build Plan`, `Build Images`, and `Deploy` stages invoke the shared CLI.
The remaining pipeline stages are still migrating. Jenkins retains CI triggers,
approvals, credential integration, and build history; Manafield-specific execution
semantics should move into CLI/reusable executors over time.

Do not mount `docker.sock` directly into the Core server container.
The current Docker commands execute from the host/CI environment running the CLI,
not from the Core API process; the future privileged Runtime Provider boundary remains separate.

CLI + Docker rebuilding a complete Instance without Jenkins is the target,
**not yet a verified end-to-end implementation**.

## Build images from a prepared workspace

```bash
manafield build "$WORKSPACE" "$CORE_SHA" "$CORE_IMAGE"
```

Reads `module-sources.tsv` (8 columns) and optional
`ci-plan/resources.tsv` (2 columns) to build Docker images for Core,
Modules, and PostgreSQL Resource Providers. Preserves the existing
`resolved-images.env` and `resource-providers.tsv` output formats.

Source checkout, Build Plan resolution, binding materialization, and release
staging remain separate. Jenkins must prepare the workspace; a complete
CLI-only first-time Instance build is still a future goal.
