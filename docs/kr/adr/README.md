# Architecture Decision Records

Manafield의 주요 Architecture 결정과 그 이유를 기록합니다.

ADR은 구현 세부사항보다 **오래 유지할 경계와 책임**을 남기는 용도로 사용합니다.

| ADR | 결정 | 상태 |
| --- | --- | --- |
| [0001](0001-rust-headless-core.md) | Rust 기반 Headless Core | Accepted |
| [0002](0002-operation-contract.md) | Operation Contract를 공통 기능 계약으로 사용 | Accepted |
| [0003](0003-registry-snapshot.md) | Registry Write Model + ArcSwap Snapshot Read Model | Accepted |
| [0004](0004-runtime-provider-boundary.md) | Runtime Provider를 일반 Module과 분리 | Accepted |
| [0005](0005-docker-provider-isolation.md) | Docker Provider를 별도 Process로 격리 | Accepted |
| [0006](0006-instance-build-plan.md) | Instance Build Plan / 인스턴스별 단일 Pipeline | Accepted |
| [0007](0007-public-platform-private-instance.md) | Public Platform / Private Instance Configuration | Accepted |
| [0008](0008-network-planes.md) | Manafield Network Planes | Accepted |
| [0009](0009-operation-web-exposure.md) | Operation Routing과 Web Exposure 분리 | Accepted |

새로운 장기 결정이 생기면 번호를 이어 추가합니다.
