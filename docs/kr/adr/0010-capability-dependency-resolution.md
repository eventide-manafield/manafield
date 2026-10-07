# ADR-0010 — Capability 기반 Requirement Resolution

- 상태: **Accepted**
- 날짜: 2026-10-07

## Context

Manafield의 구성요소는 다른 Module의 기능, Database, Cache, Storage 같은 여러 종류의 필요사항을 가질 수 있습니다.

이 필요사항을 각각 별도 dependency 문법으로 만들면 다음과 같이 모델이 계속 늘어날 수 있습니다.

```text
module dependency
resource dependency
database dependency
storage dependency
...
```

하지만 소비자 입장에서 공통적인 질문은 하나입니다.

> **“이 구성요소가 동작하기 위해 어떤 계약이 필요한가?”**

또한 dependency를 concrete Module ID에 고정하면 Fork나 대체 구현이 같은 기능을 제공해도 원래 ID가 아니라는 이유만으로 연결할 수 없습니다.

반대로 자유형 `tags`만으로 dependency를 해결하면 실제 호환성을 보장할 수 없습니다.

## Decision

### 1. 모든 일반 Requirement를 Capability Contract로 표현한다

Manafield의 일반 dependency / requirement는 **Capability Contract** 하나로 통일합니다.

Module 기능과 infrastructure resource를 서로 다른 `requires` namespace로 나누지 않습니다.

예:

```yaml
id: manafield-echo

requires:
  capabilities:
    identity:
      id: manafield.identity
      version: "^1.0.0"

    state:
      id: database.postgresql
      version: "^1.0.0"
```

Echo 관점에서 둘 다 “필요한 계약”입니다.

차이는 그 Capability를 **무엇이 제공하고 어떤 방식으로 binding되는가**에 있습니다.

```text
manafield.identity
→ Module이 제공할 수 있음

database.postgresql
→ Resource Instance가 제공할 수 있음
```

### 2. Module identity와 Capability를 분리한다

Module `id`는 concrete implementation identity입니다.

일반 requirement는 특정 Module ID가 아니라 Capability를 요구합니다.

예:

```yaml
id: manafield-account

provides:
  capabilities:
    - id: manafield.identity
      version: "1.2.0"
      description: 사용자 identity와 session 계약
```

Fork 또는 대체 구현도 동일한 Capability Contract를 완전히 만족한다면 같은 Capability를 제공할 수 있습니다.

따라서:

```text
manafield-account
better-account
third-party-account
```

모두 `manafield.identity`의 compatible target 후보가 될 수 있습니다.

정말 특정 구현체의 고유 동작에 의존하는 경우 exact implementation dependency를 별도 escape hatch로 둘 수 있지만 일반 dependency의 기본값으로 사용하지 않습니다.

### 3. Capability는 Tag보다 강한 버전된 계약이다

`tags`는 검색, 분류, UI filtering을 위한 non-binding metadata입니다.

Tag는 dependency resolution에 사용하지 않습니다.

```text
Tag
→ descriptive / non-binding metadata

Capability
→ versioned dependency / compatibility contract
```

Capability는 최소한 다음 개념을 가집니다.

- stable `id`
- contract version
- optional `description`
- 해당 contract가 의미하는 compatibility rules

Description은 사람을 위한 표시/검색 metadata이며 compatibility 판정에 사용하지 않습니다.

### 4. Capability Version은 SemVer 계약 버전이다

Capability version은 Module release version과 독립적입니다.

예:

```text
Module:
  better-account v7.3.1

Provides:
  manafield.identity v2.1.0
```

Module version은 구현체 release를 의미하고, Capability version은 **어느 계약과 호환되는가**를 의미합니다.

Capability Contract version은 SemVer를 사용합니다.

```text
MAJOR
→ backward compatibility가 깨지는 계약 변경

MINOR
→ 기존 계약과 호환되는 기능/계약 확장

PATCH
→ 기존 계약 의미를 유지하는 수정
```

Capability를 제공하는 Instance는 자신이 구현한 **정확한 Capability version**을 선언합니다.

```yaml
provides:
  capabilities:
    - id: manafield.identity
      version: "2.3.1"
```

Consumer는 허용하는 **SemVer range**를 선언합니다.

```yaml
requires:
  capabilities:
    identity:
      id: manafield.identity
      version: "^2.1.0"
```

초기 constraint syntax는 일반적인 SemVer range 표현을 사용하며, `^2.1.0`은 개념적으로 `>=2.1.0 <3.0.0`을 의미합니다.

정확한 parser/library 선택은 구현 단계에서 결정합니다.

### 5. Feature subset negotiation은 하지 않는다

Resolver는 “이 구현체가 계약의 일부 Operation만 가지고 있으니 이 소비자에는 충분할 것 같다” 같은 추론을 하지 않습니다.

