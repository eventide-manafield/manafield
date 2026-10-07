# Module Template Requirements v0

> This document captures the **initial Module Template requirements** extracted from the current `manafield-reference` implementation.
>
> It separates Module Protocol requirements from language/framework-specific template conventions.
> The project is still pre-alpha, so these requirements may be refined after additional real Modules are implemented.

## 1. Purpose

A Module Template should let a developer start a Module without cloning Manafield Core or depending on Core implementation details.

The template should:

- satisfy the Protocol contract quickly
- provide a buildable/runnable baseline
- provide health visibility
- keep Core independent from the implementation language
- avoid copying Reference-specific functionality

> **The protocol is the contract.**

A Template must not require a Manafield Core SDK.

## 2. Contract required by every Module

### Required

Current Module discovery expects:

```text
manafield.module.json
```

The descriptor contains at least:

- `id`
- `name`
- `version`
- `operations`

Search/listing UI may also use optional display metadata:

- Module `description`
- Operation `description`

Descriptions are nullable/optional and do not participate in dependency or compatibility resolution.

Each Operation follows the currently supported Binding and Codec rules.

Current HTTP Binding validation includes:

- path begins with `/`
- method is supported by the current Core implementation
- at least one Codec is declared
- Input / Output use Manafield `DataSchema`

### Health Operation

`healthOperation` is optional in the Protocol, but a normal runnable Module Template should provide one by default.

Recommended initial endpoint:

```text
GET /manafield/health
```

The path itself is not a mandatory Protocol rule. The actual contract is the Operation Binding declared in the descriptor.

If a Health Operation is declared:

- it must reference an existing Operation ID
- its `input` must be `null`

## 3. Build Contract

The initial CI/CD model uses a root Dockerfile or a Dockerfile explicitly selected by the Instance Definition.

A container-based Template should therefore:

- build from the Module source repository alone
- include all runtime files in the runtime image
- allow the runtime/instance to select the listen port
- use a non-root user where practical
- provide an application-level health endpoint

The Dockerfile must not require Manafield Core source.

## 4. Runtime Configuration

The following are **container-template conventions, not Protocol requirements**.

### `PORT`

HTTP server templates should allow the listen port to be overridden with `PORT`.

### `MANAFIELD_CORE_URL`

Use only for Modules that actually need to call Core directly.

Not every Module needs to know the Core URL.

Because Core invocation may later evolve with Runtime/Service Discovery, this is not fixed as a universal Template requirement.

## 5. Web Exposure is not a Template contract

A Module Template does not hard-code a public hostname or public path.

Public exposure belongs to the private Instance Definition:

```yaml
exposure:
  type: host
  host: example.manafield.studio
  targetPort: 8080
```

Templates therefore do not contain:

- a specific `*.manafield.studio` hostname
- Traefik/nginx-specific routes
- instance-specific TLS configuration
- Cloudflare Tunnel configuration

The Module only needs to serve correctly on its internal port.

See ADR-0009 for this boundary.

## 6. Template layers and implementation profiles

The common Module Template does not make one framework the platform standard.

```text
Module Template Contract
├─ Go Web profile
├─ TS / React Web profile
├─ Java / Spring Web profile
└─ future implementation profiles
```

The common Contract covers Protocol requirements, build/runtime boundaries, Health, optional description metadata, and dependency/resource declaration rules.

Each implementation Profile only provides a convenient skeleton for satisfying the same Contract in a particular stack.

### Go Web profile candidate

Prefer this profile for lightweight HTTP/Web Modules and the Official Web Shell.

Candidate defaults:

- Go single executable
- prefer standard-library `net/http`
- static assets through `embed.FS`
- runtime-configurable port
- non-root runtime user
- Docker HEALTHCHECK
- minimal runtime image
- `manafield.module.json`
- Protocol Health Operation
- no database dependency unless the Module actually needs one

Add a richer router/framework only when a concrete need appears. React/Vite may still be used at build time while keeping the production runtime as a single Go binary.

### TypeScript / React Web profile candidate

Reusable baseline currently visible in `manafield-reference`:

```text
manafield.module.json
Dockerfile
package.json
package-lock.json

server/
  index.ts

src/
  main.tsx
  App.tsx

index.html
vite.config.ts
tsconfig.app.json
tsconfig.server.json
```

Recommended scripts:

```text
npm run check
npm run build
npm start
```

Current implementation baseline:

- Node.js 22
- TypeScript strict mode
- React
- Vite
- Node/Express server

This stack is a **TS/React Template implementation choice**, not a Module Protocol requirement.

### Java / Spring Web profile candidate

To support a greenfield Echo v2 or similar stateful Web Modules, useful candidate defaults include:

- pinned Java toolchain
- Gradle Wrapper
- Spring Boot Web
- JDK build / JRE runtime multi-stage image
- non-root runtime user
- Docker HEALTHCHECK
- environment-driven application port
- `manafield.module.json`
- Protocol Health Operation
- a clean boundary for materializing Capability bindings into application configuration

