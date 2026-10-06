# ADR-0005 — Docker Runtime Provider Process Isolation

- Status: Accepted
- Date: 2026-10-06

## Context

Docker daemon access is highly privileged. If Core directly accesses `docker.sock`, a Core compromise may expand into control of the host Docker daemon.

Fully separating the Docker Provider into another repository and product would add unnecessary development and release complexity at this stage.

## Decision

The Docker Runtime Provider is **maintained in the same Manafield repository but built and executed as a separate binary / process**.

Expected release shape:

```text
same source / release
├─ manafield
└─ manafield-runtime-docker
```

For Docker deployment:

- Core does not receive `docker.sock`
- only the Docker Runtime Provider receives `docker.sock`
- Core and Provider communicate through a constrained Runtime Protocol
- Unix Domain Socket is the preferred initial local transport candidate

The Provider API must describe Manafield lifecycle operations rather than arbitrary Docker command execution.

## Reason

- reduces the blast radius of a Core compromise
- keeps development, release, and versioning in one repository
- avoids an expensive future process-extraction refactor
- provides a place for Docker-specific security policy

## Consequences

The Docker Provider should enforce policies such as:

- deny privileged containers by default
- restrict arbitrary host path mounts
- prevent Docker socket re-exposure
- constrain allowed networks and resources
- validate Runtime requests

Sharing a repository does not imply sharing a process. Security boundaries are created through process and privilege separation.
