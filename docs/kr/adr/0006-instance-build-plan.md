# ADR-0006 — Instance Build Plan / Single Pipeline

- 상태: Accepted
- 날짜: 2026-10-06

> 후속 결정: [ADR-0014](0014-instance-working-release.md)는 **Manafield 플랫폼 자체 빌드**와 **독립 Module 설치 및 Instance Release/Deploy**를 분리합니다. 현재 Jenkins의 Module source build는 과도기 구현이며 최종 `build all`의 책임은 아닙니다.

## Context

Manafield 인스턴스에는 Core, Runtime Provider, 여러 Module이 함께 배치될 수 있습니다.

각 Module Repository마다 Jenkins Job을 하나씩 만들면 Module 수에 비례해 Jenkins Job과 운영 화면이 늘어나며, 인스턴스 전체를 하나의 배포 단위로 보기 어려워집니다.

반대로 Jenkinsfile 안에 Module 목록과 Manafield 전용 규칙을 직접 하드코딩하면 Jenkins 자체가 Architecture의 Source of Truth가 되어 다른 CI/CD 시스템으로 옮기기 어려워집니다.

## Decision

Manafield는 **인스턴스별 Pipeline 하나**를 기본 배포 모델로 사용합니다.

```text
Manafield Instance Definition
        ↓
Build Plan Resolver
        ↓
Single CI Pipeline
        ↓
Core / Runtime Providers / Modules
        ↓
Deploy
        ↓
Health Verification
```

Jenkins는 첫 번째 remote CI/CD frontend이지만 Manafield의 배포 모델 자체는 Jenkins에 종속시키지 않습니다.

인스턴스 설정은 어떤 Core, Runtime Provider, Module을 사용할지 선언합니다. `manafield` CLI는 이 설정을 Build Plan으로 해석하고, v0 Docker executor를 통해 build / deploy 단계를 수행하는 canonical entrypoint로 발전합니다.

Jenkins Pipeline은 가능한 한 이 CLI를 호출하며 webhook, approval, credential integration, build history 같은 CI 역할에 집중합니다. Module 수가 늘어나더라도 Jenkins Job 수는 증가하지 않습니다.

CLI-first 실행과 Docker v0 baseline은 [ADR-0013](0013-docker-v0-cli-execution.md)을 따릅니다.

## Module Source / Build Contract

Module source는 두 종류를 사용합니다.

```text
git
→ repository + ref

dir
→ CI executor가 허용한 local Module root 아래에서 Module ID로 발견
```

`dir` source는 `instance.yaml`에 임의 host path를 저장하지 않습니다. Jenkins는 별도 `LOCAL_MODULES_ROOT` 값을 받고 그 바로 아래 디렉터리를 스캔합니다.

```text
<LOCAL_MODULES_ROOT>/<module-id>/manafield.module.json
```

발견된 local source는 Jenkins workspace로 복사한 뒤 build하며, 원본 host directory 자체를 build workspace로 직접 사용하지 않습니다.

각 Module은 자신의 내부 언어나 빌드 도구를 Jenkins에 노출할 필요가 없습니다.

초기 Container Module의 최소 Build Contract는 Module source root의 `Dockerfile`입니다.

```text
Python Module ─┐
Node Module   ─┼─ own Dockerfile → common build pipeline
Rust Module   ─┘
Local Module  ─┘
```

향후 별도의 Module package metadata가 필요해지면 이 계약을 확장할 수 있습니다.

## Trigger Model

여러 Repository의 webhook이 동일한 Manafield Instance Pipeline을 깨울 수 있습니다.

Pipeline은 변경된 source를 확인하고 가능한 경우 해당 구성요소만 다시 Verify / Build / Deploy할 수 있습니다.

예:

```text
manafield-reference push
        ↓
Manafield Instance Pipeline
        ↓
manafield-reference only
        ↓
Deploy
        ↓
Health check
```

Core 또는 공통 Protocol 변경처럼 영향 범위가 넓은 변경은 더 넓은 검증 / 재빌드를 수행할 수 있습니다.

## Reason

- Jenkins Job 수를 Module 수와 분리
- Manafield 인스턴스를 하나의 배포 단위로 표현
- Module 구현 언어와 CI Pipeline 분리
- Jenkins 없이도 CLI + Docker로 같은 Instance 실행 의미론 재사용 가능
- Incremental build / deployment 확장 가능
- 전체 인스턴스의 배포 상태를 한 Pipeline에서 확인 가능