PostgreSQL/JPA is not a universal Java Web requirement. Only Modules that need it should require a Capability such as `database.postgresql`.

## 7. Do not copy these Reference-specific parts into a generic Template

### Registry Observer UI

The Registry Observer, Module cards, and Operation rendering exist specifically for the Reference Module.

### Core Registry proxy

```text
GET /api/core/modules
```

and its use of `MANAFIELD_CORE_URL` are Reference-specific behavior.

### Local TypeScript copies of Manafield types

```text
src/types/manafield.ts
```

exists to consume Registry responses.

It should not become a required Template file. A future TypeScript SDK or generated contract may replace this kind of duplication.

### Vite `/api` development proxy

This is local-development convenience for the Reference implementation, not a Protocol requirement.

## 8. Good defaults confirmed by the Reference

Useful defaults worth carrying into a Template:

- non-root runtime container
- Docker HEALTHCHECK
- production-only dependencies in the runtime stage
- TypeScript strict mode
- frontend/server type checking before build
- separate development and production execution
- application Health endpoint connected to container Healthcheck

For Node templates, prefer `npm ci` over `npm install` when a lockfile is present for reproducible builds.

## 9. Do not freeze these yet

Wait for additional real Module implementations before standardizing:

- Settings endpoints
- final Permission declaration schema
- final Capability Descriptor / provider-source JSON/YAML schema
- Capability version-range / negotiation syntax
- final Capability binding / Secret injection schema
- Runtime requirement schema
- WebSocket / Event conventions
- Core SDK dependency
- a specific Web framework
- a specific HTTP server framework

## 10. Additional boundaries confirmed by the Echo comparison

The existing Spring Boot-based `manafield-echo` service was reviewed as a second, substantially different service case.

Echo is not yet registered against the current Module Protocol, so this **does not yet complete second real Module validation**.

It does strengthen several boundaries.

### Production Compose wiring is not the common Module contract

Echo's existing Compose file directly contains DB networking, reverse-proxy networking, Traefik labels, host port publishing, and a host secret mount.

Do not copy this production infrastructure wiring into generic Module Templates.

Prefer moving these responsibilities to:

```text
Instance Definition
Runtime Provider
Ingress Provider
Secret injection
```

If a Module repository includes Compose, prefer treating it as local-development convenience.

### Database / Storage is an optional requirement

Echo uses PostgreSQL/JPA, but that does not make a database universal.

Database/Storage needs follow [ADR-0010](adr/0010-capability-dependency-resolution.md) through the same **Capability Requirement** model. A Module that needs PostgreSQL requires `database.postgresql ^1`, while the Instance selects a concrete Resource providing that Capability and the connection binding.

### Secrets are not fixed into source or Templates

Echo needs a JWT public key and DB credentials.

An `.env.example` containing only variable names and placeholders can be useful, but Templates should not contain:

- a real `.env`
- real keys
- real passwords
- host-specific secret paths

### Authentication is not a generic Template implementation

Echo's JWT verifier and legacy Manafield API session validation are current integration details.

Do not make one authentication mechanism a universal Module Template default before the Manafield Identity / Permission model stabilizes.

### Framework routes are not automatically Operations

Echo already exposes many REST routes, but not every framework endpoint should automatically become a Manafield Operation.

An Operation is the **public callable contract Manafield can discover and invoke**.

The new Echo v2 design should validate:

- which framework endpoints become Operations
- how UI-internal APIs remain separate
- how Descriptor/route drift is prevented

Annotation/code generation remains a candidate, not a Template requirement.

### Good defaults visible for a future Java Template

- pinned Java toolchain
- Gradle Wrapper
- JDK build / JRE runtime multi-stage image
- environment-driven runtime port
- an application test task

Recommended additions:

- non-root runtime user
- Docker HEALTHCHECK

See [Echo Module Migration Review](echo-module-migration-review.md) for the detailed review.

## 11. Next validation

Do not migrate the existing Echo backend/deployment structure in place. Reuse only valuable CSS/JS/static frontend assets from the private Echo repository; design the Echo v2 backend and Module wiring from the current architecture.

Before creating the private Echo v2 repository, first make the Template Contract strong enough to describe:

```text
Module Template Contract
├─ unified Capability requirement / provider-source boundaries
├─ stateless Go Web Modules
├─ Official Go Web Shell
├─ Java / Spring Web Modules
└─ stateful Modules requiring the database.postgresql Capability
```

Then implement Echo v2 as the second real Module and use it to revalidate the Template Contract.

Implementation-specific template repositories may then be split when useful:

```text
manafield-module-template-go-web
manafield-module-template-ts-react
manafield-module-template-java-spring
```

No Template replaces the Manafield Protocol or makes its implementation language mandatory.
