# Echo Module Migration Review

> Review of the existing private `manafield-echo` service before adapting it to the current Manafield Module architecture.
>
> This is not an implementation plan to immediately rewrite Echo. It records what a second, substantially different service teaches us about Module Template boundaries.

## 1. Current Echo shape

Echo is currently an independent Spring Boot service with:

- Java 21
- Spring Boot 4.1
- Gradle Wrapper
- Spring MVC / Validation / Security
- PostgreSQL + JPA
- JWT authentication
- scheduled jobs
- its own REST API
- its own static Web UI

Unlike the Reference Module, Echo has a database, secrets, authorization, background work, and a Web surface.

This makes it a useful Template validation case, but it is not yet adapted to the current Module Protocol.

## 2. Differences from the current Module Protocol

Echo currently has no:

- `manafield.module.json`
- `healthOperation`
- declared health Operation such as `/manafield/health`
- Build Plan-based exposure declaration

Its current `docker-compose.yml` directly knows about:

- an external DB network
- an external `proxy` network
- Traefik labels
- host port publishing
- a JWT public-key host mount

Those are legacy deployment-wiring responsibilities. In the current architecture, most of them belong to Instance / Runtime / Ingress Provider layers.

## 3. Common Template lessons

Reference and Echo use very different stacks, yet the shared requirements are small.

### Strong common contract

- Module identity
- Module version
- Operation contracts
- health visibility
- an independent build contract
- a runtime listen port
- a rule that secrets do not live in source

This reinforces the current direction that the Module Protocol, not a framework template, is the common center.

### Language-specific build tooling remains template-specific

Reference:

```text
Node.js / npm / TypeScript / Vite
```

Echo:

```text
Java / Gradle / Spring Boot
```

Therefore npm scripts, Gradle Wrapper, React, or Spring annotations are not universal Module requirements.

## 4. Echo-specific elements that should not become generic Template requirements

### Database wiring

Echo directly consumes PostgreSQL connection settings.

Database-backed Modules are valid, but databases are not universal.

Do not standardize PostgreSQL, JPA, DB network names, or Echo-specific DB environment variables in the generic Template.

A future Module package/runtime requirement model can express database/storage dependencies.

### Authentication coupling

Echo currently verifies JWTs itself and calls a legacy Manafield API session-validation endpoint.

Its Web frontend also contains direct references to `https://manafield.studio`.

These are legacy integration details, not generic Template defaults.

When the Identity / Permission model is defined, Echo authentication should be adapted again.

### Traefik / proxy wiring

Traefik labels and the `proxy` network in Echo's Compose file should not survive as Template requirements.

The new responsibility is:

```text
Instance Definition
→ Build Plan
→ Ingress Provider
```

### Host secret mount

Directly mounting a JWT public key from a host path is not a generic Template requirement.

Secret requirements and injection belong to Instance / Runtime layers.

## 5. Good defaults confirmed by Echo

### Gradle Wrapper

Pinning the build tool through the repository improves reproducibility.

A future Java Template should normally include the Wrapper.

### Environment-driven application port

Echo can override its application port using `ECHO_APP_PORT`.

This confirms the value of a runtime-configurable listen port.

A future cross-language convention should consider a neutral name such as `PORT`.

### Multi-stage Docker build

Separating JDK build and JRE runtime images is a useful default.

The current Echo Dockerfile should additionally gain:

- a non-root runtime user
- Docker HEALTHCHECK

## 6. Minimum work to adapt Echo as a current Manafield Module

### Protocol

- add `manafield.module.json`
- add a Health Operation
- represent selected callable APIs as Operation Contracts
- pass Descriptor validation

### Container

- run as non-root
- add Docker HEALTHCHECK
- remove host port publication from production deployment
- remove production Traefik labels from the Module repository

### Instance Definition

Use the Instance Definition for:

- source / version
- build contract
- Web exposure
- DB / Secret injection using the currently available deployment mechanisms

### Legacy integration

Existing JWT / direct Manafield API calls may remain temporarily as a migration bridge, but they must not become Template requirements.

## 7. Operation Contract observation

Echo already exposes many HTTP endpoints, for example:

```text
GET  /api/echoes
POST /api/echoes
POST /api/echoes/{echoId}/replies
POST /api/echoes/{echoId}/resonate
GET  /api/admin/echoes
...
```

Treating every framework route automatically as a Manafield Operation may be inappropriate.

An Operation is not just a list of HTTP routes; it is the **public callable contract Manafield can discover and invoke**.

Echo migration should validate:

- which HTTP endpoints should become Operations
- how UI-internal endpoints differ from Operations
- how to prevent drift between the Descriptor and framework routes

Annotation/code generation is not yet a generic Template requirement.

## 8. Conclusions for Template requirements

Echo strengthens these boundaries:

- Web framework is not a common requirement
- Database/Storage is an optional runtime requirement
- Authentication implementation belongs to Module-specific or future Permission-model concerns
- public exposure belongs to Instance/Ingress
- secret mounts belong to Runtime/Instance
- Docker build contract and health visibility have strong commonality
- language-specific Templates provide build/runtime convenience but do not replace the Protocol

## 9. Status

The Echo analysis is complete, but **the second real Module validation is not yet considered complete**.

Only after Echo is actually adapted to the current Module Protocol and runs in Core Registry should the roadmap item:

```text
Revalidate Template requirements with a second real Module
```

be marked complete.
