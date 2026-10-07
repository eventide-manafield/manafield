# ADR-0012 — Replaceable Homepage Module and Web Shell Separation

- Status: **Accepted**
- Date: 2026-10-07
- Related: ADR-0011 — Official Web Shell and Same-Origin Module Composition

## Context

The initial ADR-0011 implementation allowed `manafield-web` to act as both the Web Shell and the Instance home at public root `/`.

If Manafield's Web experience is itself composed from Modules, homepage content should not be fixed inside the Shell implementation.

An Instance should be able to compose pages such as:

```text
/              → Homepage Module
/sample/*      → Sample Module
/social/*      → Social Module
/account/*     → Account Module
```

Keeping Homepage content inside `manafield-web` would require modifying the Official Web Shell whenever an Instance wants to replace or remove its Homepage and would encourage the Shell to grow into a central Web application.

## Decision

### 1. Homepage is an independent Module

Content at public root `/` may be owned by a separate Homepage Module.

The initial official example is `manafield-home`.

```text
manafield-home
→ replaceable page Module
→ the Instance binds it to "/" when desired
```

An Instance may use another Homepage Module or no Homepage at all.

### 2. `manafield-web` focuses on Shell responsibilities

The Shell may provide shared presentation responsibilities such as:

- Web surface discovery
- navigation
- common shell / chrome
- login / session entry UX
- Registry-driven status and management surfaces

Owning the public root Homepage is not a required Shell responsibility.

A composed Instance may place the Shell itself under a dedicated prefix such as:

```text
/_manafield/*
```

The exact reserved namespace may change later, but Homepage and Shell route ownership remain separate.

### 3. Nested prefixes on the same hostname are allowed

A Homepage may own `/` while other Modules own more specific prefixes.

```text
/              → home
/_manafield/*  → shell
/social/*      → social
```

Distinct prefixes on the same hostname are therefore not automatically conflicts.

Duplicate claims for the same normalized prefix are conflicts.

Ingress adapters must generate deterministic priority so more specific prefixes win over root or parent prefixes.

### 4. Prefixes are preserved

The existing Web Surface rule remains unchanged.

```text
public /_manafield/foo
→ module /_manafield/foo
```

The Instance-assigned base path may be injected into the Module at runtime.

Initial container convention:

```text
MANAFIELD_WEB_BASE_PATH=/_manafield
```

### 5. Ingress routes directly to each Module

Homepage traffic does not pass through the Shell as a mandatory proxy.

```text
Browser
  ↓
Ingress
  ├─ /              → Homepage Module
  ├─ /_manafield/*  → Web Shell
  ├─ /sample/*      → Sample Module
  └─ /social/*      → Social Module
```

Each page Module remains an independent process/container.

## Consequences

### Benefits

- Homepage can be installed, replaced, or removed
- Prevents the Web Shell from growing into a monolithic homepage application
- Sample, Social, Account, and other page Modules use the same composition path
- Preserves Headless Instance behavior
- Preserves process boundaries while presenting one origin

### Costs

- Build Plan must resolve nested prefixes on the same host deterministically
- Modules running below a prefix must correctly support their assigned base path
- Shared visual language between Shell and Homepage may need a later style/token contract

## Relationship to ADR-0011

This ADR changes the initial ADR-0011 root ownership example:

```text
Before:
  / → manafield-web

Now:
  /             → replaceable Homepage Module
  /_manafield/* → manafield-web (one possible binding)
```

The remaining ADR-0011 principles stay in effect: the Official Web Shell remains a separate Module, same-origin path composition remains supported, and Ingress routes Module traffic directly instead of forcing it through the Shell.
