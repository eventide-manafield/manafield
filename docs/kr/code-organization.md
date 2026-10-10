# 코드 구성 원칙 — 책임과 변경 이유 중심

Manafield는 파일 길이나 함수 수를 목표로 삼아 모듈을 분할하지 않습니다. **함께 변경되는 코드는 묶고, 변경 이유가 독립적인 코드는 분리하되, 분리에 드는 탐색·추상화 비용도 고려**합니다.

## 판단 기준

1. **변경 이유**: 어느 요구사항 또는 운영 정책이 바뀌면 이 코드가 수정되는가? 변경 이유가 다르면 분리를 검토합니다.
2. **응집도**: 자료구조와 도우미가 같은 불변조건을 지키고 함께 수정된다면 한 모듈에 둡니다. 짧다는 이유로 새 파일을 만들지 않습니다.
3. **의존성**: 호출자가 내부 구현을 이해할 필요가 없도록 좁은 경계를 유지합니다. 순환적·양방향 의존성을 만들면서까지 분리하지 않습니다.
4. **안전성**: Secret, 인증, 권한, 파일 영속성과 같은 독립적 보안 정책은 관련 코드와 테스트를 같은 경계 안에 둡니다.
5. **기능 보존**: 순수 리팩터링 PR은 API, CLI 플래그, JSON/TSV/Compose 산출물, 디스크 레이아웃의 의미를 바꾸지 않습니다. 변경 시 별도 PR로 다룹니다.

## 현재 Core 코드의 책임 경계

| 위치 | 소유 책임 |
| --- | --- |
| `src/cli.rs` | 명령 실행 분기, 사용자 출력, 공통 오류 모델 |
| `src/cli/args.rs` | 문자열 인자·옵션을 유효한 명령으로 파싱 |
| `src/cli/client.rs` | Core HTTP 엔드포인트와 요청·응답 프로토콜 |
| `src/cli/extensions.rs` | 신뢰된 운영자 CLI 확장 프로그램 호출 정책 |
| `src/cli/staging.rs` | 직접 배포 산출물 준비 흐름의 조율 |
| `src/cli/staging/secrets.rs` | 배포 Secret의 생성·재사용·파일 권한 |
| `src/build_plan.rs` | Instance/Build Plan 데이터 모델과 상위 해석 흐름 |
| `src/build_plan/validation.rs` | Instance Definition의 유효성 규칙 |
| `src/build_plan/ci_output.rs` | Jenkins가 요구하는 호환 Build Plan 파일 산출 |

테스트도 검증하는 책임의 경계에 가깝게 둡니다. 다만 테스트 수나 파일 길이만으로 프로덕션 모듈을 더 분해하지 않습니다.

## 다음에 다룰 분리 후보 (이번 PR 범위 제외)

- **직접 배포의 소스·이미지 준비 / Resource materialization / Compose 렌더링**: `staging.rs`에서 서로 다른 변경 원인을 갖는 부분이 남아 있습니다. 현재 데이터 교환 계약부터 분명히 한 다음 분리합니다.
- **Jenkinsfile**: Stage 자체는 Jenkins에 남기되, Resource / Secret / Compose 도메인 로직은 독립 CLI 명령과 동등성 테스트가 먼저 마련되면 점진적으로 옮깁니다.
- **Home Hero 시각 효과**: 화면별 설정·기하 계산·HUD 스타일 등 변화가 독립적인지 검토하고, 스크린샷 또는 결정적 랜덤 시드 기반 회귀 검증 뒤에 별도 Repository에서 진행합니다.

## 기본 검증

```sh
cargo fmt --all -- --check
cargo test --locked --all-features
cargo clippy --locked --all-features --all-targets -- -D warnings
cargo build --locked --release --bin manafield --bin manafield-core
```

CI/Jenkins가 아닌 권한 제한 개발 환경에서는 Docker 통합 테스트를 실행하지 못할 수 있습니다. Docker 통합 결과를 Rust 단위 테스트 결과와 혼동하지 않습니다.
