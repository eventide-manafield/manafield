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
- [x] Built-in Headless CLI v0 — `health` / `ps` / `resource`
- [ ] Module CLI contribution contract
- [ ] Module Protocol v0 문서 안정화
- [ ] Registry update semantics 정의

## Phase 0.5 — Manafield Reference

- [x] 독립 Repository 생성: `manafield-module-reference`
- [x] Node.js / TypeScript Module server
- [x] React Registry Observer
- [x] `/manafield/health` Operation
- [x] Core Registry 조회 연결
- [x] Dockerfile
- [x] Core + Manafield Reference Compose 환경
- [x] Manafield Reference를 실제 Core Registry에 등록
- [x] Observer에서 자기 자신 관찰
- [x] Reference 구현에서 Module Template 요구사항 추출 → [Module Template Requirements v0](module-template-requirements.md)
- [x] 기존 Echo 서비스와 Template 경계 비교 검토 → [Echo Module Migration Review](echo-module-migration-review.md)
- [ ] 공통 Module Template Contract 정리
- [ ] Go Web implementation profile 정리
- [ ] TS / React Web implementation profile 정리
- [ ] Java / Spring Web implementation profile 정리
- [ ] `database.postgresql` Capability를 요구하는 stateful Web Module 설계 가능 상태 검증
- [ ] 새 private Echo v2 Repository 생성 / 구현
- [ ] Echo v2를 실제 Core Registry에 등록
- [ ] 두 번째 실제 Module에서 Template Contract 재검증
- [ ] 필요 시 구현 Profile별 Template Repository 추출

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

## Phase 1.5 — Capability Resolution / Instance Binding

- [x] Module identity와 dependency contract 분리 원칙 확정
- [x] Tag와 Capability 역할 분리
- [x] 모든 일반 requirement를 Capability Contract로 통일
- [x] ADR-0010 — Capability 기반 Dependency Resolution
- [x] Module / Operation optional `description` metadata
- [x] Capability Contract v0 설계 → [Capability Contract](capability.md)
- [x] Capability matching surface를 `id + version`으로 제한
- [x] Module Instance / Resource Instance 공통 Capability registry metadata 원칙
- [x] Instance ID unique / duplicate conflict 원칙
- [x] Binding = Requirement slot → target Instance ID
- [x] Binding state = `UNBOUND` / `BOUND`
- [x] Resolver/Discovery 자동 Binding 금지
- [x] Discovery 3단계: compatible / advanced same-ID / full
- [x] Capability version mismatch = warning + log + continue
- [x] endpoint/config value는 Instance, config schema는 Definition에 배치
- [ ] `provides.capabilities` / `requires.capabilities` Core 모델 구현
- [ ] Capability ↔ Operation contract validation
- [x] Capability SemVer / range 정책
- [ ] concrete Binding 저장/조회 구현
- [ ] Capability Discovery API 구현
- [ ] Binding validation / diagnostic error model
- [ ] dependency graph / cycle validation
- [x] Resource Instance Capability metadata / generic Registry API v0
- [x] Module / Resource 공통 Instance ID namespace / duplicate conflict
- [x] PostgreSQL Resource Provider registration/watch 예제
- [ ] Resource 관리 컴포넌트 lifecycle / protocol
- [ ] Secret / connection metadata injection model
- [ ] PostgreSQL Resource 관리 구현 v0 (`database.postgresql` Capability)
- [ ] 동일 PostgreSQL instance의 Module별 database/schema/account allocation / Capability binding

## Phase 2 — Module Lifecycle

- [ ] Module install model
- [ ] Runtime requirement 선언
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

- [x] Official Web Shell을 Core와 분리하는 원칙 확정
- [x] Same-Origin path composition 방향 확정
- [x] ADR-0011 — Official Web Shell / Same-Origin Module Composition
- [ ] 독립 Repository 생성: `manafield-web`
- [ ] Go single-binary Web Shell bootstrap
- [ ] `/manafield/health` Operation
- [ ] Core Registry 조회
- [ ] Instance home / navigation
- [ ] Registry 기반 Dynamic Module navigation
- [ ] Registry / Operation browser
- [ ] Runtime Provider 상태 UI
- [ ] Module Health UI
- [x] Web Surface metadata v0 (`page / api`) → [Web Surface v0](web-surface.md)
- [ ] Web Exposure 모델 (`none / host / prefix / routes / external`)
- [ ] Build Plan same-host prefix / route normalization (prefix preserve)
- [ ] Build Plan route conflict / precedence validation
- [ ] Traefik same-host path composition 렌더링 (no prefix strip)
- [ ] Manafield reserved root path 정책
- [ ] resolved Build Plan 기반 ingress 설정 생성 / 적용
- [ ] login / session entry UX
- [ ] Settings UI

