# Manafield Instance Definition

This directory contains deployment-composition examples for a Manafield instance.

## v0 execution baseline

The Core runtime Docker image contains separate executables: `/usr/local/bin/manafield` for CLI and `/usr/local/bin/manafield-core` for the Core HTTP server. Its container entrypoint is `manafield-core`, while Jenkins can still extract the CLI at the existing path.

A supported full Manafield v0 Instance build/deployment requires Docker.

The canonical local execution path is the `manafield` CLI + Docker. Jenkins is an optional remote CI/CD frontend that should call the same CLI/reusable execution logic rather than own Manafield-specific semantics.

```text
Local terminal ─┐
                ├─> manafield CLI ─> Docker
Jenkins ────────┘
```

See [ADR-0013](../docs/en/adr/0013-docker-v0-cli-execution.md).

The Instance Definition answers:

> **Which Core, Runtime Providers, and Modules belong to this Manafield instance?**

It does **not** replace `manafield.module.json`.

```text
manafield.module.json
→ Module capability contract
→ ID / version / Operations / Health reference

instance.yaml
→ Instance composition
→ Core / Runtime Providers / Module sources

Build Plan
→ Resolved CI/CD work
→ exact revisions / image tags / affected components
```

## Immutable Instance Release YAML (ADR-0014, supported v0)

The Jenkins `<instance-root>/releases/<BUILD_NUMBER>/` continues to store pre-staged Compose artifacts. The **separate** Release-ID direct deployment command now implements immutable YAML snapshots within its supported v0 scope; Jenkins and CLI Release histories are not interchangeable.

```text
<instance-root>/manafield/release/v1_20261008T070000Z.yaml
<instance-root>/manafield/temp/working.yaml
```

Implemented: `manafield use A` copies immutable A into an editable temp YAML, `manafield module bind A B C` changes a Requirement-slot-to-Instance-ID reference, and `manafield deploy B --snapshot-only` saves a new immutable B. `manafield deploy B --staged-dir DIR` validates a prepared Build Plan against B and applies it without `--remove-orphans`. Without `--staged-dir`, the CLI now directly builds Core and Git/dir Module images and prepares Compose for non-exposed Modules with up to one PostgreSQL Resource. Ingress publication, other providers, live database credential reconciliation and full Module/Resource health remain follow-up work.

`build all` will build Manafield platform components only. Today's Jenkins compatibility `build WORKSPACE REVISION CORE_IMAGE` still builds external Module images. Existing PostgreSQL Resources and persistent volumes/data must survive Release changes. Reapplying an earlier YAML **does not** restore previous database data, and the existing `--remove-orphans` deployment flag must be reviewed for safety.

See [ADR-0014](../docs/en/adr/0014-instance-working-release.md). The remainder of this guide describes the **current** Jenkins deployment implementation, not the future Release CLI.

## Public example vs private instance

The files in this directory describe the public deployment contract and examples.

`instance.yaml.example` is the copy-ready example intended to be duplicated as `instance.yaml` and customized for an instance. It is intentionally safe to publish.


A real production `instance.yaml` is **private by default** and should be managed outside the public Core repository. The repository ignores `deploy/instance.yaml` to reduce the chance of accidentally committing a real instance definition.

A real instance may freely mix:

- public Modules
- private Modules
- public Integrations
- private Integrations

Source visibility does not change the Manafield Protocol contract. CI/CD simply needs the appropriate credentials for private sources.

Secrets still do not belong in a private `instance.yaml`; inject them through executor/runtime credential mechanisms instead.

See [ADR-0007](../docs/en/adr/0007-public-platform-private-instance.md) for the architectural decision.

## Draft v0

See [instance.yaml.example](instance.yaml.example).

The current shape is intentionally small:

```yaml
version: 0

instance:
  id: manafield-local

core:
  source:
    repository: ...
    ref: main

runtimeProviders:
  - id: docker
    enabled: true

modules:
  - id: manafield-reference
    enabled: true
    source:
      repository: ...
      ref: main
    build:
      type: docker
      context: .
      dockerfile: Dockerfile
    exposure:
      type: host
      host: reference.example.test
      targetPort: 8080

deployment:
  modulesNetwork: manafield-modules
  edgeNetwork: manafield-edge
  ingress:
    provider: traefik
    output: ./dynamic/manafield.yml
```

The **Build Plan Resolver** consumes this file and produces an executor-neutral JSON plan.

The canonical CLI entrypoint is:

```bash
manafield plan deploy/instance.yaml.example \
  --output build-plan.json \
  --ci-output ci-plan
```

Without `--output`, the resolver prints the JSON plan to stdout. The standalone `manafield-build-plan` binary remains a compatibility wrapper while Jenkins stages move toward the main CLI.

See [build-plan.example.json](build-plan.example.json) for the current normalized output.

