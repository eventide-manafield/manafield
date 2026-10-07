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
→ Provider가 등록/관리하는 Resource가 제공할 수 있음
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

모두 `manafield.identity`의 compatible provider 후보가 될 수 있습니다.

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

Provider는 자신이 구현한 **정확한 Capability version**을 선언합니다.

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

### 7. Capability provider는 Module일 수도 Resource일 수도 있다

Capability matching 모델은 하나지만 provider의 성격은 다를 수 있습니다.

#### Module-provided Capability

예:

```text
manafield.identity ^1
        ↓
Account Module
```

Module이 Operation을 통해 Capability Contract를 제공합니다.

#### Resource-provided Capability

예:

```text
database.postgresql ^1
        ↓
main-postgres Resource
        ↓ managed / registered by
PostgreSQL Provider
```

Resource는 실제 Instance에 존재하는 concrete 자원입니다.

예:

```text
main-postgres
dev-postgres
remote-postgres
```

각 Resource는 자신이 만족하는 Capability와 정확한 contract version을 광고할 수 있습니다.

Provider는 Resource의 discovery / provisioning / allocation / connection metadata 준비를 담당할 수 있습니다.

### 8. Instance가 concrete Capability binding을 선택한다

Requirement와 concrete provider 선택을 분리합니다.

예:

```text
Echo.state
requires database.postgresql ^1
        ↓
compatible candidates
├─ main-postgres
└─ dev-postgres
        ↓
Instance binding
        ↓
main-postgres
```

개념적 Instance 설정:

```yaml
bindings:
  capabilities:
    echo.state:
      resource: main-postgres
```

Module Capability도 같은 원칙을 따릅니다.

```yaml
bindings:
  capabilities:
    echo.identity:
      module: better-account
```

최종 binding schema는 후속 설계에서 정합니다.

후보가 여러 개일 때 Core가 임의로 선택하지 않습니다.

후보가 하나뿐일 때 자동 binding할지 여부도 후속 정책으로 둡니다.

### 9. Resource Provider는 application data proxy가 아니다

`database.postgresql` Capability를 제공하는 Resource를 준비한다고 해서 Core나 Provider가 SQL query를 중계하지 않습니다.

PostgreSQL Provider는 다음 책임을 가질 수 있습니다.

- existing PostgreSQL Resource 등록 / discovery
- database / schema allocation
- Module별 account / permission 준비
- connection metadata 생성
- secret reference 준비

실제 query는 application이 native client로 직접 수행합니다.

```text
Java Module   → JDBC / PostgreSQL driver ─┐
Go Module     → pgx / database/sql        ├→ PostgreSQL
Node Module   → pg                        ├→ PostgreSQL
Python Module → psycopg                   ┘
```

Core는 JDBC나 특정 DB application client를 내장하지 않습니다.

### 10. Provider는 일반 Module이 아니다

Provider는 system-side component입니다.

현재 또는 향후 예:

```text
Provider
├─ Runtime Provider
│  └─ Docker
├─ Resource Provider
│  └─ PostgreSQL
└─ Ingress Provider / Adapter
   └─ Traefik
```

이들을 하나의 만능 Provider Protocol로 성급하게 통합하지 않습니다.

Capability matching은 공통으로 사용할 수 있지만 각 Provider family의 lifecycle / privilege / protocol은 별도로 설계합니다.

## Consequences

### 장점

- Requirement 문법이 `requires.capabilities` 하나로 단순화됨
- Module 기능과 infrastructure resource 모두 같은 SemVer contract matching을 사용할 수 있음
- Fork / 대체 구현이 concrete Module ID와 무관하게 호환 계약을 제공할 수 있음
- Instance가 현재 존재하는 compatible provider/resource 후보를 보고 binding할 수 있음
- PostgreSQL 같은 공용 자원을 여러 Module에 분리 allocation하는 구조로 확장 가능
- Resolver가 feature subset을 추론하지 않아 복잡성이 제한됨

### 비용

- Capability Contract Registry / Descriptor가 필요함
- SemVer range matching이 필요함
- Capability provider source가 Module인지 Resource인지에 따라 binding semantics가 달라짐
- 여러 후보가 있을 때 ambiguity 처리 정책이 필요함
- Provider Protocol과 secret injection 모델을 추가 설계해야 함
- dependency graph / cycle validation이 필요함

## Non-goals

이 ADR은 다음을 최종 확정하지 않습니다.

- Capability Descriptor의 최종 JSON/YAML schema
- Capability provider source 표현의 최종 schema
- Instance binding의 최종 schema
- 자동 provider selection 정책
- PostgreSQL Provider Protocol
- Database credential secret format
- Module package format
- Permission / trust model
- dependency cycle 처리 정책
- feature subset negotiation

이들은 구현 단계에서 후속 문서 또는 ADR로 구체화합니다.
