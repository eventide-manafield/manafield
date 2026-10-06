# ADR-0003 — Registry Snapshot / ArcSwap Read Model

- 상태: Accepted
- 날짜: 2026-10-06

## Context

Module Registry는 조회가 많고 수정은 상대적으로 적을 것으로 예상됩니다. Reader가 페이지를 열어둔 시간 동안 Registry lock을 유지하거나, 비동기 Cache Event 때문에 stale read가 생기는 구조는 피하고 싶습니다.

## Decision

Registry를 다음 두 모델로 분리합니다.

- `ModuleRegistry`: mutable Source of Truth
- `RegistrySnapshot`: immutable Read Model

Write는 `RwLock<ModuleRegistry>`를 사용하고, 변경 후 새 Snapshot을 만든 뒤 `ArcSwap`으로 현재 Snapshot을 원자적으로 교체합니다.

Read는 mutable Registry를 직접 잠그지 않고 현재 Snapshot을 읽습니다.

## Reason

- Read path에서 Registry lock 제거
- Reader가 항상 완전한 한 시점의 상태를 봄
- 기존 Snapshot을 참조 중인 Reader와 새 Snapshot 공존 가능
- Cache 정합성을 비동기 Event에 의존하지 않음

## Consequences

- Snapshot은 수정하지 않고 매 변경마다 새로 생성합니다.
- 이전 Snapshot은 마지막 `Arc` 참조가 사라질 때 자동 해제됩니다.
- Registry Event는 Audit / Metrics / Notification 용도로 사용할 수 있지만 Snapshot publish의 정합성 경로에는 넣지 않습니다.