### Module Manager Module

Module을 관리하는 기능도 Core에 UI를 내장하기보다 **공식 관리 Module**로 제공하는 방향을 우선합니다.

- [ ] Module 목록 / 상태 조회
- [ ] Module 탑재 / 활성화
- [ ] Module 비활성화 / 분리 / 제거
- [ ] Module source / version 선택
- [ ] Web exposure (`host / prefix / routes`) 설정
- [ ] 변경 전 route conflict / 영향 미리보기
- [ ] Instance configuration 변경 diff / 확인
- [ ] Core / Runtime Provider API를 통한 lifecycle 적용
- [ ] Docker socket을 관리 Module에 직접 제공하지 않음

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
- [x] ADR-0008 — Manafield Network Planes
- [x] ADR-0009 — Operation Routing / Web Exposure 분리
- [x] ADR-0010 — Capability 기반 Dependency Resolution
- [x] ADR-0011 — Official Web Shell / Same-Origin Module Composition
- [ ] Runtime Protocol이 구체화되면 후속 ADR 추가
- [ ] Module package format이 구체화되면 후속 ADR 추가
- [ ] Permission / trust model이 구체화되면 후속 ADR 추가

## Phase 5 — CI/CD / Instance Build Plan

- [x] 인스턴스별 단일 Pipeline 원칙 확정
- [x] Module별 Jenkins Job을 만들지 않는 방향 확정
- [x] Module build contract를 source root Dockerfile로 시작
- [x] Module source type `git / dir` 분리
- [x] Jenkins `LOCAL_MODULES_ROOT` 기반 local Module discovery
- [x] Instance Definition v0
- [x] 공개 플랫폼과 Private Instance Configuration 분리 원칙
- [x] Build Plan Resolver v0
- [x] Jenkins 단일 Pipeline skeleton
- [x] `instance.yaml` 부재 시 `instance.bootstrap.yaml` 기반 first-run bootstrap
- [x] 최초 1회 Bootstrap Wizard 골격 / Account → PostgreSQL + Example Web 의존성 규칙
- [x] Bootstrap 선택형 PostgreSQL Resource example 실제 배포 연결
- [ ] Bootstrap Example Web 실제 구현
- [ ] Bootstrap Example Account 실제 구현
- [ ] GitHub webhook → 동일 Instance Pipeline 연결
- [ ] 변경 source 기반 selective build
- [ ] Core / Runtime Provider / Module image tagging
- [ ] Deploy 후 Health / Protocol verification
- [ ] CI credential store 기반 Secret 주입
- [ ] Jenkins 외 executor에서도 Build Plan 재사용 가능한 구조 검증

### Ingress Adapters

- [ ] Ingress Provider 공통 입력 / lifecycle 안정화
- [x] Traefik Ingress Adapter v0 — host exposure
- [x] Build Plan 단계 hostname 충돌 검사
- [ ] Traefik prefix / root route claim 렌더링
- [ ] Nginx Ingress Adapter
- [ ] Ingress 변경 rollback / stale route 정리

## Long-term

- [ ] Module package format
- [ ] Module install / update / remove
- [ ] Drag & Drop Module installation
- [ ] Permission review
- [ ] Distributed / advanced dependency management
- [ ] Module Registry distribution / discovery
- [ ] Compatibility Test Kit
- [ ] Third-party Module isolation
- [ ] External Runtime Provider protocol
- [ ] Remote Runtime Provider
- [ ] Kubernetes Runtime Provider

Kubernetes는 의도적으로 먼 목표로 둡니다. Docker는 초기 Runtime Provider 구현이며, Manafield Core 자체의 필수 의존성은 아닙니다.
