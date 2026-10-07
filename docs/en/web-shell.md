# Official Web Shell v0 Design

> Initial implementation notes for turning ADR-0011 and ADR-0012 into the real Web Shell.
>
> This is still pre-implementation design and may change when the `manafield-web` repository is bootstrapped.

## 1. Responsibilities

`manafield-web` is the official Manafield Web presentation layer.

v0 responsibilities:

- Core Registry reads
- Module listing with description/version
- navigation into Module Web surfaces
- common layout / visual shell
- a Shell surface below the Instance-assigned base path
- `/manafield/health` Operation

Homepage content at public root `/` may be owned by a separate Homepage Module. The initial official example is `manafield-home`.

v0 does not:

- reverse proxy every Module API
- manage Module lifecycle
- access Docker
- access databases
- dynamically load micro-frontends
- inject arbitrary Module JavaScript

## 2. Runtime

The first implementation uses Go.

```text
manafield-web
├─ Go HTTP server
├─ embedded static assets
├─ Core Registry client
└─ health endpoint
```

Initial implementation preferences:

- Go standard-library `net/http`
- `html/template` or a very thin static frontend
- `embed.FS`
- JSON Core API client
- Docker multi-stage build
- minimal non-root runtime image

Pin the current stable Go release when the repository is created.

## 3. Core connection

Initially, inject the Core endpoint through an environment variable, similar to the Reference Module.

```text
MANAFIELD_CORE_URL=http://core:8080
```

The Shell reads Module metadata from Core Registry.

Minimum APIs:

```http
GET /modules
GET /modules/{id}
```

A temporary Registry failure does not need to terminate the Shell process.

The UI should be able to show a degraded Core-unavailable state.

## 4. Public routing

Integrated Instance example:

```text
Host: manafield.studio

/                 → manafield-home
/_manafield/*     → manafield-web
/account/*        → account Module
/echo/*           → echo Module
```

The Shell process does not proxy `/account/*` or `/echo/*`.

The Traefik Adapter renders same-host routes from the resolved Build Plan.

Shell deployment and Module-route deployment therefore remain separate.

## 5. Route ownership

The Shell does not need to own public root `/`.

A separate Homepage Module may claim `/`, while the Shell runs under an Instance-assigned prefix.

Current composition example:

```text
/              → manafield-home
/_manafield/*  → manafield-web
```

Whether `/_manafield/*` becomes the final reserved namespace remains a later policy decision.

Module routes are explicitly bound in the Instance Definition.

Conceptual example:

```yaml
modules:
  - id: manafield-echo
    exposure:
      type: prefix
      host: manafield.studio
      prefix: /echo
      targetPort: 8080
```

The final schema belongs to the Build Plan prefix/routes design.

Prefixes are not stripped.

```text
/echo/foo
→ Echo also receives /echo/foo
```

The Instance-assigned base path may initially be injected with a container convention such as `MANAFIELD_WEB_BASE_PATH=/echo`. Modules use it consistently for assets, redirects, cookie paths, and browser-side API URLs.

## 6. Navigation metadata

v0 primarily uses Registry metadata:

- Module `id`
- `name`
- optional `description`
- `version`

Additional Web navigation metadata remains separate from functional dependency contracts.

Web Surface metadata first distinguishes `page` from `api`.

```text
page
→ navigable user-facing surface

api
→ public HTTP API, no navigation entry
```

Candidate page metadata:

```text
title
description
icon
navigation order/group
```

Public paths remain Instance-owned; Module metadata does not own an authoritative public path.

See [Web Surface v0](web-surface.md) for the detailed design.

## 7. Authentication

The v0 Shell does not embed an Identity implementation.

Future login/session UX connects to a Module providing the `manafield.identity` Capability.

```text
manafield-web
→ login entry UX

Identity Module
→ authentication/session contract

Other Modules
→ require manafield.identity when needed
```

Identity and dependent Modules must remain functional in a Headless Instance without the Web Shell.

## 8. Failure boundaries

Failures remain separated.

```text
manafield-web down
→ Shell unavailable
→ Homepage and other Module surfaces may remain reachable

Echo down
→ /echo unavailable
→ Shell and other Modules remain available

Core Registry temporarily unavailable
→ Shell shows degraded discovery state
→ already-routed Module traffic can continue
```

This is why Module traffic does not have to traverse the Shell process.

## 9. Initial repository shape

Candidate:

```text
manafield-web/
├─ cmd/
│  └─ manafield-web/
│     └─ main.go
├─ internal/
│  ├─ coreclient/
│  ├─ web/
│  └─ view/
├─ web/
│  ├─ static/
│  └─ templates/
├─ manafield.module.json
├─ Dockerfile
├─ go.mod
├─ go.sum
└─ README.md
```

Avoid premature package splitting; keep only the directories justified by real code.

## 10. First implementation milestone

The current milestone validates:

```text
manafield-web starts
→ /manafield/health = OK
→ Core Registry read succeeds
→ Shell page works below the Instance-assigned base path
→ MANAFIELD_WEB_BASE_PATH is reflected in handlers/assets
→ no DB
→ no Docker privilege
```

Homepage content lives in a separate `manafield-home` Module and Build Plan / Ingress compose routes such as:

```text
manafield.studio/             → manafield-home
manafield.studio/_manafield/  → manafield-web
manafield.studio/echo/        → Echo
```
