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

A future **Build Plan Resolver** will consume this file and produce a CI-executor-neutral plan.

Jenkins will execute that plan instead of owning Manafield's deployment model.

## Module Build Contract

For the initial container-based implementation, a Module repository owns its build through a root-level Dockerfile (or an explicitly configured Dockerfile path).

Jenkins does not need to know whether the Module itself is implemented in Node.js, Python, Rust, Java, or another language.

## Secrets

Do not store secrets in the Instance Definition.

Credentials, registry tokens, runtime keys, and similar values must be injected by the CI credential store or the deployment environment.

## Status

Draft / pre-alpha.
