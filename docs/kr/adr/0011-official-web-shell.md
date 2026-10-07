# ADR-0011 — Official Web Shell과 Same-Origin Module Composition

- 상태: **Accepted**
- 날짜: 2026-10-07

## Context

Manafield는 Headless Core를 유지하면서도 실제 사용자에게는 `manafield.studio` 같은 하나의 일관된 Web 경험을 제공할 필요가 있습니다.

각 Web Module을 다음처럼 독립 subdomain으로만 노출하면 기술적으로는 단순하지만 사용자 경험이 지나치게 분절될 수 있습니다.

```text
account.manafield.studio
echo.manafield.studio
info.manafield.studio
...
```

반대로 모든 Module 기능과 API를 하나의 중앙 Web server 안에 구현하면 Module 독립성이 무너지고 다시 큰 monolithic application이 됩니다.

또한 모든 Module이 공식 Web UI에 의존하면 Headless 원칙과도 충돌합니다.

## Decision

### 1. Official Web Shell을 별도 Module로 둔다

공식 Web View는 **`manafield-web`이라는 별도 Module**로 구현합니다.

`manafield-web`은 Core에 내장하지 않습니다.

일반 Module이며 다음과 같은 사용자-facing shell 책임을 가질 수 있습니다.

- Instance home
- global navigation
- Registry 기반 Module discovery
- 공통 layout / visual shell
- login / session 진입 UX
- Module Web surface로 이동하기 위한 navigation

Core와 다른 Module은 `manafield-web`이 없어도 동작할 수 있어야 합니다.

일반 Module은 기본적으로 `manafield-web`에 dependency를 갖지 않습니다.

### 2. 하나의 public origin 아래 여러 Module을 조립한다

통합 Web UX를 사용하는 Instance는 같은 hostname 아래 path routing으로 Web surface를 조립할 수 있습니다.

예:

```text
manafield.studio/           → manafield-web
manafield.studio/account/*  → Account Module
manafield.studio/echo/*     → Echo Module
manafield.studio/...        → other Module
```

외부에서는 하나의 Website처럼 보이지만 각 route의 실제 backend는 독립 Module일 수 있습니다.

이 public route mapping은 Module identity나 Operation path에서 자동 생성하지 않습니다.

ADR-0009에 따라 Instance Definition과 Ingress Provider가 concrete route binding을 결정합니다.

### 3. Web Shell을 중앙 application-data proxy로 만들지 않는다

`manafield-web`은 모든 Module API를 대신 중계하는 mandatory gateway가 아닙니다.

가능한 경우 Ingress는 public route를 실제 Module로 직접 전달합니다.

```text
Browser
   ↓
Ingress
   ├─ /              → manafield-web
   ├─ /echo/*        → Echo
   ├─ /account/*     → Account
   └─ ...
```

따라서:

- Echo 장애가 Web Shell process의 data-path 병목으로 확대되지 않음
- Web Shell 재시작이 모든 Module API를 중단시키지 않음
- Module이 자신의 HTTP semantics / streaming / WebSocket 등을 직접 소유할 수 있음

Web Shell이 Core Registry 같은 API를 읽어 navigation을 구성하는 것은 허용하지만, 일반 Module traffic의 필수 reverse proxy 역할은 맡지 않습니다.

### 4. Module Web surface와 Module 기능 dependency를 분리한다

Module은 Web UI를 제공하지 않아도 됩니다.

API-only / worker Module도 정상적인 Module입니다.

또한 Module이 Web UI를 가진다고 해서 `manafield.web-shell` 같은 Capability를 반드시 요구하지 않습니다.

```text
Echo backend
→ Web Shell 없이도 동작 가능

manafield-web
→ Registry / Instance metadata를 보고 Echo Web surface를 발견 가능
```

Web Shell은 Module의 기능 dependency가 아니라 **optional presentation layer**입니다.

### 5. 초기 Web composition은 full-page/path surface로 시작한다