예를 들어 Capability v3에서 새 Operation이 추가되었는데 어떤 Fork가 v2를 기반으로 그 Operation만 cherry-pick했다고 해도, Manafield가 이를 자동으로 v3-compatible로 판단하지 않습니다.

구현자가 어떤 Capability version을 **완전히 구현했는지** 선언할 책임을 가집니다.

```text
Capability compatibility
→ whole contract 단위

Operation subset inference
→ 하지 않음
```

필요한 기능이 독립적인 계약 가치가 있다면 별도 Capability로 분리할 수 있지만, 실제 필요가 생기기 전까지 세분화하지 않습니다.

### 6. Operation과 Capability를 구분한다

Operation은 하나의 callable contract입니다.

Capability는 하나 이상의 Operation과 semantic rules를 묶을 수 있는 상위 compatibility contract입니다.

```text
Capability
└─ one or more Operations + semantic contract
```

모든 Operation이 Capability에 속할 필요는 없습니다.

Framework route나 Web UI 내부 endpoint를 자동으로 Operation 또는 Capability로 승격하지 않습니다.

Core는 Capability가 요구하는 Operation 존재 여부처럼 **기계적으로 검증 가능한 부분**을 검증할 수 있지만, semantic compatibility 전체를 추론하지는 않습니다.

### 7. Capability는 Module Instance 또는 Resource Instance가 제공한다

Capability matching을 위해 별도의 공통 `CapabilityProvider` 구현 인터페이스를 요구하지 않습니다.

Manafield가 공통으로 요구하는 것은 Registry에 등록되는 Capability metadata입니다.

```text
Module Instance
→ provides Capability metadata

Resource Instance
→ provides Capability metadata
```

구현 언어와 실행 방식은 서로 달라도 됩니다.

예:

```yaml
id: identity-core
kind: module
provides:
  capabilities:
    - id: manafield.identity
      version: "1.2.0"
```

```yaml
id: main-postgres
kind: resource
provides:
  capabilities:
    - id: database.postgresql
      version: "1.0.0"
```

Resource를 생성하거나 등록하거나 관리하는 system-side component가 있을 수 있지만, Capability binding의 concrete target은 그 관리 컴포넌트가 아니라 Resource Instance입니다.

### 8. Module/Resource Definition과 Instance를 구분한다

Definition은 종류와 설정 구조를 설명하고, Instance는 실제 설치/등록된 concrete 대상을 표현합니다.

```text
Definition
→ config schema
→ declaration shape

Instance
→ globally distinguishable instance ID
→ config values
→ endpoint / connection metadata
→ provides Capability metadata
```

Endpoint, concrete config value, secret reference, config schema는 Capability Contract에 포함하지 않습니다.

Capability의 matching surface는 의도적으로 작게 유지합니다.

```text
Capability
→ id + version
```

Description과 tags는 human-readable/search metadata로 둘 수 있지만 compatibility matching에는 사용하지 않습니다.

### 9. Instance ID는 Manafield Instance 안에서 unique하다

Module Instance와 Resource Instance의 concrete ID는 하나의 Manafield Instance 안에서 충돌할 수 없습니다.

중복 ID가 등록되면 Core는 자동 rename하지 않고 conflict로 거부합니다.

```text
main-postgres
identity-core
echo-prod
```

Binding은 이 unique Instance ID를 target으로 사용합니다.

### 10. Binding은 Requirement slot에서 target Instance ID를 가리킨다

Requirement와 concrete selection은 분리합니다.

예:

```yaml
requires:
  capabilities:
    identity:
      id: manafield.identity
      version: "^1.0.0"

    state:
      id: database.postgresql
      version: "^1.0.0"
```

Consumer Instance의 concrete Binding은 개념적으로 다음과 같습니다.

```yaml
bindings:
  identity: identity-core
  state: main-postgres
```

`identity`와 `state`는 Requirement slot/local alias입니다.

`identity-core`와 `main-postgres`는 concrete target Instance ID입니다.

Binding 상태는 다음 두 가지만 구분합니다.

```text
UNBOUND
BOUND
```

BOUND는 valid를 의미하지 않습니다.

Binding은 **대상이 지정되었는가**만 표현하며, target 존재 여부, Capability 제공 여부, version compatibility는 별도 validation/diagnostic concern입니다.

### 11. Resolver는 자동 Binding하지 않는다

호환되는 Instance가 하나뿐이더라도 Core Resolver는 자동으로 Binding하지 않습니다.

```text
compatible candidate count = 0
→ UNBOUND remains UNBOUND

compatible candidate count = 1
→ UNBOUND remains UNBOUND

compatible candidate count > 1
→ UNBOUND remains UNBOUND
```

Concrete target 선택은 명시적으로 수행합니다.

