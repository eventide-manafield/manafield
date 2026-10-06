# ADR-0007 — Public Platform, Private Instance Configuration

- 상태: Accepted
- 날짜: 2026-10-07

## Context

Manafield 자체는 공개 소프트웨어로 배포할 수 있지만, 실제 사용자 인스턴스에는 개인 설정, 비공개 Module, 사설 Integration, 내부 서비스 주소 등 공개해서는 안 되는 정보가 섞일 수 있습니다.

Core Repository 안에서 실제 운영 인스턴스 구성을 함께 관리하면 플랫폼 코드와 개인 배포 상태의 경계가 흐려집니다.

## Decision

Manafield의 **플랫폼 / Protocol / 공식 구현은 공개 가능**한 대상으로 두고, **실제 Manafield Instance Configuration은 별도의 Private configuration**으로 관리하는 것을 기본 방향으로 합니다.

```text
Public
├─ Manafield Core
├─ Runtime Protocol
├─ official Runtime Providers
├─ reference Modules
└─ public Modules

Private by default
├─ actual instance.yaml
├─ enabled Module list
├─ private Module sources
├─ private Integrations
└─ deployment-specific configuration
```

Repository의 `deploy/instance.example.yaml`은 공개 가능한 예제일 뿐 실제 운영 인스턴스 설정의 저장 위치가 아닙니다.

개인 Module과 Integration은 Manafield Protocol을 구현하는 한 Public 또는 Private 중 어느 형태든 사용할 수 있습니다.

Secrets는 Private Instance Definition에도 직접 저장하지 않습니다. Secret은 CI credential store, runtime secret store, 환경별 secret injection 등 별도의 비밀정보 경로를 사용합니다.

## Reason

- 공개 플랫폼 코드와 개인 운영 상태 분리
- 개인 Module / Integration을 공개할 필요 없이 Manafield 사용 가능
- Core 공개 여부를 security boundary로 사용하지 않음
- Instance 설정 유출 시 피해 범위를 줄이기 위해 Secret을 별도 관리
- 동일한 공개 Manafield를 서로 완전히 다른 사설 인스턴스로 구성 가능

## Consequences

- 실제 운영용 `instance.yaml`은 Core Repository에 commit하지 않습니다.
- 공개 Repository에는 example / schema / documentation만 둡니다.
- Jenkins 같은 CI executor는 필요할 경우 Private Instance Configuration과 Private Module Repository에 대한 별도 Credential을 사용합니다.
- Build Plan은 Public/Private source를 동일한 추상화로 처리해야 합니다.
- 향후 Instance Configuration의 저장 위치와 배포 방식을 별도 Repository 또는 서버 측 private configuration으로 구체화할 수 있습니다.
