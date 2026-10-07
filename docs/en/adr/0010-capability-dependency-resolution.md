# ADR-0010 — Capability-Based Requirement Resolution

- Status: **Accepted**
- Date: 2026-10-07

## Context

Manafield components may require functionality from other Modules as well as infrastructure such as databases, caches, and storage.

Modeling each need with a separate dependency grammar would continually expand the model:

```text
module dependency
resource dependency
database dependency
storage dependency
...
```

From the consumer's perspective, the common question is simpler:

> **“Which contracts are required for this component to run?”**

Pinning dependencies to concrete Module IDs also prevents compatible forks or alternate implementations from satisfying the same requirement.

Free-form tags are too weak in the opposite direction because they do not establish compatibility.

## Decision

### 1. Express all ordinary requirements as Capability Contracts

Manafield unifies ordinary dependencies and requirements under **Capability Contracts**.

Module functionality and infrastructure resources do not use separate `requires` namespaces.

Example:

```yaml
id: manafield-echo

requires:
  capabilities:
    identity:
      id: manafield.identity
      version: "^1.0.0"

    state:
      id: database.postgresql
      version: "^1.0.0"
```

From Echo's perspective both entries are required contracts.

The difference is **what provides the Capability and how the concrete binding is delivered**.

```text
manafield.identity
→ may be provided by a Module

database.postgresql
→ may be provided by a Provider-managed Resource
```

### 2. Separate Module identity from Capability contracts

A Module `id` identifies a concrete implementation.

Ordinary requirements target Capabilities rather than concrete Module IDs.

Example:

```yaml
id: manafield-account

provides:
  capabilities:
    - id: manafield.identity
      version: "1.2.0"
      description: User identity and session contract
```

A fork or alternate implementation may provide the same Capability when it completely satisfies the same contract.

Concrete implementation dependencies may exist as an explicit escape hatch for implementation-specific behavior, but they are not the default dependency model.

### 3. Capabilities are stronger versioned contracts than tags

`tags` are non-binding metadata for search, classification, and UI filtering.

Tags do not satisfy dependencies.

```text
Tag
→ descriptive / non-binding metadata

Capability
→ versioned dependency / compatibility contract
```

A Capability has at least:

- a stable `id`
- a contract version
- an optional `description`
- compatibility semantics defined by the contract

Description is human-readable display/search metadata and does not participate in compatibility checks.

### 4. Capability versions are SemVer contract versions

Capability versions are independent from Module release versions.

Example:

```text
Module:
  better-account v7.3.1

Provides:
  manafield.identity v2.1.0
```

The Module version identifies an implementation release. The Capability version identifies **which contract the implementation is compatible with**.

Capability Contract versions use SemVer.

```text
MAJOR
→ contract change that breaks backward compatibility

MINOR
→ backward-compatible contract expansion

PATCH
→ correction that preserves contract meaning
```

Providers declare the **exact Capability version** they implement.

```yaml
provides:
  capabilities:
    - id: manafield.identity
      version: "2.3.1"
```

Consumers declare an accepted **SemVer range**.

```yaml
requires:
  capabilities:
    identity:
      id: manafield.identity
      version: "^2.1.0"
```

The initial constraint syntax follows common SemVer-range conventions; conceptually `^2.1.0` means `>=2.1.0 <3.0.0`.

The exact parser/library is selected during implementation.

### 5. Do not perform feature-subset negotiation

The Resolver does not infer that a provider is acceptable because it happens to implement only the subset of Operations used by one consumer.

If Capability v3 adds an Operation and a fork based on v2 cherry-picks only that Operation, Manafield does not automatically classify that fork as v3-compatible.

The implementation author declares which Capability versions are **fully implemented**.

```text
Capability compatibility
→ whole-contract granularity

Operation-subset inference
→ not performed
```

A feature may become a separate Capability when it has independent contractual value, but Manafield does not split Capabilities speculatively.

### 6. Distinguish Operations from Capabilities

An Operation is one callable contract.

A Capability is a higher-level compatibility contract that may compose one or more Operations plus semantic rules.

```text
Capability
└─ one or more Operations + semantic contract
```

Not every Operation belongs to a Capability.

Framework routes and Web-UI-internal endpoints are not automatically promoted to Operations or Capabilities.

Core may validate mechanically checkable parts such as required Operation presence, but it does not infer full semantic compatibility.

### 7. Capability providers may be Modules or Resources

The matching model is unified while provider semantics may differ.

#### Module-provided Capability

Example:

```text
manafield.identity ^1
        ↓
Account Module
```

A Module provides the Capability through Operations.

#### Resource-provided Capability

Example:

```text
database.postgresql ^1
        ↓
main-postgres Resource
        ↓ managed / registered by
PostgreSQL Provider
```

A Resource is a concrete resource available to an Instance.

Examples:

```text
main-postgres
dev-postgres
remote-postgres
```

Each Resource may advertise the Capability IDs and exact contract versions it satisfies.

A Provider may discover, provision, allocate, and prepare connection metadata for Resources.

### 8. The Instance selects concrete Capability bindings

Requirements are separate from concrete provider selection.

Example:

```text
Echo.state
requires database.postgresql ^1
        ↓
compatible candidates
├─ main-postgres
└─ dev-postgres
        ↓
Instance binding
        ↓
main-postgres
```

Conceptual Instance configuration:

```yaml
bindings:
  capabilities:
    echo.state:
      resource: main-postgres
```

Module-provided Capabilities follow the same principle:

```yaml
bindings:
  capabilities:
    echo.identity:
      module: better-account
```

The final binding schema is deferred.

Core does not silently pick between multiple candidates.

Automatic binding when only one candidate exists is also deferred.

### 9. A Resource Provider is not an application-data proxy

Providing a Resource that satisfies `database.postgresql` does not mean Core or the Provider proxies SQL traffic.

A PostgreSQL Provider may handle:

- registration/discovery of existing PostgreSQL Resources
- database/schema allocation
- Module-scoped accounts and permissions
- connection metadata
- secret references

Applications still query PostgreSQL directly through native clients.

```text
Java Module   → JDBC / PostgreSQL driver ─┐
Go Module     → pgx / database/sql        ├→ PostgreSQL
Node Module   → pg                        ├→ PostgreSQL
Python Module → psycopg                   ┘
```

Core does not embed JDBC or database-specific application clients.

### 10. Providers are not ordinary Modules

Providers are system-side components.

Current or future examples:

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

Capability matching may be shared while each Provider family receives its own lifecycle, privilege boundary, and protocol.

## Consequences

### Benefits

- one `requires.capabilities` grammar for ordinary requirements
- Module functionality and infrastructure resources can share SemVer contract matching
- forks and alternate implementations are not tied to concrete Module IDs
- an Instance can list compatible providers/resources before selecting a binding
- shared PostgreSQL infrastructure can later allocate isolated databases/schemas to multiple Modules
- the Resolver avoids feature-subset inference and remains bounded in complexity

### Costs

- Capability Contract Registry / descriptors are required
- SemVer range matching is required
- binding semantics vary depending on whether the provider source is a Module or Resource
- ambiguity policy is needed when multiple candidates exist
- Provider protocols and secret injection require additional design
- dependency graph / cycle validation is required

## Non-goals

This ADR does not finalize:

- the final Capability Descriptor JSON/YAML schema
- the final schema for Capability provider sources
- the final Instance binding schema
- automatic provider selection
- the PostgreSQL Provider Protocol
- database credential secret format
- Module package format
- Permission / trust model
- dependency-cycle policy
- feature-subset negotiation

These will be defined by follow-up design documents or ADRs during implementation.
