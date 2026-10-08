# ADR-0004 — Runtime Provider Boundary

- Status: Accepted
- Date: 2026-10-06

## Context

Modules may run through Docker, local processes, remote runtimes, Kubernetes, or other mechanisms. Embedding Docker-specific behavior directly into Core would make Docker a practical Core dependency.

## Decision

Execution technology is separated into a **Runtime Provider** system layer.

A Runtime Provider is not an ordinary Module. It is a privileged system component capable of creating, starting, stopping, and removing Module runtimes.

Core must remain useful without any specific Runtime Provider for Registry, Operation discovery, validation, and similar base functions.

This does not require every deployment technology to receive equal v0 support. The supported full v0 Instance build/deployment baseline is Docker; see [ADR-0013](0013-docker-v0-cli-execution.md).

## Reason

- separates Core from execution technology
- allows Core to run without Docker
- leaves room for Process / Remote / Kubernetes Providers
- creates a boundary for privileged capabilities

## Consequences

- Core and Runtime Providers require a Runtime Protocol
- Modules should prefer declaring required runtime capabilities over product-specific names where practical
- without a Runtime Provider, automatic execution is unavailable but Core still functions
