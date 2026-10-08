# Manafield Instance Definition

This directory contains deployment-composition examples for a Manafield instance.

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

Secrets still do not belong in a private `instance.yaml`; inject them through CI/runtime secret mechanisms instead.

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

The **Build Plan Resolver** consumes this file and produces a CI-executor-neutral JSON plan.

Run it with:

```bash
cargo run --features build-plan --bin manafield-build-plan -- \
  deploy/instance.yaml.example \
  build-plan.json
```

Without the second path, the resolver prints the plan to stdout.

See [build-plan.example.json](build-plan.example.json) for the current normalized output.

### First-run bootstrap

If Jenkins can see `INSTANCE_ROOT` but `<INSTANCE_ROOT>/instance.yaml` does not exist yet, the Pipeline enters a one-time Bootstrap Wizard before resolving the Build Plan.

The Wizard is intentionally shown **only when the Instance Definition does not exist**. Existing instances continue directly to normal builds.

The bootstrap choices are:

- Manafield PostgreSQL Resource — implemented
- Example Web Module — planned
- Example Account Module — planned
- initial administrator username / password, defaulting to `admin / admin`

Selecting Example Account implies both PostgreSQL and Example Web.

Selecting only PostgreSQL appends a desired Resource request to the generated `instance.yaml`:

```yaml
resources:
  - id: manafield-postgres
    enabled: true
    provider: postgresql
```

That entry does **not** mean Core already knows the Resource exists. The Build Plan carries the desired Resource request to Jenkins; Jenkins starts PostgreSQL plus the PostgreSQL Resource Provider; the Provider waits for the database to become healthy and then registers the live Resource Instance with Core through `POST /resources`.

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

The Jenkins executor receives a separate `LOCAL_MODULES_ROOT` parameter and scans its direct child directories. A local Module is discovered when this file exists:

```text
<LOCAL_MODULES_ROOT>/<module-id>/manafield.module.json
```

The child directory name is the Module ID used by the Instance Definition.

Jenkins copies the discovered directory into its workspace before building it, then derives a content-based `dir-...` revision for image tagging. The source directory itself is not used as the Docker build workspace.

This keeps host-local path policy in the CI/runtime environment rather than in the portable Instance Definition.

Jenkins will execute the generated Build Plan instead of owning Manafield's deployment model.

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

A later Jenkins Pipeline can inject exact image tags directly as environment variables and run Compose with `--no-build`, so a committed deployment `.env` is not required.

## Jenkins Pipeline

The repository root contains a `Jenkinsfile` implementing the first Instance Build Plan pipeline.

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
10. deploys the release with Docker Compose
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

Draft / pre-alpha.
