# ADR-0001 — Rust Headless Core

- Status: Accepted
- Date: 2026-10-06

## Context

Manafield Core owns strict platform boundaries such as Registry, Protocol, Validation, and Lifecycle. It should not depend on a Web UI or a specific Runtime.

## Decision

The reference Manafield Core implementation is written in **Rust** and remains **headless**.

Core must run without a Web UI. Web, CLI, Mobile, and other interfaces are separate clients of the Core API.

## Reason

- strict type and state modeling
- explicit error handling
- low runtime overhead
- single-binary deployment
- independent UI and Core lifecycles

## Consequences

- the Rust toolchain is pinned by the project
- Core APIs must stabilize independently of UI work
- UI-specific state should not leak into Core
