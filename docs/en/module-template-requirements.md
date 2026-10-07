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

## 6. TypeScript / React Template v0 candidate

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
- Permission / Capability declarations
- Storage conventions
- Dependency declarations
- Runtime requirement schema
- WebSocket / Event conventions
- Core SDK dependency
- a specific Web framework
- a specific HTTP server framework

## 10. Next validation

Do not freeze a TS/React Template repository from a single implementation yet.

Implement a **second real Module**, compare it with this document, and extract only the stable shared pieces into:

```text
manafield-module-template-ts-react
```

The Template should be a minimal runnable example with Reference-specific behavior removed.
