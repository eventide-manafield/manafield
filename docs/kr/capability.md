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

Capability를 제공하는 Instance는 정확한 version을 선언합니다.

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

## 4. Capability 제공 선언

Capability matching에 필요한 계약 표면은 의도적으로 작게 유지합니다.

```yaml
provides:
  capabilities:
    - id: manafield.identity
      version: "1.2.0"
```

핵심 필드는 다음 두 가지입니다.

```text
id
→ stable Capability Contract ID

version
→ exact Capability Contract version
```

`description`, `tags` 같은 human-readable metadata는 둘 수 있지만 matching에는 사용하지 않습니다.

Endpoint, connection 정보, config value, secret, config schema 같은 runtime/configuration 정보는 Capability에 넣지 않습니다.

## 5. Capability를 제공하는 Instance

Capability를 제공하기 위한 공통 언어-level `CapabilityProvider` 인터페이스를 요구하지 않습니다.

대신 Manafield Registry가 이해할 수 있는 **공통 등록 metadata**를 사용합니다.

Capability를 제공할 수 있는 concrete 대상은 현재 두 종류입니다.

```text
Module Instance
Resource Instance
```

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

구현 언어가 Python, Java, Rust, Go 등으로 달라도 Registry metadata가 같은 계약을 따르면 됩니다.

Resource를 생성/등록/관리하는 system-side component가 별도로 존재할 수 있지만, Capability binding의 concrete target은 그 관리 컴포넌트가 아니라 **Resource Instance**입니다.

## 6. Definition과 Instance

Module과 Resource는 definition과 concrete instance를 구분합니다.

```text
Definition
→ 종류와 선언 구조
→ config schema 같은 설정 형식

Instance
→ 실제 설치/등록된 대상
→ concrete config values
→ endpoint / connection metadata
→ provides Capability metadata
```

예:

```text
PostgreSQL Resource Definition
  config schema
    host: string
    port: integer
    database: string

main-postgres Resource Instance
  endpoint: postgres:5432
  config values:
    database: manafield
  provides:
    database.postgresql@1.2.0
```

Capability는 이 설정 구조를 소유하지 않습니다.

## 7. Instance ID

Module Instance와 Resource Instance는 하나의 Manafield Instance 안에서 **고유한 Instance ID**를 가져야 합니다.

```text
main-postgres
identity-core
echo-prod
```

같은 ID가 이미 등록되어 있다면 Core는 자동 rename하지 않고 conflict로 거부합니다.

```text
duplicate Instance ID
→ registration conflict
```

설치 UI가 대체 ID를 제안할 수는 있지만 Registry 규칙 자체는 unique ID를 요구합니다.

## 8. Matching

기본 compatibility matching은 다음과 같습니다.

```text
required Capability ID == provided Capability ID
AND
provided exact SemVer satisfies required SemVer range
```

예:

```text
Requirement
database.postgresql ^1.0.0

main-postgres  provides 1.2.0  compatible
dev-postgres   provides 1.0.3  compatible
old-postgres   provides 0.9.0  version mismatch
redis-main     provides cache.redis 1.0.0  different capability
```

Resolver는 Operation subset이나 implementation ancestry를 추론하지 않습니다.

## 9. Capability Discovery

Discovery는 Binding과 분리된 조회 기능입니다.

### 기본 조회

Capability ID와 version range가 모두 호환되는 Instance만 반환합니다.

```text
discover compatible
→ name match + version match
```

### 고급 조회

Capability ID가 같은 Instance를 version compatibility와 함께 반환합니다.

버전이 맞지 않는 Instance도 표시할 수 있습니다.

```text
main-postgres  1.2.0  compatible
old-postgres   0.9.0  incompatible: version
```

### 전체 조회

Capability filter 없이 등록된 Instance와 제공 Capability metadata를 조회할 수 있습니다.

Discovery 결과는 **절대로 Binding을 자동 생성하지 않습니다**.

호환 후보가 0개, 1개, 여러 개인지와 상관없이 Binding되지 않은 Requirement는 계속 UNBOUND입니다.

## 10. Binding

Binding은 **Requirement slot → target Instance ID** 관계입니다.

Consumer Module Instance가 다음 Requirement를 가진다고 가정합니다.

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

해당 Instance의 concrete Binding은 개념적으로 다음처럼 표현합니다.

```yaml
bindings:
  identity: identity-core
  state: main-postgres
```

`identity`, `state`는 Capability ID가 아니라 **Requirement slot / local alias**입니다.

`identity-core`, `main-postgres`는 Capability를 제공하도록 등록된 **Instance ID**입니다.

