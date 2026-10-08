# Manafield Roadmap

> Roadmap for the current redesign.  
> Order and scope may change as the architecture is validated.
>
> **The Korean documentation is the primary source of truth.**  
> If this English version differs from the Korean version, the Korean version takes precedence.

## Development Principle

Manafield defines **long-term boundaries and responsibilities early, while implementing only as much internal behavior as currently needed**.

- Define boundaries now when they would be expensive to extract later.
- Future capabilities may remain as interfaces or minimal implementations until needed.
- Core must remain usable for independent functions without a specific Runtime or UI.
- A supported full v0 Instance build/deployment requires Docker.
- The same Instance must be buildable through `manafield` CLI + Docker without Jenkins.
- Powerful privileges should be constrained to the smallest practical component.
- Major architecture decisions are recorded as ADRs.

ADR index: [Architecture Decision Records](adr/README.md)

## Phase 0 — Core Foundation

- [x] Rust Core bootstrap
- [x] Rust 1.99.0 toolchain pinning
- [x] Core HTTP API bootstrap
- [x] Core Dockerfile
- [x] Module Descriptor model
- [x] Operation Contract model
- [x] DataSchema model
- [x] Binding / Codec separation
- [x] JSON / MessagePack serialization
- [x] Health Operation reference
- [x] Module Manifest discovery
- [x] Module validation
- [x] Runtime Module Registry API
- [x] Immutable RegistrySnapshot
- [x] ArcSwap-based lock-free read model
- [x] Built-in Headless CLI v0 — `health` / `ps` / `resource`
- [ ] Module CLI contribution contract
- [ ] Stabilize Module Protocol v0 documentation
- [ ] Define Registry update semantics

## Phase 0.5 — Manafield Reference

- [x] Independent repository: `manafield-module-reference`
- [x] Node.js / TypeScript Module server
- [x] React Registry Observer
- [x] `/manafield/health` Operation
- [x] Core Registry read integration
- [x] Dockerfile
- [x] Core + Manafield Reference Compose environment
- [x] Register the Manafield Reference in the live Core Registry
- [x] Observe the Manafield Reference from its own UI
- [x] Extract Module Template requirements from the reference implementation → [Module Template Requirements v0](module-template-requirements.md)
- [x] Compare existing Echo service against Template boundaries → [Echo Module Migration Review](echo-module-migration-review.md)
- [ ] Stabilize the common Module Template Contract
- [ ] Define a Go Web implementation profile
- [ ] Define a TS / React Web implementation profile
- [ ] Define a Java / Spring Web implementation profile
- [ ] Verify the Template can describe a stateful Web Module requiring the `database.postgresql` Capability
- [ ] Create / implement the new private Echo v2 repository
- [ ] Register Echo v2 in the live Core Registry
- [ ] Revalidate the Template Contract with a second real Module
- [ ] Extract implementation-specific Template repositories when useful

## Phase 1 — Runtime Provider Boundary

Runtime implementations are treated as **privileged system components**, not ordinary Modules.

- [x] Architectural separation between Runtime Providers and ordinary Modules
- [x] Core must not depend on any specific Runtime
- [x] Process boundary direction for separating Docker privileges from Core
- [ ] Define Runtime Protocol v0
- [ ] Define Runtime request / response types
- [ ] Runtime Provider identity / capability model
- [ ] Select and implement local Runtime transport
  - [ ] Evaluate / implement Unix Domain Socket as the initial transport
- [ ] Core Runtime client
- [ ] Runtime Provider health / availability
- [ ] Runtime Provider policy model

### Docker Runtime Provider

The initial direction is to keep Docker Provider code in the same repository while building it as a **separate binary / process**.

- [ ] Organize Cargo workspace / crates
- [ ] `manafield` Core binary
- [ ] `manafield-runtime-docker` binary
- [ ] Docker API client
- [ ] Mount Docker socket only into the Runtime Provider
- [ ] Do not mount Docker socket into the Core container
- [ ] Create / Start / Stop / Remove / Status
- [ ] Docker network attachment
- [ ] Resource limits
- [ ] Policy denying privileged containers
- [ ] Restrict arbitrary host path mounts
- [ ] Prevent Docker socket re-exposure
- [ ] Runtime logs

## Phase 1.5 — Capability Resolution / Instance Binding

