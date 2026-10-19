# ADR-0006 — Instance Build Plan / Single Pipeline

- Status: Accepted
- Date: 2026-10-06

> Follow-up: [ADR-0014](0014-instance-working-release.md) distinguishes **Manafield platform builds** from **independently published Module installation and Instance Release/Deploy**. Today's Jenkins Module source builds are transitional and not the final `build all` contract.

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

Jenkins is the first remote CI/CD frontend, but the Manafield deployment model itself is not Jenkins-specific.

The Instance Definition declares which Core, Runtime Providers, and Modules belong to the instance. The `manafield` CLI resolves that definition into a Build Plan and evolves into the canonical entrypoint for v0 Docker-backed build/deploy execution.

The Jenkins Pipeline should call that CLI where practical and focus on CI concerns such as webhooks, approvals, credential integration, and build history. Adding Modules does not create additional Jenkins Jobs.

CLI-first execution and the Docker v0 baseline follow [ADR-0013](0013-docker-v0-cli-execution.md).

## Module Source / Build Contract

Module sources use two explicit forms.

```text
git
→ repository + ref

dir
→ discovered by Module ID under a local Module root allowed by the CI executor
```

A `dir` source does not store an arbitrary host path in `instance.yaml`. Jenkins receives a separate `LOCAL_MODULES_ROOT` value and scans its direct child directories.

```text
<LOCAL_MODULES_ROOT>/<module-id>/manafield.module.json
```

Jenkins copies the discovered local source into its workspace before building it instead of using the host source directory directly as the build workspace.

A Module should not require Jenkins to understand its implementation language or internal build tools.

The initial minimum build contract for container-based Modules is a `Dockerfile` at the Module source root.

```text
Python Module ─┐
Node Module   ─┼─ own Dockerfile → common build pipeline
Rust Module   ─┘
Local Module  ─┘
```

This contract may later expand through Module package metadata.

## Trigger Model

Webhooks from multiple repositories may trigger the same Manafield Instance Pipeline.

The Pipeline may identify the changed source and selectively Verify / Build / Deploy only the affected component when possible.

Example:

```text
manafield-reference push
        ↓
Manafield Instance Pipeline
        ↓
manafield-reference only
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
- keeps the same Instance execution semantics usable through CLI + Docker without Jenkins
- allows incremental build / deployment
- provides one place to observe deployment state for the whole instance

## Consequences

- per-repository Jenkinsfiles are not a default requirement
- Module repositories must provide a standard build contract
- Jenkins contains one Pipeline per Manafield instance
- Manafield needs an Instance Definition schema and Build Plan resolver
- secrets are injected through executor-specific credential integration rather than committed to repositories or Instance Definitions; Jenkins Credentials Store may be the first adapter
- deployment success includes Health / Protocol verification, not merely process or container startup
## Exclusive Resource consumer contract (2026-10-19)

A Resource may declare `exclusiveTo: { module: <module-id>, slot: <requirement-slot> }`.
Enabled Redis Resources require it. Validation rejects any other Module/slot
(including disabled declarations), Core references, a missing/disabled owner,
or an owner slot that does not bind that Resource. The resolved Build Plan
preserves the contract and CI emits `resource-exclusive-consumers.tsv`.
Direct staging validates the definition before allocating secrets or building,
then rechecks Redis ownership when injecting credentials. Jenkins rejects
exclusive Resources until its materializer supports that enforcement.
Existing PostgreSQL/S3 Resources without this field keep their current behavior.
This is credential allocation policy; prefixes are not Redis ACLs.
The current Redis executor limit remains one Resource per Manafield Instance.

## Build execution independence (user decision, 2026-10-19)

Jenkins is the recommended automated build/deploy runner. Manafield must also
build and prepare/deploy a Release through its shared CLI without a Jenkins
installation, Jenkins workspace variables or Jenkins API. Jenkins orchestrates
the shared engine; it does not own the platform's build semantics.

- Instance Definition and executor-neutral Build Plan are common inputs.
  Identical source revisions, images and plans must describe equivalent build
  targets and Release configuration; this does not promise bitwise reproducibility.
- Separate build, Release preparation, apply and verify. CI scheduling, approvals,
  logs and credential delivery are optional runner responsibilities.
- Resource interpretation, exclusive-consumer checks, bindings and secret-file
  mounts belong to shared code, without a second ruleset in Jenkinsfile.
  Consumers receive authorized file references rather than secret values.
- Direct execution uses the same explicit Instance Root/source/credential inputs
  and validation. Absence of Jenkins does not bypass validation.
- Docker is the current build/runtime backend; it is independent of the Jenkins
  runner choice. Other backends remain future work.
- Building/preparing artifacts does not automatically deploy or publish ingress.
  Deployment targets and existing-data lifecycle must be explicit.

Current status: shared CLI image builders and Compose application already exist.
Resource materialization/bindings/Release generation still duplicate behavior
inside Jenkinsfile. Next, consolidate those responsibilities into shared CLI
code and reuse it from Jenkins/direct execution while adding S3/Redis support.
This section defines the target contract, not completed Jenkins support or a
completed native Noctemblith rollout.
