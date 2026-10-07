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

Instances providing a Capability declare exact versions:

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

## 4. Capability declarations

The matching surface of a Capability is intentionally small.

```yaml
provides:
  capabilities:
    - id: manafield.identity
      version: "1.2.0"
```

The core fields are:

```text
id
→ stable Capability Contract ID

version
→ exact Capability Contract version
```

Human-readable metadata such as `description` and `tags` may exist, but does not participate in matching.

Endpoints, connection details, concrete config values, secrets, and config schemas do not belong to the Capability contract.

## 5. Instances that provide Capabilities

Manafield does not require a shared language-level `CapabilityProvider` implementation interface.

Instead, concrete instances expose common **registry metadata** that Manafield can understand.

Two concrete kinds may currently provide a Capability:

```text
Module Instance
Resource Instance
```

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

The implementation may be Python, Java, Rust, Go, or another environment as long as the registry metadata follows the same contract.

A system-side component may create, register, or manage Resources, but the concrete Capability binding target is the **Resource Instance**, not that management component.

## 6. Definitions and Instances

Modules and Resources distinguish their definitions from concrete instances.

```text
Definition
→ kind and declaration structure
→ configuration schema

Instance
→ concrete installed/registered target
→ concrete config values
→ endpoint / connection metadata
→ provided Capability metadata
```

Example:

```text
PostgreSQL Resource Definition
  config schema
    host: string
    port: integer
    database: string

main-postgres Resource Instance
  endpoint: postgres:5432
  config values:
    database: manafield
  provides:
    database.postgresql@1.2.0
```

Capabilities do not own these configuration structures.

## 7. Instance IDs

Module Instances and Resource Instances must have **unique Instance IDs** within one Manafield Instance.

```text
main-postgres
identity-core
echo-prod
```

If the ID is already registered, Core rejects the duplicate as a conflict rather than silently renaming it.

An installation UI may suggest another ID, but Registry semantics still require uniqueness.

## 8. Matching

Default compatibility matching is:

```text
required Capability ID == provided Capability ID
AND
provided exact SemVer satisfies required SemVer range
```

Example:

```text
Requirement
database.postgresql ^1.0.0

main-postgres  provides 1.2.0  compatible
dev-postgres   provides 1.0.3  compatible
old-postgres   provides 0.9.0  version mismatch
redis-main     provides cache.redis 1.0.0  different capability
```

The Resolver does not infer compatibility from Operation subsets or implementation ancestry.

## 9. Capability Discovery

Discovery is a read-only concern separate from Binding.

### Default discovery

Return only Instances whose Capability ID matches and whose exact provided version satisfies the requested range.

### Advanced discovery

Return Instances with the same Capability ID even when the version is outside the requested range, and expose compatibility metadata.

```text
main-postgres  1.2.0  compatible
old-postgres   0.9.0  incompatible: version
```

### Full discovery

Allow listing registered Instances and their provided Capability metadata without a Capability filter.

Discovery **never creates a Binding automatically**.

Whether there are zero, one, or many compatible candidates, an unbound Requirement remains UNBOUND until a target Instance ID is explicitly selected.

## 10. Binding

A Binding is a **Requirement slot → target Instance ID** relationship.

Given a consumer Module Instance with:

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

its concrete bindings are conceptually:

```yaml
bindings:
  identity: identity-core
  state: main-postgres
```

`identity` and `state` are Requirement slots/local aliases, not Capability IDs.

`identity-core` and `main-postgres` are registered target Instance IDs.

Binding has only two states:

```text
UNBOUND
→ no target Instance ID is selected

BOUND
→ a target Instance ID is selected
```

BOUND does not imply valid. Binding records only whether a target was selected; validity is handled by a separate validation/diagnostic layer.

## 11. Validation and version mismatch

A bound target may be validated before or during execution:

```text
Does the target Instance exist?
Does it provide the requested Capability ID?
Does the declared Capability version satisfy the requested range?
```

A missing target or an entirely different Capability cannot serve as a normal valid binding.

However, a **Capability version mismatch alone does not block execution**.

```text
CAPABILITY_VERSION_MISMATCH
→ WARNING
→ emit sufficient diagnostic logs
→ continue execution
```

Example:

```text
WARN CAPABILITY_VERSION_MISMATCH
consumer: echo-prod
slot: state
required: database.postgresql ^1.0.0
bound: main-postgres
provided: database.postgresql 2.0.0
```

The SemVer range expresses contract compatibility expectations. When the user explicitly bound a target, Core does not hard-block that target solely because its declared version is outside the range.

## 12. Resource runtime information

Endpoints, connection metadata, concrete config values, and secret references live on the concrete Resource Instance or Instance configuration.

```yaml
id: main-postgres
kind: resource

endpoint:
  host: postgres
  port: 5432

config:
  database: manafield
  ssl: false

provides:
  capabilities:
    - id: database.postgresql
      version: "1.2.0"
```

The **config schema** describing which settings a Resource kind accepts belongs to the Resource Definition.

The same split may be used for Modules:

```text
Module Definition
→ config schema

Module Instance
→ concrete config values
→ bindings
→ provided metadata
```

Capability contracts do not contain endpoint/config/schema/secret materialization details.

## 13. Resource management components

A system-side component may discover, register, provision, or manage a Resource such as PostgreSQL.

Example responsibilities:

```text
PostgreSQL resource manager/provider
├─ discover/register existing PostgreSQL
├─ allocate databases/schemas
├─ prepare accounts/permissions
├─ prepare connection metadata
└─ prepare secret references
```

Application data traffic still does not flow through Core or this management component.

```text
Echo ──JDBC/driver──> PostgreSQL
```

Core does not scan the filesystem for Resources or interpret concrete implementations such as PostgreSQL.

The initial live-registration flow is:

```text
Resource Provider
→ prepare/observe a concrete Resource
→ POST /resources
→ register generic Resource Instance metadata
→ publish a new RegistrySnapshot
```

The current generic registration metadata covers `id`, `name`, `type`, `description`, and `provides.capabilities`. `type` is opaque provider-specific metadata that Core does not interpret.

Module Instances and Resource Instances share the same Instance ID namespace, so duplicate IDs are rejected as conflicts.

The management component and the concrete Resource Instance are not the same concept for Capability matching.

## 14. Feature subsets

Capability compatibility is evaluated at whole-contract granularity.

```text
"cherry-picked one v3 Operation"
≠
"implements Capability v3"
```

The implementation declares which Capability version it fully implements.

Manafield does not reinterpret compatibility from only the Operation subset used by one consumer.

## 15. Description and tags

Optional `description` metadata may exist on:

- Module
- Operation
- Capability
- Resource

Descriptions and `tags` support search/management UI and do not participate in Capability matching.

## 16. Current fixed principles

```text
Requirement
→ Capability only

Capability matching surface
→ id + version

provides
→ exact contract version

requires
→ SemVer range

Capability source
→ Module Instance or Resource Instance

Instance ID
→ unique within a Manafield Instance

Binding
→ Requirement slot → target Instance ID

Binding state
→ UNBOUND / BOUND

Discovery
→ compatible / advanced-name-match / all

Automatic Binding
→ never performed by discovery/resolver

Version mismatch
→ warning + log + continue

Endpoint / config value
→ concrete Instance

Config schema
→ Module/Resource Definition
```

## 17. Still open

- storage/distribution of the Capability Contract Registry
- final location/format of Capability definition files
- SemVer parser/library
- pre-release version policy
- final representation for one Module/Resource Instance providing multiple versions of the same Capability
- dependency-cycle policy
- lifecycle/protocol of Resource management components
- final secret materialization schema
