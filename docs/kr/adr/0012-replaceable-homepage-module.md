# ADR-0012 — 교체 가능한 Homepage Module과 Web Shell 분리

- 상태: **Accepted**
- 날짜: 2026-10-07
- 관련: ADR-0011 — Official Web Shell과 Same-Origin Module Composition

## Context

ADR-0011의 초기 구현에서는 `manafield-web`이 Web Shell 역할과 함께 public root `/`의 Instance home도 소유하는 형태로 시작했습니다.

하지만 Manafield의 Web 경험 역시 Module 조합으로 구성하려면 홈페이지 콘텐츠 자체도 Shell 구현에 고정되어서는 안 됩니다.

예를 들어 하나의 Instance는 다음과 같이 여러 page Module을 조합할 수 있어야 합니다.

```text
/              → Homepage Module
/sample/*      → Sample Module
/social/*      → Social Module
/account/*     → Account Module
```

Homepage를 `manafield-web` 내부에 구현하면 Homepage를 교체하거나 제거하려 할 때 Official Web Shell 자체를 수정해야 하고, Shell이 점차 중앙 Web application으로 커질 위험이 있습니다.

## Decision

### 1. Homepage는 독립 Module로 둔다

public root `/`의 콘텐츠는 별도 Homepage Module이 소유할 수 있습니다.

공식 예제 구현은 초기에는 `manafield-home`을 사용합니다.

```text
manafield-home
→ replaceable page Module
→ Instance가 필요할 때 "/" route를 binding
```

다른 Instance는 `manafield-home` 대신 자체 Homepage Module을 사용할 수 있고, Homepage가 없는 Headless Instance도 정상입니다.

### 2. `manafield-web`은 Shell 역할에 집중한다

`manafield-web`은 다음과 같은 공통 presentation 책임을 가질 수 있습니다.

- Web surface discovery
- navigation
- 공통 shell / chrome
- login / session entry UX
- Registry 기반 상태/관리 surface

그러나 public root Homepage 콘텐츠는 필수 책임이 아닙니다.

통합 Instance에서는 Shell 자체를 별도 prefix에 둘 수 있습니다.

```text
/_manafield/*
```

정확한 reserved namespace는 후속 정책에서 바뀔 수 있지만, Homepage와 Shell의 route ownership은 분리합니다.

### 3. 동일 hostname의 nested prefix composition을 허용한다

Homepage가 `/`를 소유하면서 다른 Module이 더 구체적인 prefix를 소유할 수 있어야 합니다.

```text
/              → home
/_manafield/*  → shell
/social/*      → social
```

따라서 동일 hostname에서 서로 다른 prefix는 자동 충돌로 취급하지 않습니다.

동일한 normalized prefix의 중복 claim은 충돌입니다.

Ingress Adapter는 더 구체적인 prefix가 root 또는 상위 prefix보다 우선하도록 deterministic priority를 생성해야 합니다.

### 4. Prefix는 strip하지 않는다

기존 Web Surface 원칙을 유지합니다.

```text
public /_manafield/foo
→ module /_manafield/foo
```

Module에는 Instance가 배정한 base path를 runtime configuration으로 전달할 수 있습니다.

초기 container convention:

```text
MANAFIELD_WEB_BASE_PATH=/_manafield
```

### 5. Ingress가 각 Module로 직접 전달한다

Homepage도 Shell을 mandatory proxy로 거치지 않습니다.

```text
Browser
  ↓
Ingress
  ├─ /              → Homepage Module
  ├─ /_manafield/*  → Web Shell
  ├─ /sample/*      → Sample Module
  └─ /social/*      → Social Module
```

각 page Module은 독립 process/container로 유지됩니다.

## Consequences

### 장점

- Homepage 자체를 설치/교체/제거 가능
- Web Shell이 monolithic homepage application으로 커지는 것을 방지
- Sample, Social, Account 등 page Module을 같은 방식으로 추가 가능
- Headless Instance 원칙 유지
- 같은 origin에서 일관된 사용자 경험을 유지하면서 process boundary 보존

### 비용

- Build Plan이 동일 host의 nested prefix를 deterministic하게 처리해야 함
- prefix 아래에서 실행되는 Module은 base path를 정확히 지원해야 함
- Shell과 Homepage 사이의 공통 visual language는 별도 style/token 규칙이 필요할 수 있음

## ADR-0011과의 관계

ADR-0011의 다음 초기 결정은 이 ADR로 수정됩니다.

```text
기존:
  / → manafield-web

현재:
  /             → replaceable Homepage Module
  /_manafield/* → manafield-web (한 가지 가능한 binding)
```

Official Web Shell을 별도 Module로 두고, Ingress가 Module traffic을 직접 전달하며, same-origin path composition을 사용한다는 나머지 원칙은 그대로 유지합니다.