- [x] Separate Module identity from dependency contracts
- [x] Separate Tag and Capability semantics
- [x] Unify ordinary requirements under Capability Contracts
- [x] ADR-0010 — Capability-Based Dependency Resolution
- [x] Optional Module / Operation `description` metadata
- [x] Capability Contract v0 design → [Capability Contract](capability.md)
- [x] Keep the Capability matching surface to `id + version`
- [x] Shared Capability registry metadata for Module Instances / Resource Instances
- [x] Unique Instance IDs / duplicate conflict policy
- [x] Binding = Requirement slot → target Instance ID
- [x] Binding state = `UNBOUND` / `BOUND`
- [x] No automatic Binding by Resolver/Discovery
- [x] Three Discovery levels: compatible / advanced same-ID / full
- [x] Capability version mismatch = warning + log + continue
- [x] endpoint/config values on Instances; config schema on Definitions
- [x] Implement `provides.capabilities` / `requires.capabilities` in Core models
- [ ] Capability ↔ Operation contract validation
- [x] Capability SemVer / range policy
- [ ] Implement concrete Binding persistence/query
  - [x] Preserve Instance Definition / Build Plan bindings and validate target existence
  - [x] Inject v0 Module runtime binding target environment
- [ ] Implement Capability Discovery API
- [ ] Binding validation / diagnostic error model
- [ ] Dependency graph / cycle validation
- [x] Resource Instance Capability metadata / generic Registry API v0
- [x] Shared Instance ID namespace / duplicate conflict across Modules and Resources
- [x] PostgreSQL Resource Provider registration/watch example
- [ ] Resource management component lifecycle / protocol
- [ ] Secret / connection metadata injection model
- [ ] PostgreSQL Resource management v0 (`database.postgresql` Capability)
- [ ] Per-Module schema/role allocation / Capability binding inside a shared PostgreSQL Resource database

## Phase 2 — Module Lifecycle

- [ ] Module install model
- [ ] Runtime requirement declaration
- [ ] Runtime Provider selection
- [ ] Module create
- [ ] Start / Stop / Restart
- [ ] Runtime status
- [ ] Actual Health Operation invocation
- [ ] Health state model
- [ ] Connect Registry and Runtime state
- [ ] Failure / retry policy
- [ ] Runtime cleanup on removal

## Phase 3 — Official Web View

- [x] Separate the Official Web Shell from Core
- [x] Select same-origin path composition
- [x] ADR-0011 — Official Web Shell / Same-Origin Module Composition
- [x] ADR-0012 — Replaceable Homepage Module / Web Shell Separation
- [x] Create independent `manafield-web` repository
- [x] Bootstrap Go single-binary Web Shell
- [x] `/manafield/health` Operation
- [x] Core Registry reads
- [x] Web Shell runtime base path (`MANAFIELD_WEB_BASE_PATH`)
- [x] Replaceable Homepage Module boundary / local `manafield-home` prototype
- [ ] Registry-driven dynamic Module navigation
- [ ] Registry / Operation browser
- [ ] Runtime Provider status UI
- [ ] Module Health UI
- [x] Web Surface metadata v0 (`page / api`) → [Web Surface v0](web-surface.md)
- [ ] Web Exposure model (`none / host / prefix / routes / external`)
- [x] Build Plan same-host prefix validation / preserved prefixes
- [x] Prefix duplicate conflict / specificity precedence
- [ ] Normalize `routes` exposure / validate exact-route conflicts
- [x] Render Traefik same-host prefix composition (no prefix stripping)
- [ ] Manafield reserved root-path policy
- [ ] Generate / apply ingress configuration from the resolved Build Plan
- [ ] login / session entry UX
- [ ] Settings UI

### Module Manager Module

Prefer an **official management Module** over embedding a management UI directly into Core.

- [ ] Module list / status view
- [ ] Attach / enable Modules
- [ ] Disable / detach / remove Modules
- [ ] Select Module source / version
- [ ] Configure Web exposure (`host / prefix / routes`)
- [ ] Preview route conflicts / impact before applying changes
- [ ] Preview and confirm Instance configuration diffs
- [ ] Apply lifecycle changes through Core / Runtime Provider APIs
- [ ] Do not provide the management Module direct Docker socket access

## Phase 4 — Reference / Existing Services

- [ ] Observe
- [ ] ProtoDuck Integration
- [ ] Echo Service
- [ ] Validate registration of already-running external Services without a Runtime Provider

## Intermediate Goal — Misskey

- [ ] **Run and manage Misskey as a Manafield Service Module**

This milestone should validate management of a realistic independent service with:

- an application runtime
- PostgreSQL
- Redis
- persistent storage
- networking
- configuration
- health checks
- Web access
- lifecycle management

