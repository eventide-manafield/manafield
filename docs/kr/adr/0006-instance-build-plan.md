# ADR-0006 — Instance Build Plan / Single Pipeline

- 상태: Accepted
- 날짜: 2026-10-06

## Context

Manafield 인스턴스에는 Core, Runtime Provider, 여러 Module이 함께 배치될 수 있습니다.

각 Module Repository마다 Jenkins Job을 하나씩 만들면 Module 수에 비례해 Jenkins Job과 운영 화면이 늘어나며, 인스턴스 전체를 하나의 배포 단위로 보기 어려워집니다.

반대로 Jenkinsfile 안에 Module 목록과 Manafield 전용 규칙을 직접 하드코딩하면 Jenkins 자체가 Architecture의 Source of Truth가 되어 다른 CI/CD 시스템으로 옮기기 어려워집니다.

## Decision

Manafield는 **인스턴스별 Pipeline 하나**를 기본 배포 모델로 사용합니다.

```text
Manafield Instance Definition
        ↓
Build Plan Resolver
        ↓
Single CI Pipeline
        ↓
Core / Runtime Providers / Modules
        ↓
Deploy
        ↓
Health Verification
```

Jenkins는 첫 번째 CI executor이지만 Manafield의 배포 모델 자체는 Jenkins에 종속시키지 않습니다.

인스턴스 설정은 어떤 Core, Runtime Provider, Module을 사용할지 선언합니다. Resolver는 이 설정을 Build Plan으로 해석하고 Jenkins Pipeline은 그 Plan을 실행합니다.

Module 수가 늘어나더라도 Jenkins Job 수는 증가하지 않습니다.

## Module Build Contract

각 Module Repository는 자신의 내부 언어나 빌드 도구를 Jenkins에 노출할 필요가 없습니다.

초기 Container Module의 최소 Build Contract는 Repository root의 `Dockerfile`입니다.

```text
Python Module ─┐
Node Module   ─┼─ own Dockerfile → common build pipeline
Rust Module   ─┘
```

향후 별도의 Module package metadata가 필요해지면 이 계약을 확장할 수 있습니다.

## Trigger Model

여러 Repository의 webhook이 동일한 Manafield Instance Pipeline을 깨울 수 있습니다.

Pipeline은 변경된 source를 확인하고 가능한 경우 해당 구성요소만 다시 Verify / Build / Deploy할 수 있습니다.

예:

```text
reference-web push
        ↓
Manafield Instance Pipeline
        ↓
reference-web only
        ↓
Deploy
        ↓
Health check
```

Core 또는 공통 Protocol 변경처럼 영향 범위가 넓은 변경은 더 넓은 검증 / 재빌드를 수행할 수 있습니다.

## Reason

- Jenkins Job 수를 Module 수와 분리
- Manafield 인스턴스를 하나의 배포 단위로 표현
- Module 구현 언어와 CI Pipeline 분리
- Jenkins를 교체하더라도 Instance Definition / Build Plan 개념 재사용 가능
- Incremental build / deployment 확장 가능
- 전체 인스턴스의 배포 상태를 한 Pipeline에서 확인 가능

## Consequences

- Repository별 Jenkinsfile은 기본 요구사항이 아닙니다.
- Module Repository는 표준 Build Contract를 제공해야 합니다.
- Jenkins에는 Manafield 인스턴스별 Pipeline이 생성됩니다.
- Build Plan Resolver와 Instance Definition Schema가 필요합니다.
- Secrets는 Instance Definition이나 Repository에 직접 저장하지 않고 CI credential store에서 주입합니다.
- 배포 성공은 container/process 시작만으로 판단하지 않고 Health / Protocol verification까지 포함해야 합니다.