### First-run bootstrap

The current Jenkins frontend implements a one-time Bootstrap Wizard when it can see `INSTANCE_ROOT` but `<INSTANCE_ROOT>/instance.yaml` does not exist yet.

The Wizard is intentionally shown **only when the Instance Definition does not exist**. Existing instances continue directly to normal builds.

The bootstrap choices are:

- Manafield PostgreSQL Resource — implemented
- Example Web Module — planned
- Example Account Module — planned

Selecting Example Account implies both PostgreSQL and Example Web. Initial administrator creation is intentionally a separate explicit setup flow; v0 documentation no longer assumes an automatic `admin / admin` seed.

Selecting only PostgreSQL appends a desired Resource request to the generated `instance.yaml`:

```yaml
resources:
  - id: manafield-postgres
    enabled: true
    provider: postgresql
```

That entry does **not** mean Core already knows the Resource exists. The Build Plan carries the desired Resource request to the deployment executor. In the current Jenkins + Docker frontend, Jenkins starts PostgreSQL plus the PostgreSQL Resource Provider; the Provider waits for the database to become healthy and then registers the live Resource Instance with Core through `POST /resources`.

Example Web / Account remain unavailable for now. Selecting either stops before writing `instance.yaml` with a clear diagnostic. Leaving all example options unchecked creates the current minimal bootstrap definition from `deploy/instance.bootstrap.yaml`.

The minimal bootstrap definition is intentionally small and portable:

- no secrets
- no host-specific paths
- Core from the public `manafield` repository
- Docker Runtime Provider declared
- public `manafield-reference` Module as the first runnable example

`deploy/instance.yaml.example` remains the richer configuration reference. `deploy/instance.bootstrap.yaml` is the safe first-run default and will gain selectable example composition as those implementations become runnable.

The current v0 resolver:

- validates the Instance Definition version
- validates required IDs and source references
- rejects duplicate Instance IDs across Modules and Resources
- rejects duplicate Runtime Provider IDs
- removes disabled Runtime Providers, Modules, and Resources from the resulting plan
- normalizes Module build instructions and Resource provider requests for the CI executor

### Resource requests

Desired Resources are declared separately from Modules.

```yaml
resources:
  - id: manafield-postgres
    enabled: true
    provider: postgresql
```

The Build Plan preserves the generic `id + provider` request and emits `ci-plan/resources.tsv`.

The Resource provider is responsible for creating or observing the concrete Resource and registering live metadata with Core. Core does not infer that a Resource exists merely because it appears in the Instance Definition.

### Capability bindings

A Module Instance can explicitly bind one of its Requirement slots to a concrete Instance ID.

```yaml
modules:
  - id: manafield-account
    source:
      type: dir
    build:
      type: docker
      context: .
      dockerfile: Dockerfile
    bindings:
      state: manafield-postgres
```

The Module manifest owns the Requirement declaration:

```yaml
requires:
  capabilities:
    state:
      id: database.postgresql
      version: "^1.0.0"
```

The Instance Definition owns the concrete target selection.

The Build Plan preserves these bindings and emits `ci-plan/module-bindings.tsv`. The v0 Compose renderer passes the selected target to the Module as:

```text
MANAFIELD_BINDING_STATE_TARGET=manafield-postgres
```

This environment value identifies the bound Instance only. Endpoint, connection metadata, credentials, and secret delivery remain separate Resource/runtime concerns and are not encoded into the Capability contract.

For a PostgreSQL Resource binding, the current v0 deployment adapter uses the Resource-owned database and materializes a Module-scoped schema and role, then injects generic slot-scoped runtime values such as:

```text
MANAFIELD_BINDING_STATE_ENDPOINT_HOST
MANAFIELD_BINDING_STATE_ENDPOINT_PORT
MANAFIELD_BINDING_STATE_CONFIG_DATABASE
MANAFIELD_BINDING_STATE_CONFIG_SCHEMA
MANAFIELD_BINDING_STATE_CONFIG_USERNAME
MANAFIELD_BINDING_STATE_SECRET_PASSWORD_FILE
```

The password itself is persisted outside release artifacts under the private Instance root and mounted read-only into the Module. The PostgreSQL Resource Provider receives only an allocation manifest plus access to the private secret directory, provisions the requested schema/role idempotently inside the shared Resource database, and remains outside the application SQL data path.

### Module source types

Module sources are explicit.

Git source:

```yaml
source:
  type: git
  repository: https://github.com/example/module.git
  ref: main
```

A Git source may optionally point at a Module subdirectory inside a monorepo:

```yaml
source:
  type: git
  repository: https://github.com/example/account-suite.git
  ref: main
  subdir: modules/account-core
```

