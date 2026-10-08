# ADR-0004 — Runtime Provider Boundary

- 상태: Accepted
- 날짜: 2026-10-06

## Context

Module은 Docker, local process, remote runtime, Kubernetes 등 다양한 방식으로 실행될 수 있습니다. Core 안에 Docker 전용 로직을 직접 결합하면 Docker가 사실상 Core의 필수 의존성이 됩니다.

## Decision

Module 실행 기술을 **Runtime Provider**라는 별도 시스템 계층으로 분리합니다.

Runtime Provider는 일반 Module이 아닙니다. Module을 생성하고 시작하고 중지하고 제거할 수 있는 privileged system component입니다.

Core는 특정 Runtime이 없어도 Registry, Operation Discovery, Validation 등 기본 기능을 수행할 수 있어야 합니다.

이 결정은 모든 배포 방식이 v0에서 동등하게 지원되어야 한다는 뜻은 아닙니다. 지원되는 v0 전체 Instance 구축/배포 baseline은 Docker로 고정하며, 자세한 내용은 [ADR-0013](0013-docker-v0-cli-execution.md)을 따릅니다.

## Reason

- Core와 실행 기술 분리
- Docker 없는 환경에서도 Core 동작
- 향후 Process / Remote / Kubernetes Provider 확장
- 강한 권한을 별도 경계에 배치

## Consequences

- Core와 Runtime Provider 사이에 Runtime Protocol이 필요합니다.
- Module은 가능하면 특정 제품명보다 필요한 Runtime capability를 선언하는 방향을 검토합니다.
- Runtime Provider가 없으면 자동 실행 기능은 제한되지만 Core 자체는 동작합니다.
