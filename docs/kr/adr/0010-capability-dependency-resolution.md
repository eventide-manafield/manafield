# ADR-0010 — Capability 기반 Dependency Resolution

- 상태: **Accepted**
- 날짜: 2026-10-07

## Context

Manafield Module은 서로 다른 구현 언어와 배포 형태를 가질 수 있고, 동일한 기능을 여러 구현체가 제공할 수 있습니다.

Module dependency를 구체적인 Module ID에 직접 고정하면 다음 문제가 생깁니다.

- 기존 Module을 Fork한 호환 구현이 원래 ID가 아니라는 이유만으로 dependency를 만족하지 못함
- 공식 구현을 대체 구현으로 교체하기 어려움
- Module ID가 기능 계약과 구현 identity를 동시에 떠안게 됨
- Instance별로 다른 구현을 선택하기 어려움

반대로 자유형 `tags`만으로 dependency를 해결하면 의미가 너무 느슨합니다.

예를 들어 두 Module에 모두 `auth` tag가 있어도 한 구현은 로그인 UI만 제공하고 다른 구현은 session validation만 제공할 수 있습니다. Tag 존재만으로 실제 호출 계약의 호환성을 보장할 수 없습니다.

또한 Database, Cache, Storage 같은 기반 자원은 사용자 기능을 제공하는 일반 Module과 성격이 다릅니다. 이러한 자원은 Provider가 준비하고 Module에 binding하는 편이 자연스럽습니다.

## Decision

### 1. Module identity와 dependency contract를 분리한다

Module `id`는 **구체적인 구현체의 identity**입니다.

Module 간 일반 dependency는 기본적으로 특정 Module ID가 아니라 **Capability Contract**를 요구합니다.

개념 예:

```yaml
id: manafield-echo

requires:
  capabilities:
    identity:
      id: manafield.identity
      version: 1
```

Identity 구현체는 다음과 같이 같은 Capability를 제공할 수 있습니다.

```yaml
id: manafield-account

provides:
  capabilities:
    - id: manafield.identity
      version: 1
```

Fork 또는 대체 구현도 동일한 계약을 만족한다면 같은 Capability를 제공할 수 있습니다.

```yaml
id: better-account

provides:
  capabilities:
    - id: manafield.identity
      version: 1
```

Echo는 구현체 이름이 아니라 `manafield.identity v1` 계약에 의존합니다.

### 2. Capability는 Tag보다 강한 호환성 계약이다

`tags`는 검색, 분류, UI 필터링, 사람이 이해하기 위한 metadata로 사용합니다.

Tag는 dependency resolution에 사용하지 않습니다.

```text
Tag
→ descriptive / non-binding metadata

Capability
→ dependency / compatibility contract
```

Capability는 최소한 안정적인 ID와 version을 가집니다.

장기적으로 Capability Contract는 하나 이상의 Operation과 의미 규칙을 연결할 수 있어야 합니다.

예:

```text
manafield.identity v1
├─ identity.current-user
├─ identity.validate-session
└─ semantic compatibility rules
```

정확한 Descriptor schema와 version constraint 문법은 후속 설계에서 정의합니다.

### 3. Operation과 Capability를 구분한다

Operation은 **하나의 호출 가능한 기능 계약**입니다.

Capability는 **대체 가능한 구현체가 공통으로 만족해야 하는 상위 호환성 계약**입니다.

```text
Capability
└─ one or more Operations + semantic contract
```

모든 Operation이 반드시 Capability에 속할 필요는 없습니다.

Web UI 내부 endpoint와 같은 구현 세부 route를 자동으로 Capability 또는 Operation으로 승격하지 않습니다.

### 4. Instance가 concrete binding을 선택한다

Dependency requirement와 실제 구현체 선택을 분리합니다.

```text
Module requirement
        ↓
Capability candidates in Registry
        ↓
Instance binding
        ↓
Concrete Module
```

후보가 여러 개 존재하는 경우 Core가 임의로 하나를 선택하지 않습니다.

Instance Definition 또는 관리 UI가 concrete binding을 명시하는 방향을 사용합니다.

후보가 하나뿐인 경우의 자동 binding 정책은 후속 설계에서 결정합니다.

정말로 특정 구현체의 고유 동작에 의존해야 하는 경우에는 **명시적인 exact implementation dependency**를 별도 escape hatch로 허용할 수 있지만, 일반 dependency의 기본값으로 사용하지 않습니다.

### 5. Infrastructure resource dependency를 별도로 표현한다

Database, Cache, Object Storage, Filesystem 같은 기반 자원은 Module Capability와 분리합니다.

개념 예:

```yaml
requires:
  resources:
    state:
      id: database.postgresql
      version: 1
```

이 requirement는 일반 Module이 아니라 해당 Resource를 제공할 수 있는 **Provider**가 만족합니다.

예:

```text
Echo
└─ requires resource: database.postgresql
                         ↓
                 PostgreSQL Provider
```

Provider는 DB connection을 대신 query하는 proxy가 아닙니다.

예를 들어 PostgreSQL Provider는 다음과 같은 provisioning / allocation 책임을 가질 수 있습니다.

- database / schema 생성
- Module별 account / permission 생성
- connection metadata 생성
- secret reference 준비

실제 application query는 Module이 자신의 native client를 통해 Database에 직접 수행합니다.

```text
Java Module   → JDBC / PostgreSQL driver ─┐
Node Module   → pg                       ├→ PostgreSQL
Python Module → psycopg                  ┘
```

Core가 JDBC 또는 특정 DB client를 내장하는 구조로 만들지 않습니다.

### 6. Provider는 일반 Module이 아니다

Provider는 Manafield가 외부 실행 환경이나 기반 자원을 연결하기 위한 system-side component입니다.

현재 확정된 Runtime Provider 외에도 앞으로 Resource Provider가 존재할 수 있습니다.

예:

```text
Provider
├─ Runtime Provider
│  └─ Docker
├─ Resource Provider
│  └─ PostgreSQL
└─ Ingress Provider / Adapter
   └─ Traefik
```

다만 이들을 하나의 공통 Provider Protocol로 성급하게 통합하지 않습니다.

각 Provider 종류는 권한과 lifecycle이 다를 수 있으며, 필요한 경계와 Protocol은 별도로 설계합니다.

## Consequences

### 장점

- Fork / 대체 구현이 동일 Capability Contract를 구현해 기존 dependency를 만족할 수 있음
- Module 생태계가 특정 공식 구현 ID에 과도하게 결합되지 않음
- Instance별로 구현체를 선택할 수 있음
- 기능 dependency와 infrastructure resource dependency가 명확히 구분됨
- PostgreSQL 같은 공용 기반 자원을 여러 Module에 분리 allocation하는 구조로 확장 가능
- Protocol 중심의 언어 독립성을 유지함

### 비용

- Capability ID / version 정책이 필요함
- Capability Contract와 Operation 간 validation이 필요함
- 여러 후보가 있을 때 binding / ambiguity 처리 정책이 필요함
- Resource Provider Protocol과 secret injection 모델을 추가 설계해야 함
- Module install 전에 dependency graph를 검증해야 함

## Non-goals

이 ADR은 다음 세부 규격을 확정하지 않습니다.

- Capability Descriptor의 최종 JSON/YAML schema
- version range / negotiation 문법
- 자동 provider selection 알고리즘
- PostgreSQL Provider Protocol
- Database credential secret format
- Module package format
- Permission / trust model
- dependency cycle 처리 정책

이들은 구현 단계에서 후속 문서 또는 ADR로 구체화합니다.
