# Manafield Roadmap

> Roadmap for the current redesign.  
> Order and scope may change as the architecture is validated.

## Phase 0 — Foundation

- [ ] Rust Core bootstrap
- [ ] Core API
- [ ] Module state model
- [ ] Module Registry
- [ ] Manifest parser
- [ ] Module Protocol v0
- [ ] Reference Headless Module

## Phase 1 — Docker Runtime

- [ ] Docker Runtime Adapter
- [ ] Module container lifecycle
- [ ] Health checks
- [ ] Start / Stop / Restart
- [ ] Runtime status
- [ ] Basic log access

## Phase 2 — Official Web View

- [ ] Manafield Web
- [ ] Dynamic Module navigation
- [ ] Module Web Contribution
- [ ] Settings UI
- [ ] Module status UI

## Phase 3 — Reference Modules

- [ ] Observe
- [ ] ProtoDuck Integration
- [ ] Echo Service

## Intermediate Goal — Misskey

- [ ] **Run and manage Misskey as a Manafield Service Module**

This milestone should validate management of a realistic independent service with:

- an application runtime
- PostgreSQL
- Redis
- persistent storage
- networking
- configuration
- health checks
- Web access
- lifecycle management

If Misskey can be modeled cleanly without adding Misskey-specific behavior to Core, the Service Module abstraction is probably on the right track.

## Long-term

- [ ] Module package format
- [ ] Module install / update / remove
- [ ] Drag & Drop Module installation
- [ ] Permission review
- [ ] Dependency management
- [ ] Module Registry
- [ ] Compatibility Test Kit
- [ ] Third-party Module isolation
- [ ] Kubernetes Runtime Adapter

Kubernetes is intentionally a distant target. Docker is the primary runtime for early versions.
