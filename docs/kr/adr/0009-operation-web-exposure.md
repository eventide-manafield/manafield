# ADR-0009 — Operation Routing과 Web Exposure 분리

- 상태: Accepted
- 날짜: 2026-10-07

## Context

Manafield의 Operation은 Module이 제공하는 호출 가능한 기능을 표현하는 공통 계약입니다.

반면 Module이 브라우저에 Web UI나 HTTP endpoint를 어떻게 공개할지는 인스턴스의 외부 노출 정책에 가깝습니다.

이 둘을 같은 개념으로 취급하면 다음 문제가 생깁니다.

- Operation 이름이 곧 전역 URL 경로가 되어 Module 간 이름 충돌이 발생할 수 있음
- Web application이 path prefix 아래에서만 동작하도록 강제될 수 있음
- Module 자체의 공개 가능한 기능과 특정 인스턴스의 실제 hostname / path 정책이 결합됨
- 외부 노출 설정을 바꾸기 위해 Module Protocol 자체를 변경해야 할 수 있음

## Decision

Operation routing과 Web exposure를 별개의 계약으로 유지합니다.

### Operation

Operation은 Module namespace 안에서 식별되는 callable contract입니다.

예를 들어 서로 다른 두 Module이 모두 `generate` Operation을 가져도 충돌하지 않습니다.

Operation ID로부터 `/generate` 같은 공개 URL을 자동으로 생성하지 않습니다.

### Web Exposure

Web exposure는 인스턴스 구성에서 결정합니다.

Module은 Web surface를 제공할 수 있음을 선언할 수 있지만, 실제 public hostname / path는 Instance Definition이 선택합니다.

초기 exposure 형태는 다음을 고려합니다.

- `none`: 외부 Web 노출 없음
- `host`: 독립 hostname을 통한 reverse proxy
- `prefix`: Module이 하나의 path namespace를 소유
- `routes`: 선택한 root-level path를 개별적으로 claim
- `external`: 이미 존재하는 외부 URL을 연결

### Host Proxy

일반적인 독립 Web Module의 기본 방식으로 권장합니다.

예:

```text
reference.manafield.studio
```

Module은 `/` 기준으로 동작할 수 있으므로 asset path, redirect, cookie, WebSocket 등에서 path-prefix 의존을 줄일 수 있습니다.

### Prefix Proxy

Path 기반 노출이 필요한 Module에 명시적으로 사용합니다.

예:

```text
/modules/example/...
```

Prefix는 기본적으로 strip하지 않습니다.

```text
public  /echo/foo
→ module /echo/foo
```

Instance가 할당한 prefix/base path는 Module runtime configuration으로 전달할 수 있어야 하며, Module은 해당 경로를 기준으로 asset, redirect, cookie, browser-side API path를 구성합니다.

Prefix Proxy는 모든 Module의 기본 노출 방식으로 사용하지 않습니다.

### Root Route Claims

특수한 Module은 Module namespace 없이 전역 경로를 claim할 수 있습니다.

예:

```text
/healthz
/search
/hook
```

이 경로는 전역 URL 공간의 희소 자원으로 취급합니다.

실제 public path와 Module 내부 target path는 별개로 설정할 수 있습니다.

```yaml
routes:
  - match: exact
    public: /search
    target: /api/search
```

### Conflict Detection

Build Plan Resolver는 배포 전에 public route claim 충돌을 검사합니다.

최소한 다음은 충돌로 처리합니다.

- 동일 hostname의 중복 소유
- 동일 exact path의 중복 claim
- prefix claim이 다른 exact / prefix route를 가리는 경우
- Manafield가 예약한 root path와의 충돌

충돌이 발견되면 배포를 계속하지 않고 Build Plan 생성을 실패시킵니다.

오류는 충돌 경로와 이를 claim한 Module을 명시해야 합니다.

예:

```text
Route conflict: /search

claimed by:
  module-a
  module-b

Choose a different public path or isolate one Module behind a prefix or host.
```

### Reserved Paths

Manafield 자체가 사용하는 root namespace는 별도 정책으로 예약할 수 있습니다.

예:

```text
/api/*
/.well-known/*
```

구체적인 예약 목록은 실제 public API 구조가 안정화될 때 확정합니다.

## Reason

- Operation Contract를 Web routing 기술과 독립적으로 유지
- Module ID / Operation ID의 namespace와 전역 URL namespace를 분리
- 독립 Web application에는 host proxy를 사용해 구현 제약을 줄임
- 특수 Module에는 root-level route claim을 허용
- 충돌을 runtime이 아니라 build / deploy 전에 발견
- 공개 URL 정책을 private Instance Configuration에 유지

## Consequences

- Module manifest만으로 public URL이 자동 결정되지 않습니다.
- Instance Definition과 Build Plan에 Web exposure 모델이 추가됩니다.
- Build Plan Resolver는 route normalization과 conflict detection을 담당하게 됩니다.
- ingress configuration은 resolved Build Plan에서 생성하거나 적용할 수 있어야 합니다.
- Root-level route claim은 특별한 경우에만 사용합니다.
- Operation API와 Web proxy URL은 서로 다른 lifecycle과 compatibility 정책을 가질 수 있습니다.
