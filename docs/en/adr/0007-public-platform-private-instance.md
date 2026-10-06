# ADR-0007 — Public Platform, Private Instance Configuration

- Status: Accepted
- Date: 2026-10-07

## Context

Manafield itself may be distributed as public software, while a real user instance can contain personal configuration, private Modules, private Integrations, and internal service addresses that should not be published.

Keeping the actual production instance configuration inside the Core repository would blur the boundary between platform code and private deployment state.

## Decision

The **Manafield platform, protocol, and official implementations may be public**, while **real Manafield Instance Configuration is private by default** and managed separately.

```text
Public
├─ Manafield Core
├─ Runtime Protocol
├─ official Runtime Providers
├─ reference Modules
└─ public Modules

Private by default
├─ actual instance.yaml
├─ enabled Module list
├─ private Module sources
├─ private Integrations
└─ deployment-specific configuration
```

The repository's `deploy/instance.example.yaml` is a public example only and is not the storage location for a real production instance definition.

Personal Modules and Integrations may be either public or private as long as they implement the Manafield Protocol.

Secrets are not stored directly even in a private Instance Definition. They should be supplied through a CI credential store, runtime secret store, or another environment-specific secret injection mechanism.

## Reason

- separates public platform code from private deployment state
- allows private Modules / Integrations without requiring publication
- avoids treating Core source visibility as a security boundary
- keeps secrets outside general instance configuration
- allows the same public Manafield platform to host completely different private instances

## Consequences

- production `instance.yaml` files are not committed to the Core repository
- public repositories contain examples, schemas, and documentation only
- CI executors such as Jenkins use separate credentials when private Instance Configuration or private Module repositories are involved
- Build Plan resolution must treat public and private sources through the same abstraction
- the concrete storage model for private Instance Configuration may later become a separate private repository or server-side configuration
