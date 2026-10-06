# ADR-0003 — Registry Snapshot / ArcSwap Read Model

- Status: Accepted
- Date: 2026-10-06

## Context

Registry reads are expected to greatly outnumber writes. Long-lived read locks and asynchronously updated caches can create unnecessary contention or stale reads.

## Decision

Split Registry state into:

- `ModuleRegistry`: mutable source of truth
- `RegistrySnapshot`: immutable read model

Writes use `RwLock<ModuleRegistry>`. After each successful mutation, Core builds a new snapshot and atomically publishes it through `ArcSwap`.

Readers access the current snapshot without locking the mutable Registry.

## Reason

- removes Registry lock from the read path
- readers always see one complete point-in-time state
- old and new snapshots can coexist safely
- snapshot consistency does not depend on asynchronous events

## Consequences

- snapshots are immutable and replaced rather than modified
- previous snapshots are released automatically when their last `Arc` reference disappears
- Registry events may serve audit, metrics, or notifications but are not required for snapshot consistency