초기 구현에서는 dynamic micro-frontend, remote component runtime, arbitrary JavaScript injection 같은 복잡한 composition을 도입하지 않습니다.

v0의 기본 단위는 다음입니다.

```text
Web Shell
→ root/home/navigation

Module Web Surface
→ Instance가 할당한 path namespace에서 자체 page/API 제공
```

예:

```text
/echo/*
/account/*
```

Module별 frontend bundle을 Shell process 안에 동적으로 로드하는 모델은 실제 필요가 생긴 뒤 별도 설계합니다.

### 6. 독립 subdomain exposure도 계속 지원한다

Same-origin composition은 통합 Manafield UX를 위한 선택지이지 모든 Web service의 강제 규칙이 아닙니다.

독립 application이나 infrastructure tool은 ADR-0009의 `host` exposure를 계속 사용할 수 있습니다.

예:

```text
jenkins.manafield.studio
misskey.manafield.studio
reference.manafield.studio
```

Instance는 Module 성격에 따라 `host`, `prefix`, `routes`, `external`을 선택할 수 있습니다.

### 7. 초기 Official Web Shell 구현은 Go를 사용한다

첫 `manafield-web` 구현은 **Go 기반의 경량 Web Module**로 시작합니다.

초기 구현 방향:

- single executable
- standard library `net/http` 우선
- `embed.FS`를 사용한 static asset 포함
- Core Registry 조회
- application Health Operation
- runtime-configurable listen port
- non-root container
- minimal runtime image
- 자체 DB 없음

React/Node runtime을 production 필수 요소로 만들지 않습니다.

필요한 frontend assets가 생기면 build-time tool로 Node/Vite 등을 사용할 수 있지만 production runtime은 Go binary 하나로 유지할 수 있습니다.

정확한 Go version과 frontend implementation은 Repository bootstrap 시 고정합니다.

## Initial routing shape

개념 예:

```text
Host: manafield.studio

/                 → manafield-web
/_manafield/*     → manafield-web internal surface (candidate)
/account/*        → Account Web surface
/echo/*           → Echo Web surface
```

`/_manafield/*`는 Shell 내부 자산/API를 위한 **후보 namespace**이며 이 ADR에서 최종 reserved path로 확정하지 않습니다.

Root path reservation과 route conflict는 Build Plan Resolver에서 검증합니다.

## Authentication note

Same-origin composition은 login/session UX를 단순화할 수 있습니다.

예를 들어 Account/Identity Module이 같은 origin의 path에서 인증을 수행하면 Browser가 subdomain 사이를 이동하지 않고 session UX를 유지할 수 있습니다.

그러나 cookie scope, token propagation, CSRF, Identity Capability의 정확한 Protocol은 이 ADR의 범위가 아닙니다.

Identity 기능 dependency는 ADR-0010에 따라 Capability Contract로 표현합니다.

## Consequences

### 장점

- 사용자에게 하나의 일관된 `manafield.studio` 경험 제공
- Core의 Headless 원칙 유지
- Module implementation/runtime 독립성 유지
- Web Shell이 거대한 backend monolith가 되는 것을 방지
- Module API traffic이 중앙 Shell process를 필수 경유하지 않음
- Go single-binary implementation으로 Shell 자체를 가볍게 유지 가능
- 독립 subdomain application과 통합 path application을 함께 지원

### 비용

- Ingress Adapter가 같은 hostname의 path route composition을 지원해야 함
- Build Plan Resolver가 path conflict와 precedence를 정확히 검증해야 함
- path prefix 아래에서 동작하는 Module의 asset/redirect/cookie semantics를 고려해야 함
- 공통 UI consistency를 위해 향후 style/token 또는 Web contribution 규칙이 필요할 수 있음

## Non-goals

이 ADR은 다음을 확정하지 않습니다.

- dynamic micro-frontend runtime
- arbitrary frontend bundle loading
- shared JavaScript dependency runtime
- cookie/session Protocol
- Identity Module 구현체
- 최종 reserved root path 목록
- 모든 Module을 same-origin path로 강제하는 정책
