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
→ may be provided by a Resource Instance
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

Instances providing a Capability declare the **exact Capability version** they implement.

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

The Resolver does not infer that an implementation is compatible because it happens to implement only the subset of Operations used by one consumer.

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

### 7. Capabilities are provided by Module Instances or Resource Instances

Manafield does not require a shared language-level `CapabilityProvider` implementation interface.

The common requirement is the Capability metadata registered in the Manafield Registry.

```text
Module Instance
→ provides Capability metadata

Resource Instance
→ provides Capability metadata
```

The implementation language and runtime may differ.

Example:

```yaml
id: identity-core
kind: module
provides:
  capabilities:
    - id: manafield.identity
      version: "1.2.0"
```

```yaml
id: main-postgres
kind: resource
provides:
  capabilities:
    - id: database.postgresql
      version: "1.0.0"
```

A system-side component may create, register, or manage Resources, but the concrete Capability binding target is the Resource Instance rather than that management component.

### 8. Distinguish Module/Resource Definitions from Instances

A Definition describes a kind and its configuration shape, while an Instance represents a concrete installed or registered target.

```text
Definition
→ config schema
→ declaration shape

Instance
→ globally distinguishable instance ID
→ config values
→ endpoint / connection metadata
→ provided Capability metadata
```

Endpoints, concrete config values, secret references, and config schemas are not part of the Capability Contract.

The matching surface remains intentionally small:

```text
Capability
→ id + version
```

Descriptions and tags may exist as human-readable/search metadata, but do not participate in compatibility matching.

### 9. Instance IDs are unique within a Manafield Instance

Concrete IDs for Module Instances and Resource Instances may not collide within one Manafield Instance.

Core rejects duplicate registration as a conflict rather than silently renaming the target.

```text
main-postgres
identity-core
echo-prod
```

Bindings target these unique Instance IDs.

### 10. A Binding maps a Requirement slot to a target Instance ID

Requirements and concrete selection remain separate.

Example:

```yaml
requires:
  capabilities:
    identity:
      id: manafield.identity
      version: "^1.0.0"

    state:
      id: database.postgresql
      version: "^1.0.0"
```

Conceptual bindings on the consumer Instance:

```yaml
bindings:
  identity: identity-core
  state: main-postgres
```

`identity` and `state` are Requirement slots/local aliases.

`identity-core` and `main-postgres` are concrete target Instance IDs.

Binding has only two states:

```text
UNBOUND
BOUND
```

BOUND does not mean valid.

Binding records only whether a target was selected. Target existence, Capability presence, and version compatibility are separate validation/diagnostic concerns.

### 11. The Resolver does not automatically bind

Even when there is exactly one compatible Instance, Core does not create a Binding automatically.

```text
compatible candidate count = 0
→ UNBOUND remains UNBOUND

compatible candidate count = 1
→ UNBOUND remains UNBOUND

compatible candidate count > 1
→ UNBOUND remains UNBOUND
```

Concrete target selection is explicit.

Whether a new Module reuses an existing Resource, creates a new Resource, or presents a user choice belongs to the Module management layer rather than the Core Resolver.

### 12. Capability Discovery is separate from Binding

Discovery is a read-only concern.

Three base query levels are defined.

#### Compatible discovery

Return only Instances whose Capability ID and SemVer range are compatible.

#### Advanced discovery

Return Instances with the same Capability ID even when the version is outside the requested range, and expose compatibility metadata.

#### Full discovery

Allow listing registered Instances and their provided Capability metadata without a Capability filter.

Discovery never creates or changes a Binding.

### 13. Version mismatch is a Warning and does not hard-block execution

A bound target may provide the same Capability ID while declaring a version outside the required range.

In that case:

```text
CAPABILITY_VERSION_MISMATCH
→ WARNING
→ emit diagnostic log
→ continue execution
```

The SemVer range expresses an expected contract compatibility range. When the user explicitly bound a target Instance, Core does not block execution solely because of a declared version mismatch.

A missing target Instance or a target that does not provide the required Capability at all remains a separate validation/error concern.

### 14. Resource management components are not application-data proxies

A system-side component may discover, register, provision, or manage Resources.

For PostgreSQL this may include:

- discovery/registration of existing PostgreSQL Resources
- database/schema allocation
- Module-scoped accounts and permissions
- connection metadata
- secret references

Application SQL traffic still does not flow through Core or the management component.

```text
Java Module   → JDBC / PostgreSQL driver ─┐
Go Module     → pgx / database/sql        ├→ PostgreSQL
Node Module   → pg                        ├→ PostgreSQL
Python Module → psycopg                   ┘
```

### 15. System-side Provider families retain separate boundaries

Runtime Providers, Resource management components, and Ingress Providers/Adapters may require privileges and lifecycles different from ordinary Modules.

Manafield does not prematurely collapse them into one universal Provider Protocol.

The important distinction is:

```text
Capability binding target
→ Module Instance or Resource Instance

System component that prepares/manages a Resource
→ separate lifecycle/protocol concern
```

## Consequences

### Benefits

- ordinary requirements remain under one `requires.capabilities` grammar
- the Capability matching surface stays small at `id + version`
- implementation language is decoupled from Capability Registry metadata
- Module Instances and Resource Instances share one Binding model
- Binding is simplified to Requirement slot → unique Instance ID
- Discovery and Binding are separated, removing implicit selection behavior
- version mismatch remains diagnosable while respecting explicit user bindings
- endpoint/config/schema/secret details do not bloat Capability Contracts

### Costs

- Instance ID uniqueness must be enforced
- Capability Discovery APIs/UI are required
- Binding validation and diagnostic/error models are separate concerns
- SemVer range matching and warning reporting are required
- Module/Resource Definition and Instance boundaries must be implemented
- Resource management lifecycle/protocol and secret materialization need separate design
- dependency graph/cycle validation is required

## Non-goals

This ADR does not finalize:

- the final Capability Descriptor JSON/YAML wire schema
- storage/distribution of the Capability Contract Registry
- SemVer parser/library
- pre-release version policy
- final representation for one Instance providing multiple versions of the same Capability
- final Resource-management protocol
- database credential/secret materialization format
- Module package format
- Permission/trust model
- dependency-cycle policy
- feature-subset negotiation

The following points are explicitly decided by this ADR:

```text
automatic binding
→ never performed

binding state
→ UNBOUND / BOUND

binding target
→ unique Instance ID

discovery
→ compatible / advanced / full

version mismatch
→ warning + log + continue

Capability runtime/config payload
→ excluded
```

