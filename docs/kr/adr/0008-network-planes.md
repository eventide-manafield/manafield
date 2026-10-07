# ADR-0008 — Manafield Network Planes

- 상태: Accepted
- 날짜: 2026-10-07

## Context

Manafield Core와 Module 사이의 내부 통신과, Traefik / Cloudflared 같은 외부 진입 계층과의 통신은 서로 다른 신뢰 경계를 가집니다.

하나의 범용 `proxy` 네트워크 이름은 역할이 불명확하고, Core까지 외부 경계에 직접 연결되는 구조를 유도할 수 있습니다.

## Decision

Manafield 배포는 다음 두 네트워크 plane을 구분합니다.

- `manafield-modules`: Core와 Module 사이의 내부 통신망
- `manafield-edge`: 외부 공개가 필요한 서비스와 edge infrastructure가 만나는 공용 경계망

Core는 기본적으로 `manafield-modules`에만 연결합니다.

외부 Web UI를 제공하는 Module은 필요할 때 두 네트워크에 함께 연결할 수 있습니다.

```text
Internet
   |
Cloudflare / Traefik
   |
manafield-edge
   |
Manafield Reference
   |
manafield-modules
   |
Manafield Core
```

`manafield-edge`는 개별 Manafield Compose가 소유하지 않는 external Docker network로 취급합니다.

## Reason

- 내부 Module 통신과 외부 ingress 경계 분리
- Core의 직접 외부 노출 방지
- 네트워크 이름만으로 역할 식별 가능
- Traefik / Cloudflared / Jenkins와 공개 Module이 동일한 edge network를 공유 가능
- 향후 Runtime Provider가 Module을 내부 network에 일관되게 연결할 수 있음

## Consequences

- 외부 공개가 필요 없는 Module은 `manafield-edge`에 연결하지 않습니다.
- `manafield-edge`는 배포 전에 host infrastructure가 생성해야 합니다.
- 기존 범용 `proxy` network 이름은 사용하지 않습니다.
- 인터넷 outbound 가능 여부와 `manafield-edge` 연결 여부는 별개입니다.
