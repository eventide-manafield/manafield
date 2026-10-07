# Manafield Architecture

> This document describes the current architecture direction.  
> Manafield is still in the **pre-alpha / design stage**, so details may change.
>
> **The Korean documentation is the primary source of truth.**  
> If this English version differs from the Korean version, the Korean version takes precedence.

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

## 2. Development Baseline

The current Manafield Core development baseline is **Rust 1.99.0**.

```text
rustc 1.99.0
cargo 1.99.0
```

This is the current development and validation baseline. It does **not** yet define the project's official MSRV (Minimum Supported Rust Version).

A formal minimum supported Rust version may be defined later as the project stabilizes.

## 3. Core Philosophy

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

### Boundary-first Development

Manafield defines **boundaries and responsibilities that would be expensive to extract later according to the long-term architecture, while implementing only the internal behavior currently needed**.

This does not mean implementing every future feature up front. It means explicitly establishing boundaries such as:

- Core and Module
- Operation and Binding / Codec
- Registry write model and read snapshot
- Core and Runtime Provider
- ordinary Modules and privileged system components
- Module identity and Capability Contracts
- functional dependencies and infrastructure resource dependencies

Major architecture decisions and their rationale are recorded in [ADRs](adr/README.md).

### Role of Operations

In Manafield, an **Operation** is the common contract used to describe callable functionality exposed by a Module.

Core does not need to understand the Module's internal functions, classes, or implementation language. Instead, Operation Contracts in the Module Descriptor tell Core:

- which callable Operations are available
- which Input / Output Schemas they use
- which Binding is used to invoke them
- which Codec is used for payloads

This keeps Module implementations free while giving Core and other clients one consistent model for discovery and validation.

```mermaid
flowchart LR
    Module["Module"]
    Operation["Operation Contract"]
    Schema["Input / Output<br/>DataSchema"]
    Binding["Binding<br/>HTTP now, others later"]
    Codec["Codec<br/>JSON / MessagePack"]

    Module --> Operation
    Operation --> Schema
    Operation --> Binding
    Binding --> Codec
```

See [Operation](operation.md) for the detailed model.

### Role of Capabilities and Dependencies

An Operation represents one callable piece of functionality, while Module dependencies should not normally be pinned directly to a concrete implementation ID or route name.

Manafield separates **Capability Contracts** from Module identity.

```text
Module ID
→ concrete implementation identity

Operation
→ one callable contract

Capability
→ compatibility contract shared by interchangeable implementations

Tag
→ descriptive / non-binding metadata for search and classification
```

Searchable entries such as Modules, Capabilities, and Resources may carry an optional human-readable `description`. Description is display/search-assistance metadata and does not participate in dependency resolution or compatibility checks.

For example, Echo should depend on a `manafield.identity v1` Capability rather than the concrete `manafield-account` Module ID. A compatible fork or alternate implementation can then satisfy the same requirement.

The concrete Module is selected through Instance binding. When multiple candidates exist, Core should not silently choose one.

Infrastructure such as databases, caches, and storage is modeled as a separate **Resource Requirement**. Resources are prepared or allocated by Resource Providers rather than ordinary Modules, and Core does not proxy the application data path.

For example, a PostgreSQL Provider may create a database/schema/account and prepare connection secrets, while Echo still sends SQL directly through its own JDBC driver.

See [ADR-0010](adr/0010-capability-dependency-resolution.md) for the decision.

## 4. Headless Core

Manafield Core is designed to be headless.

The Core does not require a built-in Web UI to function.

```mermaid
flowchart LR
    Web["Official Web View"]
    CLI["CLI"]
    Mobile["Mobile Client"]
    Other["Other Clients"]
    Core["Manafield Core"]

    Web <-->|"Core API"| Core
    CLI <-->|"Core API"| Core
    Mobile <-->|"Core API"| Core
    Other <-->|"Core API"| Core
```

An official Web View may be provided separately, but the Core and Modules should remain usable without it.

## 5. High-level Architecture

```mermaid
flowchart TB
    Web["Manafield Web<br/>(Optional)"]
    Core["Manafield Core<br/>Rust<br/><br/>Registry · Lifecycle · Settings<br/>Permission · Health · Discovery<br/>Runtime · Routing · Events"]

    Module["Module"]
    Service["Service"]
    Integration["Integration"]

    Web -->|"Core API"| Core
    Core -->|"Module Protocol"| Module
    Core -->|"Module Protocol"| Service
    Core -->|"Module Protocol"| Integration
```

### Registry Read Model

The Module Registry separates the mutable write model from its read snapshot.

- `ModuleRegistry` is the source of truth for registration and mutation.
- Writes are serialized through an `RwLock`.
- After a successful mutation, Core builds a new immutable `RegistrySnapshot`.
- The current snapshot is atomically replaced through `ArcSwap`.
- Normal reads do not acquire the Registry `RwLock`; they read the current snapshot.
- Previous snapshots are automatically released through `Arc` reference counting once no readers still hold them.

```mermaid
flowchart LR
    Write["Register / Update / Remove"]
    Registry["ModuleRegistry<br/>Source of Truth<br/>RwLock"]
    Build["Build new<br/>RegistrySnapshot"]
    Swap["ArcSwap<br/>atomic swap"]
    Snapshot["Current immutable<br/>RegistrySnapshot"]
    Read["Read API / Web / CLI"]
    Old["Previous Snapshot<br/>kept only while referenced"]

    Write --> Registry
    Registry --> Build
    Build --> Swap
    Swap --> Snapshot
    Snapshot --> Read
    Swap -.-> Old
```

