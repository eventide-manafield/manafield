# Manafield Architecture

> This document describes the current architecture direction.  
> Manafield is still in the pre-alpha / design stage, so details may change.

## 1. Project Definition

Manafield is a **modular personal platform**.

It is not a single website and it is not tied to a single UI.

The project aims to provide a small control plane capable of connecting and managing:

- modules
- independent services
- bots
- record systems
- external integrations
- optional Web interfaces

`manafield.studio` is an instance built on top of Manafield, not Manafield itself.

## 2. Core Philosophy

### Strict Core, Free Modules

The Core owns the parts that should remain consistent and predictable:

- Module Registry
- Manifest validation
- Module lifecycle
- Runtime abstraction
- Authentication and permissions
- Settings
- Health checks
- Service discovery
- Routing metadata
- Events and messaging

Modules are intentionally less constrained.

A Module may be implemented using Python, Go, Node.js, Java, Rust, or another environment capable of implementing the protocol.

## 3. Headless Core

Manafield Core is designed to be headless.

That means the Core does not require a built-in Web UI to function.

```text
Manafield Core
├─ Official Web View
├─ CLI
├─ Mobile Client
└─ Other Clients
```

An official Web View may be provided separately, but the Core and Modules should remain usable without it.

## 4. High-level Architecture

```text
                    ┌─────────────────┐
                    │  Manafield Web  │
                    │    optional     │
                    └────────┬────────┘
                             │ Core API
                             ▼
┌────────────────────────────────────────────┐
│              Manafield Core               │
│                   Rust                    │
│                                            │
│ Registry       Lifecycle      Settings    │
│ Permission     Health         Discovery   │
│ Runtime        Routing        Events      │
└───────┬───────────────┬───────────────┬───┘
        │               │               │
        │ Protocol      │ Protocol      │ Protocol
        ▼               ▼               ▼
     Module          Service        Integration
```

## 5. Component Types

### Module

Provides functionality directly to a Manafield instance.

Examples:

- Observe
- Records
- Dashboard

### Service

Represents an independently running service managed or connected by Manafield.

Examples:

- Echo
- Misskey

### Integration

Connects Manafield to an external system or platform.

Examples:

- ProtoDuck
- Discord
- external APIs

The categories describe execution characteristics and responsibility. They should share as much of the common protocol as practical.

## 6. Runtime Model

Manafield does **not** implement a container runtime.

Instead, Core talks to runtime implementations through an adapter boundary.

Initial target:

```text
Manafield Core
      │
      │ Runtime Adapter
      ▼
    Docker
```

Possible future target:

```text
Manafield Core
      │
      │ Runtime Adapter
      ▼
 Kubernetes
```

Conceptually:

```rust
trait ModuleRuntime {
    async fn install(&self, module: &ModuleSpec) -> Result<()>;
    async fn start(&self, id: &ModuleId) -> Result<()>;
    async fn stop(&self, id: &ModuleId) -> Result<()>;
    async fn status(&self, id: &ModuleId) -> Result<ModuleStatus>;
    async fn remove(&self, id: &ModuleId) -> Result<()>;
}
```

The exact Rust API is not finalized.

## 7. Web Contributions

A Module does not need to provide a UI.

Initial Web contribution concepts:

### none

No Web UI.

### proxy

The Module runs its own Web application and Manafield exposes or routes it through an instance.

### external

The Module points to an external URL.

Future versions may experiment with more tightly integrated remote UI mechanisms, but they are intentionally outside the initial scope.

## 8. Isolation

The default architecture avoids loading arbitrary Module code directly into the Core process.

```text
Manafield Core
      │
      │ API
      ▼
Module Process / Container
```

This makes language independence easier and provides a clearer security and failure boundary.

Third-party isolation may later include restricted containers or sandboxing.

## 9. Non-goals

Manafield does not aim to reimplement:

- Docker
- container isolation
- a database engine
- a reverse proxy from scratch
- Kubernetes

Manafield should orchestrate and compose existing technologies rather than replace them.
