# Web Surface v0

> Initial Web Surface design connecting Module Web Exposure with the Manafield Web Shell.
>
> Public URL ownership follows ADR-0009, while the Official Web Shell architecture follows ADR-0011 and ADR-0012.

## 1. Purpose

Having an HTTP server does not tell Manafield whether a Module provides:

- a user-facing page
- a public API
- both
- or only internal HTTP endpoints

Web exposure metadata therefore distinguishes **pages from API surfaces**.

## 2. Surface kind

Initial Web Surface kinds:

```text
page
→ user-facing surface
→ eligible for Web Shell navigation

api
→ externally exposed HTTP API
→ does not create a navigation entry
```

A Module may provide both.

Example:

```yaml
web:
  surfaces:
    - id: page
      kind: page
      title: Echo
      description: A place for short messages and observations

    - id: api
      kind: api
      description: Echo public HTTP API
```

Exact manifest field names may be refined during implementation, while the page/API semantic distinction remains.

## 3. Operations and public Web APIs are different concepts

An `api` Web Surface expresses public HTTP exposure intent.

An Operation is a protocol contract Manafield can discover/invoke.

Therefore:

```text
HTTP endpoint
≠ automatically an Operation
≠ automatically a public API
```

A Module may distinguish internal endpoints, Operations, and public API surfaces.

## 4. Public paths belong to the Instance

A Module does not authoritatively own a public path such as `/echo`.

The Instance selects the concrete path binding.

Conceptual example:

```yaml
exposure:
  type: prefix
  host: manafield.studio
  prefix: /echo
  targetPort: 8080
```

A Module may provide presentation metadata such as title/description/icon, while public paths remain Instance policy.

## 5. Prefixes are preserved

Default same-origin prefix routing **does not strip the public prefix**.

```text
public request
GET /echo/foo

module receives
GET /echo/foo
```

It does not become:

```text
GET /echo/foo
→ GET /foo   ❌
```

This keeps route semantics consistent between development and deployment.

## 6. Module base path

Because public prefixes are Instance-owned, the assigned base path must be available to the Module as runtime configuration.

Initial container convention candidate:

```text
MANAFIELD_WEB_BASE_PATH=/echo
```

The final injection schema belongs to Runtime/Build Plan design.

Modules use the base path consistently for:

- handler routes
- static asset URLs
- redirect targets
- cookie Path
- browser-side API URLs

## 7. Homepage and Shell

An integrated Web Instance may separate Homepage content from the Shell.

```text
/                 → Homepage Module
/_manafield/*     → manafield-web
/account/*        → Account page/api surfaces
/echo/*           → Echo page/api surfaces
```

The Homepage Module may be replaced or omitted.

`manafield-web` does not proxy Homepage or other Module routes.

Ingress sends same-origin routes directly to the owning Module. When `/` and a more specific prefix share the same hostname, the more specific prefix wins.

## 8. Navigation

The Web Shell considers only `kind: page` surfaces for navigation.

`kind: api` surfaces do not create navigation items.

Candidate page metadata:

- title
- description
- icon
- navigation group
- navigation order

This is presentation metadata and is unrelated to functional Capability matching.

## 9. API-only Modules

A Module may provide a public API without a page.

```text
Module
├─ no page surface
└─ api surface
```

A Module with only internal Operations and no public Web exposure is also valid.

## 10. Deferred details

- final manifest JSON/YAML field names
- navigation for Modules with multiple page surfaces
- icon format
- WebSocket-specific metadata
- CORS policy
- cookie/session protocol
- final reserved root-path list
