# ADR-0009 — Separate Operation Routing from Web Exposure

- Status: Accepted
- Date: 2026-10-07

## Context

A Manafield Operation is the common contract for callable functionality exposed by a Module.

How a Module exposes a Web UI or HTTP endpoint to a browser is instead an instance-level exposure policy.

Treating these as one concept would create several problems:

- Operation names could become global URL paths and collide across Modules
- Web applications could be forced to operate under path prefixes
- Module capabilities would become coupled to an instance-specific hostname / path policy
- changing external exposure could require changing the Module Protocol itself

## Decision

Operation routing and Web exposure remain separate contracts.

### Operation

An Operation is identified inside a Module namespace.

Two different Modules may both define an Operation named `generate` without conflict.

A public URL such as `/generate` is never created automatically from an Operation ID.

### Web Exposure

Web exposure is selected by instance configuration.

A Module may declare that it provides a Web surface, but the Instance Definition selects the actual public hostname or path.

Initial exposure forms include:

- `none`: no external Web exposure
- `host`: reverse proxy through a dedicated hostname
- `prefix`: one path namespace owned by a Module
- `routes`: selected root-level path claims
- `external`: link to an already-existing external URL

### Host Proxy

Recommended for normal self-contained Web Modules.

Example:

```text
reference.manafield.studio
```

The Module can operate from `/`, reducing path-prefix constraints on assets, redirects, cookies, and WebSockets.

### Prefix Proxy

Used explicitly when path-based exposure is appropriate.

Example:

```text
/modules/example/...
```

The public prefix is preserved by default rather than stripped.

```text
public  /echo/foo
→ module /echo/foo
```

The Instance-assigned prefix/base path must be available to the Module as runtime configuration so assets, redirects, cookie paths, and browser-side API paths use the same route base.

Prefix Proxy is not the default exposure mode for every Module.

### Root Route Claims

Special Modules may claim global paths without a Module namespace.

Examples:

```text
/healthz
/search
/hook
```

These paths are treated as scarce global URL-space resources.

The public path and the Module-internal target path are configured independently.

```yaml
routes:
  - match: exact
    public: /search
    target: /api/search
```

### Conflict Detection

The Build Plan Resolver checks public route claims before deployment.

At minimum, the following are conflicts:

- duplicate `host` catch-all ownership of the same hostname
- duplicate exact-path claims
- duplicate normalized prefix claims on the same hostname
- collisions with Manafield-reserved root paths

Nested but distinct prefixes are not automatically conflicts. For example, a `/` Homepage and a `/social` Module may coexist when the Ingress Adapter assigns deterministic priority to the more specific prefix.

A conflict fails Build Plan generation instead of continuing deployment.

Diagnostics must identify both the conflicting path and the Modules that claimed it.

Example:

```text
Route conflict: /search

claimed by:
  module-a
  module-b

Choose a different public path or isolate one Module behind a prefix or host.
```

### Reserved Paths

Manafield may reserve root namespaces for its own public surface.

Examples:

```text
/api/*
/.well-known/*
```

The concrete reserved set will be finalized when the public API structure stabilizes.

## Reason

- keep Operation Contracts independent from Web routing technology
- separate Module / Operation namespaces from the global URL namespace
- use host proxying for self-contained Web applications to reduce implementation constraints
- allow special Modules to claim root-level routes when needed
- detect conflicts before runtime
- keep public URL policy in private Instance Configuration

## Consequences

- public URLs are not determined by the Module manifest alone
- the Instance Definition and Build Plan gain a Web exposure model
- the Build Plan Resolver gains route normalization and conflict detection
- ingress configuration must be generated or applied from the resolved Build Plan
- root-level route claims remain an exceptional mechanism
- Operation APIs and Web proxy URLs may evolve under different lifecycle and compatibility policies
