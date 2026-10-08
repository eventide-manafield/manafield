# ADR-0013 — Docker-required v0 Deployment / CLI-first Execution

- Status: Accepted
- Date: 2026-10-08

## Context

Manafield separates the general Core registry/protocol model from concrete execution technology through the Runtime Provider boundary.

Supporting Process, Remote, Kubernetes, and Docker equally in v0 would require Manafield to reimplement too much lifecycle, networking, filesystem, secret, restart, and isolation behavior.

At the same time, allowing Manafield-specific build/deploy semantics to accumulate in Jenkinsfile would make Jenkins a practical mandatory runtime.

## Decision

A supported **full Manafield v0 Instance build / deployment requires Docker**.

This does not embed Docker-specific concepts into the Core model.

```text
Manafield model
  Instance / Module / Resource / Capability / Binding
          ↓
manafield CLI
          ↓
Docker-based v0 executor
```

The Core binary may still run registry, validation, API, and other independent functions without Docker. A complete v0 Instance build, materialization, and deployment targets an environment with Docker installed.

The Manafield CLI is the canonical entrypoint for Instance execution.

```text
manafield plan
manafield build
manafield deploy
manafield verify
manafield rebuild
```

These commands are implemented incrementally. Existing Manafield-specific build/deployment semantics in Jenkinsfile move into the CLI or reusable executors gradually.

Jenkins is **not required; it is the first official remote CI/CD frontend**.

```text
Local terminal ─┐
                ├─> manafield CLI ─> Docker
Jenkins ────────┘
```

Jenkins may own CI concerns such as triggers, credential integration, approvals, build history, and remote UI, while avoiding ownership of Manafield-specific build/deploy rules where practical.

## Consequences

- Docker is part of the full v0 Instance build/deploy requirements.
- Core development, validation, and some headless functions may still run without Docker.
- the initial Module build contract remains Dockerfile-oriented.
- Jenkins Pipeline stages are gradually reduced to `manafield` CLI calls.
- CI credential stores, approvals, and webhooks remain outside the executor-neutral Manafield model.
- Process, Remote, and Kubernetes Runtime Providers remain future extension points rather than v0 compatibility requirements.

## Non-goals

This does not mean mounting `docker.sock` directly into Core, putting Docker container IDs into Capability / Binding contracts, requiring Jenkins to use Manafield, or committing every future Manafield version to Docker.

Docker is an **intentional v0 deployment dependency**. Jenkins is an **optional orchestration frontend**.