`subdir` is relative to the repository root. Absolute paths, backslashes, empty path segments, `.`, and `..` traversal are rejected by the Build Plan Resolver. The Module build context and Dockerfile remain relative to the selected Module directory, not the repository root.

Local directory source:

```yaml
source:
  type: dir
```

A `dir` source does not store an arbitrary host path in `instance.yaml`.

The current Jenkins frontend receives a separate `LOCAL_MODULES_ROOT` parameter and scans its direct child directories. A local Module is discovered when this file exists:

```text
<LOCAL_MODULES_ROOT>/<module-id>/manafield.module.json
```

The child directory name is the Module ID used by the Instance Definition.

The current Jenkins frontend copies the discovered directory into its workspace before building it, then derives a content-based `dir-...` revision for image tagging. The source directory itself is not used as the Docker build workspace.

This keeps host-local path policy in the executor environment rather than in the portable Instance Definition.

This source-resolution behavior is expected to move behind reusable `manafield` CLI/executor logic over time; Jenkins should orchestrate that logic rather than own it.

### Ingress Adapter v0

Web exposure is instance configuration, not a hard-coded Module URL.

The current v0 implementation supports host exposure:

```yaml
exposure:
  type: host
  host: reference.example.test
  targetPort: 8080
```

and a Traefik ingress adapter:

```yaml
deployment:
  ingress:
    provider: traefik
    output: /path/to/traefik/dynamic/manafield.yml
```

`manafield-ingress-traefik` consumes the normalized Build Plan and renders Traefik dynamic configuration. Jenkins publishes the generated file after Module deployment.

The Build Plan Resolver rejects duplicate host claims among enabled Modules before deployment.

Traefik requires a one-time host bootstrap so its file provider watches the configured dynamic directory. Routine Module exposure changes should not require editing the Traefik Compose definition again.

The Jenkins Pipeline performs a read-only ingress provider preflight. If the configured Traefik provider is not bootstrapped yet, the build pauses and asks for explicit approval in Jenkins before modifying the Traefik Compose deployment. After bootstrap, normal builds only render and publish dynamic ingress configuration.

The adapter boundary is intentionally provider-specific: future implementations such as `manafield-ingress-nginx` can consume the same resolved Build Plan without changing Module contracts.

## Compose deployment

The current Compose stack is [compose.yml](compose.yml).

The base Compose file starts Manafield Core. Jenkins adds enabled Modules through the generated `modules.compose.yml`.

When the `postgresql` profile is enabled the base stack additionally starts:

- Manafield PostgreSQL Resource
- PostgreSQL Resource Provider

The Provider registers the live Resource with Core after PostgreSQL is healthy.

The Docker Runtime Provider is not included yet because its binary and Runtime Protocol have not been implemented.

For a local manual build, keep the repositories as siblings:

```text
workspace/
├─ manafield/
└─ manafield-module-reference/
```

Stage the Manafield Reference descriptor for Core discovery:

```bash
mkdir -p deploy/modules/manafield-reference
cp ../manafield-module-reference/manafield.module.json \
  deploy/modules/manafield-reference/manafield.module.json
```

Then run from the Manafield repository:

```bash
docker compose \
  -f deploy/compose.yml \
  -f deploy/compose.dev.yml \
  build

docker compose \
  -f deploy/compose.yml \
  -f deploy/compose.dev.yml \
  up -d
```

`compose.yml` is the runtime deployment definition. `compose.dev.yml` only adds local source build contexts.

No committed `.env` file is required. The Compose file has local defaults.

Optional environment variables can override image names, ports, paths, and logging:

```text
MANAFIELD_CORE_IMAGE
MANAFIELD_REFERENCE_IMAGE
MANAFIELD_REFERENCE_CONTEXT
MANAFIELD_CORE_PORT
MANAFIELD_REFERENCE_PORT
MANAFIELD_MODULES_NETWORK
MANAFIELD_EDGE_NETWORK
MANAFIELD_LOG
```

The runtime Compose uses two named Docker networks:

```text
manafield-modules  internal Core ↔ Module communication
manafield-edge     external-facing edge shared with ingress infrastructure
```

The runtime Compose publishes only the Core loopback port:

```text
Core 127.0.0.1:18080
```

The development override additionally publishes:

```text
Manafield Reference 127.0.0.1:18081
```

The containers use a read-only root filesystem, drop Linux capabilities, and enable `no-new-privileges`. Core does not receive Docker socket access.

The deployment executor can inject exact image tags directly as environment variables and run Compose with `--no-build`, so a committed deployment `.env` is not required. The current Jenkins frontend performs this work; the behavior is expected to move behind reusable CLI/executor logic.

### Deploy a staged release with the CLI

A prepared release must contain `release.env`, `compose.yml`,
`modules.compose.yml`, and `compose-profiles.txt`.