Binding 상태는 두 가지만 구분합니다.

```text
UNBOUND
→ target Instance ID가 지정되지 않음

BOUND
→ target Instance ID가 지정됨
```

BOUND는 곧 valid를 의미하지 않습니다. Binding은 연결 지정 여부만 표현하고, 실제 유효성 확인은 별도 validation/diagnostic 단계에서 수행합니다.

## 11. Validation과 Version Mismatch

Binding된 target은 실행 전 또는 실행 과정에서 다음을 검증할 수 있습니다.

```text
target Instance가 존재하는가
target Instance가 요구한 Capability ID를 제공하는가
declared Capability version이 required range와 호환되는가
```

존재하지 않는 target이나 전혀 다른 Capability를 가리키는 경우는 정상적인 binding으로 사용할 수 없습니다.

단, **Capability version mismatch만으로 실행을 차단하지 않습니다.**

```text
CAPABILITY_VERSION_MISMATCH
→ WARNING
→ 충분한 diagnostic log 기록
→ 실행 계속
```

예:

```text
WARN CAPABILITY_VERSION_MISMATCH
consumer: echo-prod
slot: state
required: database.postgresql ^1.0.0
bound: main-postgres
provided: database.postgresql 2.0.0
```

SemVer range는 계약상 호환성을 판단하기 위한 정보이며, 사용자가 명시적으로 Binding한 대상을 Core가 version mismatch 하나만으로 강제 차단하지 않습니다.

## 12. Resource runtime 정보

Endpoint, connection metadata, config value, secret reference는 concrete Resource Instance 또는 Instance configuration에 둡니다.

```yaml
id: main-postgres
kind: resource

endpoint:
  host: postgres
  port: 5432

config:
  database: manafield
  ssl: false

provides:
  capabilities:
    - id: database.postgresql
      version: "1.2.0"
```

Resource 종류가 받을 수 있는 설정의 구조, 즉 **config schema**는 Resource Definition 쪽에 둡니다.

같은 원칙을 Module에도 적용할 수 있습니다.

```text
Module Definition
→ config schema

Module Instance
→ concrete config values
→ bindings
→ provides metadata
```

Capability는 endpoint/config/schema/secret 전달 형식을 포함하지 않습니다.

## 13. Resource 관리 컴포넌트

PostgreSQL 같은 Resource를 발견, 등록, 생성, 관리하는 system-side component는 별도로 둘 수 있습니다.

예:

```text
PostgreSQL resource manager/provider
├─ existing PostgreSQL 발견/등록
├─ database/schema allocation
├─ account/permission 준비
├─ connection metadata 준비
└─ secret reference 준비
```

하지만 application data path는 Core나 관리 컴포넌트를 통과하지 않습니다.

```text
Echo ──JDBC/driver──> PostgreSQL
```

이 관리 컴포넌트와 Resource Instance 자체를 Capability matching에서 같은 개념으로 취급하지 않습니다.

## 14. Feature subset

Capability compatibility는 전체 contract 단위입니다.

```text
"v3 Operation 하나만 cherry-pick했다"
≠
"Capability v3를 구현했다"
```

구현자는 자신이 완전히 구현한 Capability version을 선언합니다.

Manafield는 특정 Consumer가 사용하는 Operation subset만 보고 compatibility를 재해석하지 않습니다.

## 15. Description과 Tags

다음 항목에는 optional `description`을 둘 수 있습니다.

- Module
- Operation
- Capability
- Resource

Description과 `tags`는 검색/관리 UI를 위한 metadata이며 Capability matching에는 사용하지 않습니다.

## 16. 현재 확정된 원칙

```text
Requirement
→ 모두 Capability

Capability matching surface
→ id + version

provides
→ exact contract version

requires
→ SemVer range

Capability source
→ Module Instance 또는 Resource Instance

Instance ID
→ Manafield Instance 안에서 unique

Binding
→ Requirement slot → target Instance ID

Binding state
→ UNBOUND / BOUND

Discovery
→ compatible / advanced-name-match / all

Automatic Binding
→ 하지 않음

Version mismatch
→ warning + log + continue

Endpoint / config value
→ concrete Instance

Config schema
→ Module/Resource Definition
```

## 17. 아직 미정인 항목

- Capability Contract Registry의 저장/배포 방식
- Capability 정의 파일의 최종 위치와 형식
- SemVer parser/library
- pre-release version 정책
- Module/Resource Instance가 같은 Capability 여러 버전을 동시에 제공하는 최종 표현
- dependency cycle policy
- Resource 관리 컴포넌트별 lifecycle / protocol
- Secret 전달의 최종 schema