## Consequences

- Repository별 Jenkinsfile은 기본 요구사항이 아닙니다.
- Module Repository는 표준 Build Contract를 제공해야 합니다.
- Jenkins에는 Manafield 인스턴스별 Pipeline이 생성됩니다.
- Build Plan Resolver와 Instance Definition Schema가 필요합니다.
- Secrets는 Instance Definition이나 Repository에 직접 저장하지 않고 executor-specific credential integration에서 주입합니다. Jenkins Credentials Store는 첫 adapter일 수 있습니다.
- 배포 성공은 container/process 시작만으로 판단하지 않고 Health / Protocol verification까지 포함해야 합니다.
## Resource 전용 소비자 계약 (2026-10-19)

Resource에 `exclusiveTo: { module: <모듈 ID>, slot: <Requirement 슬롯> }`을
선언할 수 있다. 활성 Redis Resource에는 필수다. 다른 모듈·슬롯의 참조
(비활성 모듈 선언 포함), Core의 참조, 누락·비활성 소유자, 소유자의
해당 슬롯 바인딩 누락을 검증 단계에서 거부한다. Build Plan에도 이 선언을
보존하고 CI는 `resource-exclusive-consumers.tsv`를 출력한다.
Direct staging은 비밀 파일 생성·이미지 빌드 전에 정의를 검증하며,
Redis 자격증명 주입 시 소유자를 다시 검사한다. Jenkins Stage Release와
직접 staging은 같은 CLI Resource materializer로 이 계약을 적용한다.
기존 PostgreSQL·S3는 필드가 없으면 종전대로 동작한다.
자격증명 할당 정책이며 키 prefix를 Redis ACL로 간주하지 않는다.
현재 Redis executor의 Instance당 Resource 최대 1개 제한은 유지한다.

## 빌드 실행 환경 독립성 (2026-10-19 사용자 확정)

권장 자동 빌드·배포 실행 환경은 Jenkins다. 마나필드의 빌드·배포 기능은
Jenkins 설치, 전용 작업 공간 변수 또는 Jenkins API 없이 공통 CLI로도
수행할 수 있어야 한다. Jenkins는 공통 CLI를 실행하는 오케스트레이터다.

- Instance Definition과 executor 중립 Build Plan이 공통 입력 계약이다.
  같은 소스 revision·이미지·Build Plan을 입력하면 동등한 빌드 대상과
  Release 구성이 나와야 한다. 이것은 비트 단위 재현성을 주장하지 않는다.
- 빌드, Release 준비, 적용, 검증의 책임을 분리한다. Jenkins의 승인·예약·
  로그·자격증명 전달 기능은 공통 엔진에 대한 선택적 실행 환경이다.
- Resource 설정 해석, 전용 소비자 검사, 바인딩 및 비밀 파일 마운트 생성은
  공통 코드가 책임진다. Jenkinsfile과 직접 CLI에 별도 규칙을 복제하지 않는다.
  소비자에게 비밀 값이 아니라 허용된 파일 참조만 전달한다.
- CLI를 직접 실행할 때도 동일한 명시적 Instance Root·소스·자격증명
  입력과 검증을 사용한다. Jenkins가 없다는 이유로 검증을 생략하지 않는다.
- 현재 Docker 의존성은 Runtime/Build backend에 있다. Jenkins 실행 환경과
  Docker backend는 별개의 선택이다. 다른 backend 지원은 추후 범위다.
- 공통 빌드 또는 Release 준비는 운영 배포와 Traefik 공개를 자동 실행하지
  않는다. 배포 대상과 기존 데이터 수명주기는 별도로 명시한다.

현재 구현: 공통 CLI 이미지 빌더와 Compose 적용 executor에 더해
`stage-resources`가 PostgreSQL Module 할당·S3·전용 Redis 바인딩을 준비한다.
Jenkins Stage Release와 직접 staging이 이 코드를 재사용한다. 기존 Jenkins의
PostgreSQL/Core logging 및 Account 설정에는 과도기 중복이 남아 있다.
선택 적용은 `apply RELEASE_DIR --service NAME`으로 실행하며 기존 서비스를
자동 종료하지 않는다. 소스/단위 검증과 실제 Jenkins·본체 배포 검증은 구분한다.