```bash
manafield deploy /opt/manafield/instance/releases/123
```

This requires a working Docker CLI, Docker Compose plugin, and access to a Docker daemon.
The CLI applies the staged `COMPOSE_PROFILES`, invokes `docker compose up -d
--no-build --remove-orphans`, then `docker compose ps`. It does **not** yet
clone sources, build images, stage a release, publish ingress, or perform
post-deployment Health/Protocol verification; those remain separate Jenkins
stages during the migration.

### Build images with the CLI

From a prepared Jenkins workspace, run:

```bash
manafield build "$WORKSPACE" "$CORE_SHA" "$CORE_IMAGE"
```

This builds Core, Module and PostgreSQL Resource Provider Docker images from the
existing TSV source lists and preserves the output files `resolved-images.env`
and `resource-providers.tsv`. Jenkins still prepares source checkouts and inputs.

## Jenkins Pipeline

The repository root contains a `Jenkinsfile` implementing the first remote CI/CD frontend for Instance builds.

Jenkins is not required by the Manafield model. The current migration direction is to reduce Pipeline stages to `manafield` CLI calls plus Jenkins-specific concerns such as triggers, credentials, approvals, workspace management, and build history. The Resolve Build Plan stage already uses `manafield plan`.

Jenkins parameters:

- `INSTANCE_ROOT` — private Instance root containing `instance.yaml`
- `LOCAL_MODULES_ROOT` — optional root scanned for `source.type: dir` Modules

When Jenkins itself runs in a container, keep stable paths inside the Jenkins container and map whatever host paths the installation uses into them.

Recommended Jenkins container paths:

```text
/opt/manafield/instance
/opt/manafield/local-modules
```

Example Compose bind mounts:

```yaml
services:
  jenkins:
    volumes:
      - ${MANAFIELD_INSTANCE_HOST_PATH:-/srv/manafield/instance}:/opt/manafield/instance
      - ${MANAFIELD_LOCAL_MODULES_HOST_PATH:-/srv/manafield/local-modules}:/opt/manafield/local-modules:ro
```

`INSTANCE_ROOT` defaults to `/opt/manafield/instance` and must be writable because Jenkins creates release directories under it. `LOCAL_MODULES_ROOT` defaults to `/opt/manafield/local-modules` and may be read-only because local Module sources are copied into the Jenkins workspace before Docker build.

If an installation uses different in-container paths, use **Build with Parameters** to override them. See `Jenkinsfile.example` for the path-related customization points.

The pipeline:

1. checks out Manafield Core
2. resolves the private `instance.yaml` into `build-plan.json`
3. discovers requested local directory Modules under `LOCAL_MODULES_ROOT`
4. runs Core verification in the Docker `verify` target
5. snapshots enabled Git/local Module sources into the workspace
6. builds Core / Module images and any selected Resource Provider images
7. creates an immutable-ish release directory
8. stages Module descriptors for Core discovery
9. enables required Compose profiles for desired Resources
10. invokes `manafield deploy RELEASE_DIR` for staged Docker Compose deployment
11. verifies Core health, Module Registry state, Resource Registry state, and Manafield Reference health

A Jenkins deployment directory is expected to look like:

```text
<INSTANCE_ROOT>/
├─ instance.yaml
├─ releases/
│  ├─ 1/
│  │  ├─ compose.yml
│  │  ├─ release.env
│  │  ├─ build-plan.json
│  │  ├─ resolved-images.env
│  │  └─ modules/
│  └─ ...
└─ current -> releases/<successful build>
```

`release.env` contains resolved image names and deployment paths, not application secrets.

The current bootstrap pipeline requires the `manafield-reference` Module because the Docker Runtime Provider has not been implemented yet.

## Module Build Contract

For the initial container-based implementation, a Module repository owns its build through a root-level Dockerfile (or an explicitly configured Dockerfile path).

Jenkins does not need to know whether the Module itself is implemented in Node.js, Python, Rust, Java, or another language.

## Secrets

Do not store secrets in the Instance Definition.

Credentials, registry tokens, runtime keys, and similar values must be injected by the CI credential store or the deployment environment.

## Status

Pre-alpha. Jenkins deploys the current multi-Module Instance and publishes Traefik ingress plus a validated Manage Binding snapshot. The new CLI Release-ID path can prepare/deploy private Git/dir Modules and one PostgreSQL Resource directly, but not the full Jenkins ingress/multi-Resource scope. Full direct Instance health/recovery still requires integration verification.

On the Jenkins production path, Stage Release provisions separate read-only Account login-audit and Role-check credentials for Manage, and configures Account Core to accept forwarding headers **only** from the configured trusted Traefik peer. Manage never receives the full Account/Role management credentials.

Do not treat a Jenkins release directory as the immutable YAML snapshot of CLI `deploy <release-id>`.
