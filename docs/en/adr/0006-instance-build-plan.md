# ADR-0006 — Instance Build Plan / Single Pipeline

- Status: Accepted
- Date: 2026-10-06

## Context

A Manafield instance may deploy Core, Runtime Providers, and many Modules together.

Creating one Jenkins Job for every Module makes the Jenkins surface grow with the Module count and makes it difficult to treat the whole instance as one deployment unit.

Hard-coding Module lists and Manafield-specific architecture directly into Jenkinsfiles would instead make Jenkins itself the source of truth and make migration to another CI/CD executor unnecessarily expensive.

## Decision

Manafield uses **one Pipeline per Manafield instance** as the default deployment model.

```text
Manafield Instance Definition
        ↓
Build Plan Resolver
        ↓
Single CI Pipeline
        ↓
Core / Runtime Providers / Modules
        ↓
Deploy
        ↓
Health Verification
```

Jenkins is the first CI executor, but the Manafield deployment model itself is not Jenkins-specific.

The Instance Definition declares which Core, Runtime Providers, and Modules belong to the instance. A resolver turns that definition into a Build Plan, and the Jenkins Pipeline executes the plan.

Adding Modules does not create additional Jenkins Jobs.

## Module Build Contract

A Module repository should not require Jenkins to understand its implementation language or internal build tools.

The initial minimum build contract for container-based Modules is a root-level `Dockerfile`.

```text
Python Module ─┐
Node Module   ─┼─ own Dockerfile → common build pipeline
Rust Module   ─┘
```

This contract may later expand through Module package metadata.

## Trigger Model

Webhooks from multiple repositories may trigger the same Manafield Instance Pipeline.

The Pipeline may identify the changed source and selectively Verify / Build / Deploy only the affected component when possible.

Example:

```text
reference-web push
        ↓
Manafield Instance Pipeline
        ↓
reference-web only
        ↓
Deploy
        ↓
Health check
```

Broader changes such as Core or shared Protocol changes may trigger wider verification and rebuilding.

## Reason

- decouples Jenkins Job count from Module count
- models a Manafield instance as one deployment unit
- separates Module implementation languages from the CI Pipeline
- keeps Instance Definition / Build Plan portable to other CI/CD executors
- allows incremental build / deployment
- provides one place to observe deployment state for the whole instance

## Consequences

- per-repository Jenkinsfiles are not a default requirement
- Module repositories must provide a standard build contract
- Jenkins contains one Pipeline per Manafield instance
- Manafield needs an Instance Definition schema and Build Plan resolver
- secrets are injected from CI credential storage rather than committed to repositories or Instance Definitions
- deployment success includes Health / Protocol verification, not merely process or container startup