If Misskey can be modeled cleanly without Misskey-specific Core behavior, both the Service Module abstraction and Runtime Provider boundary are probably on the right track.

## Architecture Decision Records

- [x] ADR-0001 — Rust Headless Core
- [x] ADR-0002 — Operation Contract
- [x] ADR-0003 — Registry Snapshot / ArcSwap Read Model
- [x] ADR-0004 — Runtime Provider Boundary
- [x] ADR-0005 — Docker Runtime Provider Process Isolation
- [x] ADR-0006 — Instance Build Plan / single Pipeline per instance
- [x] ADR-0007 — Public Platform / Private Instance Configuration
- [x] ADR-0008 — Manafield Network Planes
- [x] ADR-0009 — Separate Operation Routing from Web Exposure
- [x] ADR-0010 — Capability-Based Dependency Resolution
- [x] ADR-0011 — Official Web Shell / Same-Origin Module Composition
- [x] ADR-0012 — Replaceable Homepage Module / Web Shell Separation
- [x] ADR-0013 — Docker-required v0 Deployment / CLI-first Execution
- [ ] Add follow-up ADR when Runtime Protocol becomes concrete
- [ ] Add follow-up ADR when Module package format becomes concrete
- [ ] Add follow-up ADR when Permission / trust model becomes concrete

## Phase 5 — CI/CD / Instance Build Plan

- [x] One Pipeline per instance
- [x] No Jenkins Job per Module
- [x] Start the Module build contract with a source-root Dockerfile
- [x] Separate Module source types into `git / dir`
- [x] Support Git Module source `subdir` for monorepo layouts
- [x] Jenkins local Module discovery through `LOCAL_MODULES_ROOT`
- [x] Instance Definition v0
- [x] Separate public platform code from private Instance Configuration
- [x] Build Plan Resolver v0
- [x] Integrate the Build Plan Resolver as built-in CLI `manafield plan`
- [x] Move the Jenkins Resolve Build Plan stage to `manafield plan`
- [x] Implement `manafield deploy RELEASE_DIR` and migrate the Jenkins Deploy stage
- [x] Implement `manafield build WORKSPACE REVISION CORE_IMAGE` and migrate the Jenkins Build Images stage
- [x] Define CLI + Docker as the canonical Instance execution path
- [x] Define Jenkins as an optional remote CI/CD frontend
- [x] Single Jenkins Pipeline skeleton
- [x] Generate dynamic Module Compose services from the resolved Build Plan
- [x] First-run bootstrap from `instance.bootstrap.yaml` when `instance.yaml` is absent
- [x] One-time Bootstrap Wizard skeleton / Account implies PostgreSQL + Example Web
- [x] Wire selectable PostgreSQL Resource example through bootstrap deployment
- [ ] Implement bootstrap Example Web
- [ ] Implement bootstrap Example Account
- [ ] Connect GitHub webhooks to the same Instance Pipeline
- [ ] Selective build based on changed source
- [ ] Core / Runtime Provider / Module image tagging
- [ ] Post-deploy Health / Protocol verification
- [ ] Secret injection through CI credential storage
- [ ] Gradually move Jenkins Checkout / Build / Materialize / Stage / Deploy / Verify semantics into CLI/reusable executors
- [ ] Implement remaining `manafield verify / rebuild` CLI surfaces and stages
- [ ] Validate a full Instance rebuild using only CLI + Docker, without Jenkins

### Ingress Adapters

- [ ] Stabilize common Ingress Provider input / lifecycle
- [x] Traefik Ingress Adapter v0 — host exposure
- [x] Build Plan hostname conflict detection
- [x] Render Traefik prefixes / nested-prefix priority
- [ ] Render Traefik exact root route claims
- [ ] Nginx Ingress Adapter
- [ ] Ingress rollback / stale-route cleanup

## Long-term

- [ ] Module package format
- [ ] Module install / update / remove
- [ ] Drag & Drop Module installation
- [ ] Permission review
- [ ] Distributed / advanced dependency management
- [ ] Distributed Module Registry / discovery
- [ ] Compatibility Test Kit
- [ ] Third-party Module isolation
- [ ] External Runtime Provider protocol
- [ ] Remote Runtime Provider
- [ ] Kubernetes Runtime Provider

Kubernetes is intentionally a distant target. The general Core model remains separated from Docker-specific privilege, but a **supported full Manafield v0 Instance build/deployment requires Docker**. Docker-free runtimes are future extension points, not v0 compatibility targets.
