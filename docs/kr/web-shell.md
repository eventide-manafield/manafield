# Official Web Shell v0 Design

> 이 문서는 ADR-0011을 실제 첫 구현으로 옮기기 위한 초기 설계 메모입니다.
>
> 아직 구현 전 단계이며, `manafield-web` Repository bootstrap 시 세부 구조가 바뀔 수 있습니다.

## 1. 역할

`manafield-web`은 Manafield의 공식 Web presentation layer입니다.

v0 책임:

- `/` Instance home
- Core Registry 조회
- Module 목록 / 설명 / version 표시
- Web surface navigation
- 기본 layout / visual shell
- `/manafield/health` Operation

v0에서 하지 않는 것:

- 모든 Module API reverse proxy
- Module lifecycle 관리
- Docker 접근
- Database 접근
- dynamic micro-frontend loading
- arbitrary Module JavaScript injection

## 2. Runtime

초기 구현은 Go를 사용합니다.

```text
manafield-web
├─ Go HTTP server
├─ embedded static assets
├─ Core Registry client
└─ health endpoint
```

권장 초기 기술 선택:

- Go standard library `net/http`
- `html/template` 또는 매우 얇은 static frontend
- `embed.FS`
- JSON Core API client
- Docker multi-stage build
- minimal non-root runtime image

Go version은 Repository 생성 시 현재 stable version으로 pinning합니다.

## 3. Core 연결

초기에는 Reference Module과 비슷하게 환경변수로 Core endpoint를 주입할 수 있습니다.

```text
MANAFIELD_CORE_URL=http://core:8080
```

Web Shell은 Core Registry에서 Module metadata를 읽습니다.

최소 사용 대상:

```http
GET /modules
GET /modules/{id}
```

Registry 장애가 발생해도 Shell process 자체가 즉시 종료될 필요는 없습니다.

UI에서 Core unavailable 상태를 표시할 수 있어야 합니다.

## 4. Public routing

통합 Instance 예:

```text
Host: manafield.studio

/             → manafield-web
/account/*    → account Module
/echo/*       → echo Module
```

Shell process가 `/account/*`, `/echo/*`를 proxy하지 않습니다.

Traefik Adapter가 resolved Build Plan을 기반으로 같은 hostname의 route들을 생성합니다.

따라서 Web Shell deployment와 Module route deployment는 분리됩니다.

## 5. Route ownership

Web Shell은 root `/`와 Shell에 예약된 최소 namespace만 소유합니다.

후보:

```text
/
/_manafield/*
```

`/_manafield/*`의 최종 예약 여부는 아직 결정하지 않습니다.

Module route는 Instance Definition에서 명시적으로 binding합니다.

예:

```yaml
modules:
  - id: manafield-echo
    exposure:
      type: prefix
      host: manafield.studio
      prefix: /echo
      targetPort: 8080
```

실제 schema는 Build Plan prefix/routes 설계에서 확정합니다.

Prefix는 strip하지 않습니다.

```text
/echo/foo
→ Echo도 /echo/foo로 수신
```

Instance가 배정한 base path는 초기 Container convention으로 `MANAFIELD_WEB_BASE_PATH=/echo`처럼 주입할 수 있습니다. Module은 이 값을 asset/redirect/cookie/browser-side API path에 일관되게 사용합니다.

## 6. Navigation metadata

v0에서는 Registry의 다음 metadata를 우선 사용합니다.

- Module `id`
- `name`
- optional `description`
- `version`

Web navigation에 필요한 추가 metadata는 Module Protocol의 기능 dependency와 분리합니다.

Web Surface metadata는 우선 `page`와 `api`를 구분합니다.

```text
page
→ navigation 가능한 user-facing surface

api
→ public HTTP API, navigation 없음
```

Page metadata 후보:

```text
title
description
icon
navigation order/group
```

public path 자체는 Instance가 결정하므로 Module metadata는 authoritative path를 갖지 않습니다.

자세한 설계는 [Web Surface v0](web-surface.md)을 참고합니다.

## 7. Authentication

v0 Web Shell은 Identity 구현체를 자체 내장하지 않습니다.

향후 login/session UX는 `manafield.identity` Capability를 제공하는 Module과 연결합니다.

```text
manafield-web
→ login entry UX

Identity Module
→ authentication/session contract

Other Modules
→ require manafield.identity when needed
```

Web Shell이 존재하지 않는 Headless Instance에서도 Identity Module과 다른 Module은 동작할 수 있어야 합니다.

## 8. Failure boundaries

다음 failure는 분리되어야 합니다.

```text
manafield-web down
→ Shell/home unavailable
→ Module APIs may remain reachable

Echo down
→ /echo unavailable
→ Shell and other Modules remain available

Core Registry temporarily unavailable
→ Shell shows degraded discovery state
→ already-routed Module traffic can continue
```

Ingress가 Module traffic을 Shell process에 강제로 통과시키지 않는 이유입니다.

## 9. Initial Repository shape

후보:

```text
manafield-web/
├─ cmd/
│  └─ manafield-web/
│     └─ main.go
├─ internal/
│  ├─ coreclient/
│  ├─ web/
│  └─ view/
├─ web/
│  ├─ static/
│  └─ templates/
├─ manafield.module.json
├─ Dockerfile
├─ go.mod
├─ go.sum
└─ README.md
```

작은 규모에서는 과도한 package 분리를 피하고 실제 코드가 생기는 만큼만 디렉터리를 유지합니다.

## 10. First implementation milestone

첫 milestone은 다음만 성공하면 됩니다.

```text
manafield-web starts
→ /manafield/health = OK
→ Core Registry read succeeds
→ / renders Instance home
→ registered Modules are listed
→ description/version visible
→ no DB
→ no Docker privilege
```

그 다음 Build Plan에 same-host prefix routing을 추가해:

```text
manafield.studio/
manafield.studio/echo/
```

를 동시에 구성하는 것을 다음 milestone으로 둡니다.
