# Manafield CLI / Docker v0 Execution Guide

> Baseline: Manafield v0, 2026-10-08. Current implementation is distinguished from the target.
>
> The Korean documentation takes precedence. See [ADR-0013](adr/0013-docker-v0-cli-execution.md).

## Prerequisites

A supported **full Manafield v0 Instance build and deployment requires Docker**:
the Docker CLI, Docker Compose plugin, and access to a Docker daemon.
Jenkins is the first optional CI/CD frontend, not a Manafield installation requirement.

Independent Core API / Registry / validation work and `manafield plan` may run without Docker.
That does not imply a full Docker-free v0 Instance deployment.

## CLI status

| Command | Status | Purpose |
| --- | --- | --- |
| `manafield serve` | Implemented | Run Core API (also the no-argument default) |
| `manafield health` | Implemented | Query Core health |
| `manafield ps` | Implemented | Query Core, Modules, Resources |
| `manafield resource [id]` | Implemented | List or inspect Resources |
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
