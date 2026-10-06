# Manafield Roadmap

> 현재 redesign을 위한 Roadmap입니다.  
> Architecture를 검증하면서 순서와 범위가 변경될 수 있습니다.
>
> **한국어 문서를 기준 문서로 우선합니다.**  
> 영문판과 내용이 다를 경우 이 한국어판을 우선합니다.

## 진행 원칙

Manafield는 **큰 경계와 책임은 장기 구조를 기준으로 먼저 설계하고, 각 구성요소의 내부 구현은 현재 필요한 만큼만 채우는 방식**으로 개발합니다.

- 나중에 분리하기 비싼 경계는 지금 정의합니다.
- 아직 필요하지 않은 기능은 빈 인터페이스나 최소 구현으로 남길 수 있습니다.
- Core는 특정 Runtime이나 UI가 없어도 동작할 수 있어야 합니다.
- 강한 권한은 가능한 한 작은 구성요소에만 부여합니다.
- 주요 Architecture 결정은 ADR로 기록합니다.

ADR 목록: [Architecture Decision Records](adr/README.md)

## Phase 0 — Core Foundation

- [x] Rust Core bootstrap
- [x] Rust 1.99.0 toolchain pinning
- [x] Core HTTP API bootstrap
- [x] Core Dockerfile
- [x] Module Descriptor 모델
- [x] Operation Contract 모델
- [x] DataSchema 모델
- [x] Binding / Codec 분리
- [x] JSON / MessagePack 직렬화
- [x] Health Operation 참조
- [x] Module Manifest discovery
- [x] Module validation
- [x] Runtime Module Registry API
- [x] Immutable RegistrySnapshot
- [x] ArcSwap 기반 lock-free read model
- [ ] Module Protocol v0 문서 안정화
- [ ] Registry update semantics 정의

## Phase 0.5 — Reference Module

- [x] 독립 Repository 생성: `manafield-module-reference`
- [x] Node.js / TypeScript Module server
- [x] React Registry Observer
- [x] `/manafield/health` Operation
- [x] Core Registry 조회 연결
- [x] Dockerfile
- [x] Core + Reference Module Compose 환경
- [ ] Reference Module을 실제 Core Registry에 등록
- [ ] Observer에서 자기 자신 관찰
- [ ] Reference 구현에서 Module Template 요구사항 추출

## Phase 1 — Runtime Provider Boundary

Runtime은 일반 Module과 구분되는 **privileged system component**로 취급합니다.

- [x] Runtime Provider를 일반 Module과 분리하는 Architecture 결정
- [x] Core가 특정 Runtime에 종속되지 않아야 한다는 원칙 확정
- [x] Docker 권한을 Core와 분리하는 Process boundary 방향 확정
- [ ] Runtime Protocol v0 정의
- [ ] Runtime request / response 타입 정의
- [ ] Runtime Provider identity / capability 모델
- [ ] Local Runtime transport 결정 및 구현
  - [ ] Unix Domain Socket 기반 초기 통신 검토/구현
- [ ] Core Runtime client
- [ ] Runtime Provider health / availability
- [ ] Runtime Provider policy model

### Docker Runtime Provider

Docker Provider는 Core와 같은 Repository에서 관리하되 **별도 Binary / Process**로 빌드하는 방향을 우선합니다.

- [ ] Cargo workspace / crate 구조 정리
- [ ] `manafield` Core binary
- [ ] `manafield-runtime-docker` binary
- [ ] Docker API client
- [ ] Docker socket은 Runtime Provider에만 mount
- [ ] Core container에는 Docker socket을 mount하지 않음
- [ ] Create / Start / Stop / Remove / Status
- [ ] Docker network 연결
- [ ] Resource 제한
- [ ] privileged container 금지 정책
- [ ] 임의 host path mount 제한
- [ ] Docker socket 재노출 금지
- [ ] Runtime logs

## Phase 2 — Module Lifecycle

- [ ] Module install model
- [ ] Runtime requirement / capability 선언
- [ ] Runtime Provider 선택
- [ ] Module create
- [ ] Start / Stop / Restart
- [ ] Runtime status
- [ ] Health Operation 실제 호출
- [ ] Health 상태 모델
- [ ] Registry와 Runtime state 연결
- [ ] 실패 / 재시도 정책
- [ ] 제거 시 Runtime 정리

## Phase 3 — Official Web View

- [ ] Manafield Web
- [ ] Dynamic Module navigation
- [ ] Registry / Operation browser
- [ ] Runtime Provider 상태 UI
- [ ] Module Health UI
- [ ] Module Web Contribution
- [ ] Settings UI

## Phase 4 — Reference / Existing Services

- [ ] Observe
- [ ] ProtoDuck Integration
- [ ] Echo Service
- [ ] 기존 외부 Service를 Runtime 없이 등록하는 흐름 검증

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

Misskey 전용 로직을 Core에 추가하지 않고도 Misskey를 자연스럽게 모델링할 수 있다면 Service Module abstraction과 Runtime Provider 경계가 올바른 방향에 있다고 판단할 수 있습니다.

## Architecture Decision Records

- [x] ADR-0001 — Rust 기반 Headless Core
- [x] ADR-0002 — Operation Contract
- [x] ADR-0003 — Registry Snapshot / ArcSwap Read Model
- [x] ADR-0004 — Runtime Provider Boundary
- [x] ADR-0005 — Docker Runtime Provider Process Isolation
- [x] ADR-0006 — Instance Build Plan / 인스턴스별 단일 Pipeline
- [x] ADR-0007 — Public Platform / Private Instance Configuration
- [ ] Runtime Protocol이 구체화되면 후속 ADR 추가
- [ ] Module package format이 구체화되면 후속 ADR 추가
- [ ] Permission / trust model이 구체화되면 후속 ADR 추가

## Phase 5 — CI/CD / Instance Build Plan

- [x] 인스턴스별 단일 Pipeline 원칙 확정
- [x] Module별 Jenkins Job을 만들지 않는 방향 확정
- [x] Module build contract를 Repository root Dockerfile로 시작
- [x] Instance Definition v0
- [x] 공개 플랫폼과 Private Instance Configuration 분리 원칙
- [x] Build Plan Resolver v0
- [ ] Jenkins 단일 Pipeline skeleton
- [ ] GitHub webhook → 동일 Instance Pipeline 연결
- [ ] 변경 source 기반 selective build
- [ ] Core / Runtime Provider / Module image tagging
- [ ] Deploy 후 Health / Protocol verification
- [ ] CI credential store 기반 Secret 주입
- [ ] Jenkins 외 executor에서도 Build Plan 재사용 가능한 구조 검증

## Long-term

- [ ] Module package format
- [ ] Module install / update / remove
- [ ] Drag & Drop Module installation
- [ ] Permission review
- [ ] Dependency management
- [ ] Module Registry distribution / discovery
- [ ] Compatibility Test Kit
- [ ] Third-party Module isolation
- [ ] External Runtime Provider protocol
- [ ] Remote Runtime Provider
- [ ] Kubernetes Runtime Provider

Kubernetes는 의도적으로 먼 목표로 둡니다. Docker는 초기 Runtime Provider 구현이며, Manafield Core 자체의 필수 의존성은 아닙니다.
