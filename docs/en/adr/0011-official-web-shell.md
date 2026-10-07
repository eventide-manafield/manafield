# ADR-0011 — Official Web Shell and Same-Origin Module Composition

- Status: **Accepted**
- Date: 2026-10-07
- Amendment: ADR-0012 separates public-root Homepage ownership into a replaceable Module

## Context

Manafield keeps a Headless Core, but a real instance still benefits from a coherent Web experience such as `manafield.studio`.

Exposing every Web Module only through separate subdomains is technically simple but can fragment the user experience:

```text
account.manafield.studio
echo.manafield.studio
info.manafield.studio
...
```

At the opposite extreme, implementing every Module feature and API inside one central Web server recreates a monolithic application and destroys Module independence.

Making every Module depend on the official Web UI would also conflict with the Headless principle.

## Decision

### 1. Provide the Official Web Shell as a separate Module

The official Web View is implemented as a separate **`manafield-web` Module**.

It is not embedded in Core.

It may provide user-facing shell responsibilities such as:

- Shell-owned navigation / status surfaces
- global navigation
- Registry-driven Module discovery
- common layout / visual shell
- login / session entry UX
- navigation into Module Web surfaces

Core and other Modules must continue to function without `manafield-web`.

Ordinary Modules do not depend on `manafield-web` by default.

### 2. Compose multiple Modules under one public origin

An Instance may compose Web surfaces under one hostname using path routing.

Example:

```text
manafield.studio/                 → Homepage Module
manafield.studio/_manafield/*     → manafield-web
manafield.studio/account/*        → Account Module
manafield.studio/echo/*           → Echo Module
manafield.studio/...              → other Module
```

The browser sees one Website, while the actual backends remain independent Modules.

Public route mappings are not derived automatically from Module IDs or Operation paths.

Following ADR-0009, the Instance Definition and Ingress Provider select concrete route bindings.

### 3. Do not turn the Web Shell into the mandatory application-data proxy

`manafield-web` is not a mandatory gateway that proxies every Module API.

Where practical, Ingress routes public requests directly to the owning Module.

```text
Browser
   ↓
Ingress
   ├─ /              → Homepage Module
   ├─ /_manafield/*  → manafield-web
   ├─ /echo/*        → Echo
   ├─ /account/*     → Account
   └─ ...
```

This avoids making the Web Shell a bottleneck or a failure domain for every Module API and lets Modules retain their own HTTP semantics, streaming, and WebSocket behavior.

The Shell may read Core Registry APIs to build navigation, but it is not the required reverse proxy for ordinary Module traffic.

### 4. Separate Module Web surfaces from functional Module dependencies

A Module does not need to provide Web UI.

API-only and worker Modules remain valid.

Providing a Web UI also does not require the Module to depend on a `manafield.web-shell` Capability.

```text
Echo backend
→ works without Web Shell

manafield-web
→ may discover Echo's Web surface from Registry / Instance metadata
```

The Web Shell is an optional presentation layer, not a functional dependency of ordinary Modules.

### 5. Start with full-page/path surfaces

The initial implementation does not introduce dynamic micro-frontends, remote component runtimes, or arbitrary JavaScript injection.

The v0 composition unit is:

```text
Web Shell
→ provides navigation / common surfaces in its Instance-assigned shell namespace

Module Web Surface
→ owns its Instance-assigned path namespace, including an optional Homepage
```

Examples:

```text
/echo/*
/account/*
```

Dynamic loading of Module frontend bundles into the Shell process is deferred until a real need appears.

#### Web Surface kind

Public Web intent distinguishes at least `page` from `api`.

```text
page
→ user-facing surface
→ eligible for Web Shell navigation

api
→ public HTTP API
→ no navigation entry
```

HTTP endpoints, Operations, and public API surfaces are not automatically treated as the same concept.

#### Prefix preservation

Same-origin prefix routing preserves the prefix by default.

```text
public  /echo/foo
→ module /echo/foo
```

The Instance-assigned base path must be available to the Module through runtime configuration, and the Module uses the same base path for assets, redirects, cookie paths, and browser-side API paths.

See [Web Surface v0](../web-surface.md) for the metadata design.

### 6. Keep independent subdomain exposure

Same-origin composition is an option for an integrated Manafield UX, not a mandatory rule for every Web service.

Independent applications and infrastructure tools can continue to use ADR-0009 `host` exposure.

Examples:

```text
jenkins.manafield.studio
misskey.manafield.studio
reference.manafield.studio
```

Each Instance may select `host`, `prefix`, `routes`, or `external` based on Module needs.

### 7. Use Go for the first Official Web Shell implementation

The first `manafield-web` implementation starts as a **lightweight Go Web Module**.

Initial direction:

- single executable
- prefer standard-library `net/http`
- static assets embedded with `embed.FS`
- Core Registry reads
- application Health Operation
- runtime-configurable listen port
- non-root container
- minimal runtime image
- no Shell-owned database

React or a Node runtime is not required in production.

Node/Vite may still be used as a build-time tool if richer frontend assets become useful while keeping the production runtime as one Go binary.

The exact Go version and frontend implementation are pinned when the repository is bootstrapped.

## Initial routing shape

Conceptual example:

```text
Host: manafield.studio

/                 → Homepage Module
/_manafield/*     → manafield-web (candidate Shell namespace)
/account/*        → Account Web surface
/echo/*           → Echo Web surface
```

`/_manafield/*` is only a **candidate namespace** for Shell-owned assets/APIs and is not finalized as a reserved path by this ADR.

Root-path reservation and route conflicts are validated by the Build Plan Resolver.

## Authentication note

Same-origin composition can simplify login/session UX.

For example, an Account/Identity Module can authenticate under a path on the same origin without sending users between subdomains.

The exact cookie scope, token propagation, CSRF policy, and Identity Capability protocol are outside this ADR.

Identity dependencies follow the Capability model from ADR-0010.

## Consequences

### Benefits

- one coherent `manafield.studio` user experience
- preserves the Headless Core
- preserves Module runtime/implementation independence
- avoids rebuilding a giant backend monolith inside the Shell
- Module API traffic does not have to traverse the Shell process
- a Go single-binary Shell can remain lightweight
- integrated path applications and independent subdomain applications can coexist

### Costs

- Ingress Adapters must support same-host path composition
- the Build Plan Resolver must validate path conflicts and precedence
- path-mounted Modules must account for asset, redirect, and cookie semantics
- common UI consistency may later require style tokens or Web contribution rules

## Non-goals

This ADR does not finalize:

- a dynamic micro-frontend runtime
- arbitrary frontend bundle loading
- a shared JavaScript dependency runtime
- cookie/session protocol
- an Identity Module implementation
- the final reserved root-path set
- a policy forcing every Module into same-origin path exposure
