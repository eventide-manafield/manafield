# ADR-0001 — Rust 기반 Headless Core

- 상태: Accepted
- 날짜: 2026-10-06

## Context

Manafield Core는 Registry, Protocol, Validation, Lifecycle 등 플랫폼의 가장 엄격한 경계를 담당합니다. 동시에 Web UI나 특정 Runtime에 종속되지 않아야 합니다.

## Decision

Manafield Core의 기준 구현은 **Rust**로 작성하고 **Headless Core**로 유지합니다.

Core는 Web UI 없이 실행 가능해야 하며, Web / CLI / Mobile 등은 Core API를 사용하는 별도 Client로 취급합니다.

## Reason

- 엄격한 타입과 상태 모델
- 명시적인 오류 처리
- 낮은 Runtime overhead
- 단일 바이너리 배포
- UI와 Core lifecycle 분리

## Consequences

- Rust toolchain을 프로젝트에서 고정합니다.
- Core API가 UI보다 먼저 안정화되어야 합니다.
- UI 편의를 위해 Core 내부에 UI 전용 상태를 넣지 않습니다.
