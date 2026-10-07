# ADR-0008 — Manafield Network Planes

- Status: Accepted
- Date: 2026-10-07

## Context

Internal communication between Manafield Core and Modules has a different trust boundary from communication with external ingress infrastructure such as Traefik and Cloudflared.

A generic network name such as `proxy` does not describe this boundary and can encourage directly attaching Core to externally facing infrastructure.

## Decision

Manafield deployments distinguish two network planes:

- `manafield-modules`: internal communication between Core and Modules
- `manafield-edge`: shared edge network for externally exposed services and ingress infrastructure

Core connects only to `manafield-modules` by default.

A Module that provides an externally exposed Web surface may connect to both networks when needed.

```text
Internet
   |
Cloudflare / Traefik
   |
manafield-edge
   |
Manafield Reference
   |
manafield-modules
   |
Manafield Core
```

`manafield-edge` is treated as an external Docker network owned by host infrastructure rather than an individual Manafield Compose deployment.

## Reason

- separates internal Module communication from the external ingress boundary
- prevents direct Core exposure by default
- makes network purpose obvious from naming
- allows Traefik / Cloudflared / Jenkins and exposed Modules to share one edge network
- gives future Runtime Providers a stable internal network target for Modules

## Consequences

- Modules that do not need external exposure do not join `manafield-edge`
- host infrastructure must create `manafield-edge` before deployment
- the generic `proxy` network name is no longer used
- outbound Internet access is separate from membership in `manafield-edge`