새 Module 설치 시 기존 Resource를 재사용할지 새 Resource를 만들지, 사용자에게 어떤 선택 UX를 보여줄지는 Core Resolver가 아니라 Module management layer의 책임입니다.

### 12. Capability Discovery는 Binding과 분리한다

Discovery는 read-only 조회 기능입니다.

세 가지 기본 조회 수준을 둡니다.

#### Compatible discovery

Capability ID와 SemVer range가 모두 맞는 Instance만 반환합니다.

#### Advanced discovery

Capability ID가 같은 Instance를 반환하며, version mismatch도 결과에 포함하고 compatibility를 표시합니다.

#### Full discovery

Capability filter 없이 등록된 Instance와 제공 Capability metadata를 조회할 수 있습니다.

Discovery 결과는 Binding을 생성하거나 변경하지 않습니다.

### 13. Version mismatch는 Warning이며 실행을 강제 차단하지 않는다

Binding된 target이 같은 Capability ID를 제공하지만 declared version이 required range 밖일 수 있습니다.

이 경우:

```text
CAPABILITY_VERSION_MISMATCH
→ WARNING
→ diagnostic log 기록
→ 실행 계속
```

SemVer range는 계약상 호환성 기대를 표현하지만, 사용자가 명시적으로 target Instance를 Binding했다면 version mismatch 하나만으로 Core가 실행을 차단하지 않습니다.

반면 target Instance가 존재하지 않거나 요구 Capability 자체를 제공하지 않는 상황은 version mismatch와 별개의 validation/error concern입니다.

### 14. Resource 관리 컴포넌트는 application data proxy가 아니다

Resource를 발견, 등록, 생성, 관리하는 system-side component가 존재할 수 있습니다.

PostgreSQL 예:

- existing PostgreSQL Resource 등록/발견
- database/schema allocation
- Module-scoped account/permission 준비
- connection metadata 준비
- secret reference 준비

하지만 실제 SQL traffic은 Core나 이 관리 컴포넌트를 통과하지 않습니다.

```text
Java Module   → JDBC / PostgreSQL driver ─┐
Go Module     → pgx / database/sql        ├→ PostgreSQL
Node Module   → pg                        ├→ PostgreSQL
Python Module → psycopg                   ┘
```

### 15. system-side Provider family는 별도 boundary로 유지한다

Runtime Provider, Resource 관리 컴포넌트, Ingress Provider/Adapter 같은 system-side component는 일반 Module과 다른 privilege/lifecycle을 가질 수 있습니다.

이들을 하나의 universal Provider Protocol로 성급하게 통합하지 않습니다.

중요한 구분은 다음과 같습니다.

```text
Capability binding target
→ Module Instance 또는 Resource Instance

Resource를 준비/관리하는 system component
→ 별도 lifecycle/protocol concern
```

## Consequences

### 장점

- 일반 Requirement 문법이 `requires.capabilities` 하나로 유지됨
- Capability matching surface가 `id + version`으로 작게 유지됨
- Module/Resource 구현 언어와 Capability Registry metadata가 분리됨
- Module Instance와 Resource Instance가 동일한 Binding 모델을 사용함
- Binding은 Requirement slot → unique Instance ID로 단순화됨
- Discovery와 Binding이 분리되어 자동 선택에 의한 비결정성이 없음
- version mismatch를 진단 가능하게 유지하면서 명시적 사용자 Binding을 존중함
- endpoint/config/schema/secret 정보가 Capability Contract를 비대하게 만들지 않음

### 비용

- Instance ID uniqueness enforcement가 필요함
- Capability Discovery API/UI가 필요함
- Binding validation과 diagnostic/error 모델이 별도로 필요함
- SemVer range matching과 warning reporting이 필요함
- Module/Resource Definition과 Instance 경계를 구현해야 함
- Resource 관리 컴포넌트의 lifecycle/protocol과 secret materialization은 별도 설계가 필요함
- dependency graph/cycle validation이 필요함

## Non-goals

이 ADR은 다음을 확정하지 않습니다.

- Capability Descriptor의 최종 JSON/YAML wire schema
- Capability Contract Registry의 저장/배포 방식
- SemVer parser/library
- pre-release version 정책
- 같은 Instance가 하나의 Capability 여러 버전을 동시에 제공하는 최종 표현
- Resource 관리 컴포넌트의 최종 protocol
- database credential/secret 전달 형식
- Module package format
- Permission/trust model
- dependency-cycle policy
- feature-subset negotiation

다음 항목은 이 ADR에서 명시적으로 결정했습니다.

```text
automatic binding
→ 하지 않음

binding state
→ UNBOUND / BOUND

binding target
→ unique Instance ID

discovery
→ compatible / advanced / full

version mismatch
→ warning + log + continue

Capability runtime/config payload
→ 넣지 않음
```

