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

## Draft v0

See [instance.example.yaml](instance.example.yaml).

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
  - id: reference-web
    enabled: true
    source:
      repository: ...
      ref: main
    build:
      type: docker
      context: .
      dockerfile: Dockerfile
```

The **Build Plan Resolver** consumes this file and produces a CI-executor-neutral JSON plan.

Run it with:

```bash
cargo run --features build-plan --bin manafield-build-plan -- \
  deploy/instance.example.yaml \
  build-plan.json
```

Without the second path, the resolver prints the plan to stdout.

See [build-plan.example.json](build-plan.example.json) for the current normalized output.

The current v0 resolver:

- validates the Instance Definition version
- validates required IDs and source references
- rejects duplicate Module / Runtime Provider IDs
- removes disabled Runtime Providers and Modules from the resulting plan
- normalizes Module build instructions for the CI executor

Jenkins will execute the generated Build Plan instead of owning Manafield's deployment model.

## Compose deployment

The current Compose stack is [compose.yml](compose.yml).

It starts:

- Manafield Core
- Manafield Reference Module

The Docker Runtime Provider is not included yet because its binary and Runtime Protocol have not been implemented.

For a local manual build, keep the repositories as siblings:

```text
workspace/
├─ manafield/
└─ manafield-module-reference/
```

Then run from the Manafield repository:

```bash
docker compose -f deploy/compose.yml build
docker compose -f deploy/compose.yml up -d
```

No `.env` file is required. The Compose file has local defaults.

Optional environment variables can override image names, ports, paths, and logging:

```text
MANAFIELD_CORE_IMAGE
MANAFIELD_REFERENCE_IMAGE
MANAFIELD_REFERENCE_CONTEXT
MANAFIELD_CORE_PORT
MANAFIELD_REFERENCE_PORT
MANAFIELD_NETWORK
MANAFIELD_LOG
```

The default host bindings are loopback-only:

```text
Core          127.0.0.1:18080
Reference Web 127.0.0.1:18081
```

The containers use a read-only root filesystem, drop Linux capabilities, and enable `no-new-privileges`. Core does not receive Docker socket access.

A later Jenkins Pipeline can inject exact image tags directly as environment variables and run Compose with `--no-build`, so a committed deployment `.env` is not required.

## Module Build Contract

For the initial container-based implementation, a Module repository owns its build through a root-level Dockerfile (or an explicitly configured Dockerfile path).

Jenkins does not need to know whether the Module itself is implemented in Node.js, Python, Rust, Java, or another language.

## Secrets

Do not store secrets in the Instance Definition.

Credentials, registry tokens, runtime keys, and similar values must be injected by the CI credential store or the deployment environment.

## Status

Draft / pre-alpha.
