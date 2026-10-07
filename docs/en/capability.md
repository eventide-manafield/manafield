# Capability Contract v0

> Initial design for the Capability-based Requirement Resolution established by ADR-0010.
>
> Manafield is still pre-alpha, so the final wire schema may be refined during implementation.

## 1. Core principle

All ordinary requirements in Manafield are expressed as **Capability Requirements**.

```text
needs functionality from another Module
→ Capability

needs a database
→ Capability

needs a cache
→ Capability

needs storage
→ Capability
```

There is no separate `requires.resources` or `requires.services` namespace.

## 2. Requirement

A Module gives each required Capability a local alias.

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

`identity` and `state` are local dependency names.

`manafield.identity` and `database.postgresql` are global Capability Contract IDs.

## 3. Capability version

A Capability version is a **contract version**.

It is independent from a Module release version.

```text
Module version
→ concrete implementation release

Capability version
→ compatibility contract version
```

Capability versions use SemVer.

Providers declare exact versions:

```yaml
version: "2.3.1"
```

Consumers declare SemVer ranges:

```yaml
version: "^2.1.0"
```

Initial meaning:

```text
^2.1.0
→ >=2.1.0 <3.0.0
```

MAJOR / MINOR / PATCH:

```text
MAJOR
→ backward-incompatible contract change

MINOR
→ backward-compatible contract expansion

PATCH
→ correction preserving contract meaning
```

## 4. Module-provided Capability

A Module may declare Capabilities it fully implements.

```yaml
provides:
  capabilities:
    - id: manafield.identity
      version: "1.2.0"
      description: User identity and session functionality
```

`description` is optional human-readable metadata and is not used for matching.

When a Capability Contract requires Operations, Core may validate mechanically checkable requirements against the actual Module Descriptor.

## 5. Resource-provided Capability

Infrastructure uses the same Capability matching model.

Example Resource registered in an Instance:

```yaml
id: main-postgres
description: Manafield instance shared PostgreSQL

provides:
  capabilities:
    - id: database.postgresql
      version: "1.0.0"
      description: PostgreSQL database connection capability
```

`main-postgres` is a **concrete Resource ID**, not a Capability ID.

```text
database.postgresql
→ contract

main-postgres
→ concrete Resource providing that contract
```

A PostgreSQL Provider may discover, register, or create the Resource.

## 6. Matching

Basic Resolver matching is intentionally simple.

```text
required capability ID == provided capability ID
AND
provided exact SemVer satisfies required SemVer range
```

Example:

```text
Requirement
database.postgresql ^1.0.0

Candidates
main-postgres  provides 1.2.0  ✅
dev-postgres   provides 1.0.3  ✅
old-postgres   provides 0.9.0  ❌
redis-main     provides cache.redis 1.0.0 ❌
```

The Resolver does not infer Operation subsets or implementation ancestry.

## 7. Binding

Capability Requirements and concrete provider selection are separate.

Conceptual example:

```yaml
bindings:
  capabilities:
    echo.identity:
      module: better-account

    echo.state:
      resource: main-postgres
```

Both are Capability bindings.

The final binding schema will be stabilized with the Build Plan / Instance Definition design.

## 8. Provider source

Capability provider sources initially include:

```text
Capability Provider Source
├─ Module
└─ Resource
```

### Module

Provides callable functionality through Operations.

Example:

```text
manafield.identity
→ Account Module
```

### Resource

Provides infrastructure that must be materialized as connection/configuration data.

Example:

```text
database.postgresql
→ main-postgres Resource
→ managed by PostgreSQL Provider
```

The Capability matching model is shared even when binding materialization differs by provider source.

## 9. Resource Provider

A Resource Provider is not an ordinary Module.

PostgreSQL Provider example:

```text
PostgreSQL Provider
├─ discover/register existing PostgreSQL
├─ manage Resource metadata
├─ allocate database/schema
├─ prepare accounts/permissions
├─ prepare connection metadata
└─ prepare secret references
```

Application SQL traffic does not pass through Core or the Provider.

```text
Echo ──JDBC/driver──> PostgreSQL
```

## 10. Feature subsets

Capability compatibility is evaluated at whole-contract granularity.

```text
"cherry-picked one v3 Operation"
≠
"implements Capability v3"
```

The Module/Provider author declares which Capability version is fully implemented.

Manafield does not reinterpret compatibility based on only the Operation subset used by a specific consumer.

## 11. Description and tags

The following may carry optional `description` metadata:

- Module
- Operation
- Capability
- Resource

Descriptions are for search and management UI.

`tags` are also discovery/classification metadata.

Neither participates in Capability matching.

## 12. Deferred details

- storage/distribution of the Capability Contract Registry
- final location/format of Capability definition files
- SemVer parser/library
- pre-release version policy
- final representation for providing multiple versions of one Capability
- automatic binding policy
- dependency-cycle policy
- provider-specific binding materialization schemas
