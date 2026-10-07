# Capability Contract v0

> 이 문서는 ADR-0010의 Capability 기반 Requirement Resolution을 구체화하는 초기 설계입니다.
>
> Manafield는 아직 pre-alpha이므로 최종 wire schema는 구현 과정에서 조정될 수 있습니다.

## 1. 핵심 원칙

Manafield에서 일반적인 필요사항은 모두 **Capability Requirement**로 표현합니다.

```text
다른 Module의 기능이 필요함
→ Capability

Database가 필요함
→ Capability

Cache가 필요함
→ Capability

Storage가 필요함
→ Capability
```

별도의 `requires.resources`, `requires.services` namespace를 만들지 않습니다.

## 2. Requirement

Module은 자신이 필요한 Capability에 local alias를 붙여 선언합니다.

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

여기서:

```text
identity
state
```

는 해당 Module 내부에서 dependency를 구분하기 위한 local name입니다.

```text
manafield.identity
database.postgresql
```

는 global Capability Contract ID입니다.

## 3. Capability Version

Capability version은 **계약 버전**입니다.

Module release version과 독립적입니다.

```text
Module version
→ concrete implementation release

Capability version
→ compatibility contract version
```

Capability version은 SemVer를 사용합니다.

Provider는 정확한 version을 선언합니다.

```yaml
version: "2.3.1"
```

Consumer는 SemVer range를 선언합니다.

```yaml
version: "^2.1.0"
```

초기 의미:

```text
^2.1.0
→ >=2.1.0 <3.0.0
```

MAJOR / MINOR / PATCH 의미:

```text
MAJOR
→ backward-incompatible contract change

MINOR
→ backward-compatible contract expansion

PATCH
→ contract meaning을 유지하는 correction
```

## 4. Module-provided Capability

Module은 자신이 완전히 구현한 Capability를 선언할 수 있습니다.

```yaml
provides:
  capabilities:
    - id: manafield.identity
      version: "1.2.0"
      description: 사용자 identity와 session 기능
```

`description`은 optional human-readable metadata이며 matching에는 사용하지 않습니다.

Capability Contract가 Operation들을 요구한다면 Core는 검증 가능한 범위에서 실제 Module Descriptor의 Operation들과 비교할 수 있습니다.

## 5. Resource-provided Capability

Infrastructure도 같은 Capability matching을 사용합니다.

예를 들어 Instance에 등록된 Resource:

```yaml
id: main-postgres
description: Manafield instance shared PostgreSQL

provides:
  capabilities:
    - id: database.postgresql
      version: "1.0.0"
      description: PostgreSQL database connection capability
```

`main-postgres`는 Capability ID가 아니라 **concrete Resource ID**입니다.

```text
database.postgresql
→ 계약

main-postgres
→ 그 계약을 제공하는 실제 자원
```

Resource는 PostgreSQL Provider가 발견하거나 등록하거나 생성할 수 있습니다.

## 6. Matching

Resolver의 기본 matching은 단순합니다.

```text
required capability ID == provided capability ID
AND
provided exact SemVer satisfies required SemVer range
```

예:

```text
Requirement
database.postgresql ^1.0.0

Candidates
main-postgres  provides 1.2.0  ✅
dev-postgres   provides 1.0.3  ✅
old-postgres   provides 0.9.0  ❌
redis-main     provides cache.redis 1.0.0 ❌
```

Resolver는 Operation subset이나 implementation ancestry를 추론하지 않습니다.

## 7. Binding

Capability Requirement와 concrete provider selection은 분리합니다.

개념 예:

```yaml
bindings:
  capabilities:
    echo.identity:
      module: better-account

    echo.state:
      resource: main-postgres
```

이 예에서:

```text
echo.identity
→ Module provider

echo.state
→ Resource provider
```

둘 다 Capability binding이라는 점은 동일합니다.

최종 binding schema는 Build Plan / Instance Definition 설계에서 안정화합니다.

## 8. Provider source

Capability를 제공하는 source는 최소 두 종류가 있습니다.

```text
Capability Provider Source
├─ Module
└─ Resource
```

### Module

호출 가능한 기능을 Operation으로 제공합니다.

예:

```text
manafield.identity
→ Account Module
```

### Resource

connection/configuration이 필요한 infrastructure를 제공합니다.

예:

```text
database.postgresql
→ main-postgres Resource
→ PostgreSQL Provider가 관리
```

Capability model은 공통이지만 실제 binding materialization 방식은 provider source에 따라 다를 수 있습니다.

## 9. Resource Provider

Resource Provider는 일반 Module이 아닙니다.

PostgreSQL Provider 예:

```text
PostgreSQL Provider
├─ existing PostgreSQL 발견/등록
├─ Resource metadata 관리
├─ database/schema allocation
├─ account/permission 준비
├─ connection metadata 준비
└─ secret reference 준비
```

실제 SQL traffic은 Provider/Core를 통과하지 않습니다.

```text
Echo ──JDBC/driver──> PostgreSQL
```

## 10. Feature subset

Capability compatibility는 전체 contract 단위입니다.

```text
"v3 Operation 하나만 cherry-pick했다"
≠
"Capability v3를 구현했다"
```

Module/Provider 작성자가 자신이 완전히 구현한 Capability version을 선언합니다.

Manafield는 특정 Consumer가 쓰는 Operation subset만 보고 compatibility를 재해석하지 않습니다.

## 11. Description과 Tags

다음 항목에는 optional `description`을 둘 수 있습니다.

- Module
- Operation
- Capability
- Resource

Description은 검색/관리 UI를 위한 정보입니다.

`tags`도 검색/분류용 metadata입니다.

둘 다 Capability matching에는 사용하지 않습니다.

## 12. 아직 미정인 항목

- Capability Contract Registry의 저장/배포 방식
- Capability 정의 파일의 최종 위치와 형식
- SemVer parser/library
- pre-release version 정책
- Module이 같은 Capability 여러 버전을 동시에 제공하는 최종 표현
- binding 자동 선택 정책
- dependency cycle policy
- Provider별 binding materialization schema