Snapshot construction and swapping occur while the Registry write lock is still held so concurrent writers cannot publish snapshots out of order. Readers can continue using the previous snapshot while a new one is being built.

Future Registry events may feed audit logs, WebSocket notifications, metrics, and similar consumers, but the primary read snapshot should not depend on asynchronous event processing for consistency.

## 6. Component Types

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

The Module / Service / Integration categories describe execution characteristics and responsibility while sharing as much of the common Module Protocol as practical.

### Provider

A Provider is not an ordinary Module. It is a **system-side component that connects Manafield to execution environments, infrastructure resources, or ingress functionality**.

Examples:

- Docker Runtime Provider
- PostgreSQL Resource Provider
- Traefik Ingress Provider / Adapter

Providers may require stronger privileges or host-infrastructure access than ordinary Modules, so they have separate boundaries.

## 7. Runtime Model

Manafield Core is not tied to a specific execution technology.

Actual Module creation, start, stop, removal, and runtime control are delegated to a **Runtime Provider**. A Runtime Provider is not an ordinary Module; it is a privileged system component that controls an execution environment.

Core must remain useful without a Runtime Provider for Registry, Operation discovery, validation, and similar base capabilities.

```mermaid
flowchart LR
    Core["Manafield Core<br/>no docker.sock"]
    Protocol["Runtime Protocol"]
    DockerProvider["Docker Runtime Provider<br/>separate process"]
    Docker["Docker daemon"]
    Modules["Module containers"]

    Core --> Protocol
    Protocol --> DockerProvider
    DockerProvider --> Docker
    Docker --> Modules
```

The initial Docker Provider is maintained in the **same repository / release** as Core but built and executed as a separate binary / process.

```text
same source / release
├─ manafield
└─ manafield-runtime-docker
```

In Docker deployment, Core does not receive `docker.sock`; only the Docker Runtime Provider does. This separates Docker-specific privilege and policy from general Core API and Registry logic.

Core and Provider communicate through a constrained **Runtime Protocol**. The protocol should express Manafield lifecycle operations rather than arbitrary Docker command execution.

For example:

```text
Create
Start
Stop
Remove
Status
```

Unix Domain Socket is the preferred initial local transport candidate. Future Runtime Providers may target local processes, remote hosts, Kubernetes, or other environments.

```mermaid
flowchart TB
    Core["Manafield Core"]
    RuntimeProtocol["Runtime Protocol"]

    DockerProvider["Docker Provider"]
    ProcessProvider["Process Provider<br/>(future)"]
    RemoteProvider["Remote Provider<br/>(future)"]
    K8sProvider["Kubernetes Provider<br/>(long-term)"]

    Core --> RuntimeProtocol
    RuntimeProtocol --> DockerProvider
    RuntimeProtocol -.-> ProcessProvider
    RuntimeProtocol -.-> RemoteProvider
    RuntimeProtocol -.-> K8sProvider
```

The detailed Runtime Protocol and capability model are still being designed. See [ADR-0004](adr/0004-runtime-provider-boundary.md) and [ADR-0005](adr/0005-docker-provider-isolation.md) for rationale.

### Provider Family

In addition to Runtime Providers, Manafield may use **Resource Providers** to prepare infrastructure resources.

For example:

```text
Provider
├─ Runtime Provider
│  └─ Docker
├─ Resource Provider
│  └─ PostgreSQL
└─ Ingress Provider / Adapter
   └─ Traefik
```

These share the broad property of adding system-side functionality without being ordinary Modules.

Runtime, Resource, and Ingress Providers may have very different privileges and lifecycles, so Manafield does not prematurely force them into one universal Provider Protocol. Each Provider family can receive its own protocol and security boundary when needed.

### Network Planes

Docker deployments separate internal Module communication from externally facing edge traffic.

```text
manafield-modules
→ internal Core ↔ Module communication

manafield-edge
→ shared edge network for Traefik / Cloudflared / externally exposed Modules
```

Core connects only to `manafield-modules` by default. Modules that need an external Web surface may additionally join `manafield-edge`.

`manafield-edge` is treated as an external Docker network owned by host infrastructure rather than by an individual Manafield Compose deployment. See [ADR-0008](adr/0008-network-planes.md) for rationale.

## 8. Web Contributions / Exposure

A Module does not need to provide a UI.

The fact that a Module provides a Web surface is separate from **where that surface is publicly exposed**. Concrete public exposure is selected by the private Instance Definition and an Ingress Provider.

The current Web Exposure model uses:

- `none` — no public Web exposure
- `host` — a dedicated hostname
- `prefix` — exposure under a path prefix
- `routes` — explicit root-level route claims
- `external` — an external URL outside Manafield

Public URLs are not derived automatically from Operation IDs or Binding paths.

See [ADR-0009](adr/0009-operation-web-exposure.md) for the decision.

## 9. Isolation

The default architecture avoids loading arbitrary Module code directly into the Core process.

```mermaid
flowchart LR
    Core["Manafield Core"]
    Module["Module Process / Container"]

    Core <-->|"Module Protocol / API"| Module
```

This makes language independence easier and provides a clearer security and failure boundary.

Third-party isolation may later include restricted containers or sandboxing.

## 10. Non-goals

Manafield does not aim to reimplement:

- Docker
- container isolation
- a database engine
- a reverse proxy
- Kubernetes

Manafield should orchestrate and compose existing technologies through explicit Module and Provider contracts rather than replace them.
