# ADR-0005 — Docker Runtime Provider Process Isolation

- 상태: Accepted
- 날짜: 2026-10-06

## Context

Docker daemon 접근권한은 매우 강합니다. Core가 직접 `docker.sock`에 접근하면 Core의 취약점이 Host Docker 제어권까지 확대될 수 있습니다.

한편 Docker Provider를 완전히 별도 Repository와 제품으로 분리하면 초기 개발과 버전 관리가 불필요하게 복잡해집니다.

## Decision

Docker Runtime Provider는 **Core와 같은 Manafield Repository에서 관리하지만 별도 Binary / Process로 실행**합니다.

예상 배포 형태:

```text
same source / release
├─ manafield
└─ manafield-runtime-docker
```

Docker 배포 시:

- Core에는 `docker.sock`을 mount하지 않습니다.
- Docker Runtime Provider에만 `docker.sock`을 제공합니다.
- Core와 Provider는 제한된 Runtime Protocol로 통신합니다.
- 초기 local transport로 Unix Domain Socket을 우선 검토합니다.

Provider API는 임의 Docker 명령 실행이 아니라 Manafield Runtime lifecycle 작업만 표현해야 합니다.

## Reason

- Core 침해 시 Docker 권한으로 바로 확장되는 피해 범위 감소
- 개발 / Release / Version 관리는 한 Repository에서 유지
- 나중에 별도 Process로 뜯는 대공사를 피함
- Provider에서 Docker-specific security policy 적용 가능

## Consequences

Docker Provider는 다음과 같은 정책 경계를 가져야 합니다.

- privileged container 기본 금지
- 임의 host path mount 제한
- Docker socket 재노출 금지
- 허용 가능한 network / resource 정책
- Runtime request validation

동일 Repository라고 해서 동일 Process일 필요는 없습니다. 보안 경계는 Process와 권한으로 형성합니다.
