# ADR-0002 — Operation Contract

- 상태: Accepted
- 날짜: 2026-10-06

## Context

Module은 Python, Go, Node.js, Java, Rust 등 서로 다른 언어로 구현될 수 있습니다. Core가 각 Module의 내부 함수나 타입을 직접 이해하는 구조는 언어 독립성을 깨뜨립니다.

## Decision

Module이 외부에 제공하는 호출 가능한 기능을 **Operation Contract**로 표현합니다.

Operation은 다음을 포함합니다.

- ID
- Input / Output DataSchema
- Binding
- Codec

Binding과 Codec은 분리합니다. 현재 첫 Binding은 HTTP이며, 첫 Codec은 JSON과 MessagePack입니다.

Module은 선택적으로 기존 Operation 하나를 `healthOperation`으로 참조할 수 있습니다.

## Reason

- 구현 언어와 계약 분리
- Discovery / Validation 공통화
- HTTP 외 Binding 확장 가능
- JSON 외 wire format 확장 가능
- Health 정보를 중복 정의하지 않음

## Consequences

- Registry에는 Operation Contract가 Module Descriptor와 함께 저장됩니다.
- Core는 Module 내부 구현이 아니라 Contract만 신뢰합니다.
- 새로운 Binding / Codec은 기존 Operation 모델을 깨지 않고 추가할 수 있어야 합니다.
