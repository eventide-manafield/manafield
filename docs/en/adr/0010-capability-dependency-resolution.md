# ADR-0010 — Capability-Based Dependency Resolution

- Status: **Accepted**
- Date: 2026-10-07

## Context

Manafield Modules may use different implementation languages and deployment forms, and multiple implementations may provide the same functionality.

If dependencies are pinned directly to concrete Module IDs:

- a compatible fork cannot satisfy the dependency merely because it has a different ID
- replacing an official implementation becomes difficult
- Module identity is forced to also act as the functional contract
- different instances cannot easily select different implementations

Using free-form `tags` for dependency resolution is too weak in the opposite direction.

Two Modules may both have an `auth` tag while one only provides a login UI and another only validates sessions. A tag does not prove wire-level or semantic compatibility.

Infrastructure resources such as databases, caches, and storage also have a different role from ordinary user-facing Modules. Those resources should be prepared and bound by Providers.

## Decision

### 1. Separate Module identity from dependency contracts

A Module `id` identifies a **concrete implementation**.

General Module dependencies should target a **Capability Contract**, not a concrete Module ID.

Conceptual example:

```yaml
id: manafield-echo

requires:
  capabilities:
    identity:
      id: manafield.identity
      version: 1
```

An identity implementation may provide that Capability:

```yaml
id: manafield-account

provides:
  capabilities:
    - id: manafield.identity
      version: 1
```

A fork or alternate implementation may provide the same Capability when it satisfies the same contract.

```yaml
id: better-account

provides:
  capabilities:
    - id: manafield.identity
      version: 1
```

Echo depends on the `manafield.identity v1` contract rather than the implementation name.

### 2. Capabilities are stronger contracts than tags

`tags` are descriptive metadata for search, classification, UI filtering, and humans.

Tags are not used for dependency resolution.

```text
Tag
→ descriptive / non-binding metadata

Capability
→ dependency / compatibility contract
```

A Capability has at least a stable ID and version.

Searchable/listable entries such as Modules, Operations, Capabilities, and Resources may carry an optional human-readable `description`. Descriptions are display metadata for humans; **Descriptions do not participate in contract resolution or compatibility checks**.

Over time, a Capability Contract should be able to refer to one or more Operations plus semantic compatibility rules.

Example:

```text
manafield.identity v1
├─ identity.current-user
├─ identity.validate-session
└─ semantic compatibility rules
```

The exact Descriptor schema and version-constraint syntax are deferred.

### 3. Distinguish Operations from Capabilities

An Operation is **one callable functionality contract**.

A Capability is a **higher-level compatibility contract that interchangeable implementations can satisfy**.

```text
Capability
└─ one or more Operations + semantic contract
```

Not every Operation must belong to a Capability.

Framework routes and UI-internal endpoints are not automatically promoted to Operations or Capabilities.

### 4. The Instance selects concrete bindings

Requirements and concrete implementation selection are separate.

```text
Module requirement
        ↓
Capability candidates in Registry
        ↓
Instance binding
        ↓
Concrete Module
```

When multiple candidates exist, Core does not silently choose one.

The Instance Definition or a management UI selects the concrete binding.

Automatic binding when exactly one candidate exists is deferred.

For rare cases that truly depend on implementation-specific behavior, an explicit exact-implementation dependency may exist as an escape hatch, but it is not the default model.

### 5. Express infrastructure resource dependencies separately

Databases, caches, object storage, filesystems, and similar resources are distinct from Module Capabilities.

Conceptual example:

```yaml
requires:
  resources:
    state:
      id: database.postgresql
      version: 1
```

A **Provider**, rather than an ordinary Module, satisfies such a requirement.

```text
Echo
└─ requires resource: database.postgresql
                         ↓
                 PostgreSQL Provider
```

A Provider is not a query proxy.

A PostgreSQL Provider may be responsible for provisioning and allocation such as:

- creating a database or schema
- creating Module-scoped accounts and permissions
- producing connection metadata
- preparing secret references

The application still talks directly to PostgreSQL through its native client.

```text
Java Module   → JDBC / PostgreSQL driver ─┐
Node Module   → pg                       ├→ PostgreSQL
Python Module → psycopg                  ┘
```

Core does not embed JDBC or database-specific application clients.

### 6. Providers are not ordinary Modules

Providers are system-side components used to connect Manafield to execution environments and infrastructure resources.

In addition to the already-established Runtime Provider boundary, future Resource Providers may exist.

Example:

```text
Provider
├─ Runtime Provider
│  └─ Docker
├─ Resource Provider
│  └─ PostgreSQL
└─ Ingress Provider / Adapter
   └─ Traefik
```

Do not prematurely force these into one universal Provider Protocol.

Different Provider families may have different privilege and lifecycle requirements and can receive separate protocols.

## Consequences

### Benefits

- compatible forks and alternate implementations can satisfy existing dependencies
- the ecosystem is not unnecessarily coupled to official implementation IDs
- each Instance can choose concrete implementations
- functional dependencies and infrastructure resource dependencies are clearly separated
- shared PostgreSQL infrastructure can later allocate isolated databases/schemas to multiple Modules
- language independence remains centered on protocol contracts

### Costs

- Capability ID and version policies are required
- Capability-to-Operation validation is required
- binding and ambiguity policy is required when multiple candidates exist
- Resource Provider protocols and secret injection need additional design
- dependency graphs must be validated before Module installation

## Non-goals

This ADR does not finalize:

- the final Capability Descriptor JSON/YAML schema
- version range / negotiation syntax
- automatic provider selection
- the PostgreSQL Provider Protocol
- database credential secret format
- Module package format
- Permission / trust model
- dependency-cycle policy

These will be defined by follow-up design documents or ADRs as implementation approaches.
