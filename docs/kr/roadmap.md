# Manafield Roadmap

> 현재 redesign을 위한 Roadmap입니다.  
> Architecture를 검증하면서 순서와 범위가 변경될 수 있습니다.
>
> **한국어 문서를 기준 문서로 우선합니다.**  
> 영문판과 내용이 다를 경우 이 한국어판을 우선합니다.

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
- [ ] Health check
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

- [ ] **Misskey를 Manafield Service Module로 실행하고 관리하기**

이 목표에서는 다음 요소를 가진 실제 독립 서비스를 Manafield가 관리할 수 있는지 검증합니다.

- Application runtime
- PostgreSQL
- Redis
- Persistent storage
- Networking
- Configuration
- Health check
- Web access
- Lifecycle management

Misskey 전용 로직을 Core에 추가하지 않고도 Misskey를 자연스럽게 모델링할 수 있다면 Service Module abstraction이 올바른 방향에 있다고 판단할 수 있습니다.

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

Kubernetes는 의도적으로 먼 목표로 둡니다. 초기 버전의 주 Runtime은 Docker입니다.
