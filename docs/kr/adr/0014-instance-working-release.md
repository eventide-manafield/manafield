# ADR-0014 — Instance 작업본, 불변 Release, 영속 Resource

- 상태: Accepted (설계 원칙; CLI 명령 문법과 구현은 진행 예정)
- 날짜: 2026-10-08
- 관련 결정: [ADR-0006](0006-instance-build-plan.md), [ADR-0010](0010-capability-dependency-resolution.md), [ADR-0013](0013-docker-v0-cli-execution.md)

## 문제

기존 Jenkins 중심의 파이프라인은 Instance 설정, Module 소스 빌드, Resource 준비, Release 디렉터리 생성과 배포를 한 번에 실행한다. 이 방식은 `manafield build`를 플랫폼 빌드와 Instance 운영이 섞인 명령으로 만들 수 있다.

사용자에게는 이미 배포한 Resource를 계속 사용하면서 Module과 Binding 구성을 변경하고, 특정 시점의 구성을 이름 붙여 배포하는 단순한 흐름이 필요하다.

## 결정: 서로 다른 네 영역

1. **Platform Build** — Manafield 자체 구성요소(CLI, Core, 공식 실행기/Provider 등)를 빌드한다. 외부 Module은 이 빌드의 구성원이 아니다.
2. **Instance Definition / Working Copy** — Module/Resource Instance와 Requirement slot Binding을 기술하는 수정 가능한 YAML 작업본이다.
3. **Release** — 작업본을 검증하여 확정한 **불변 YAML 스냅샷**이다. Module/Resource 자체의 새 생성이나 데이터 복제본이 아니다.
4. **Runtime State** — 현재 실행 중인 컨테이너, 실제 Resource Instance, Volume, DB 데이터와 관리 상태다. Release 파일과 생명주기가 다르다.

CLI와 Core 서버는 **서로 다른 바이너리**로 분리하되 하나의 Manafield 배포 묶음에 포함한다. CLI는 사용자 명령을 해석하고 작업을 공통 관리/배포 엔진에 위임한다. 향후 Docker Runtime Provider의 강한 권한은 Core로 유입시키지 않는다.

## Binding은 논리적 대상 참조

```text
manafield module bind <consumer-id> <requirement-slot> <target-instance-id>
manafield module bind echo-prod state main-postgres
```

위 명령은 `echo-prod.requires.state -> main-postgres`를 작업본에 선언한다. 대상은 Module 또는 Resource Instance일 수 있으며 **프로그램을 합치거나 대상 버전을 복제하지 않는다**. 해당 대상이 이미 존재하는지, 제공 Capability가 맞는지에 대한 검증/진단은 구분한다. 호환 후보가 있어도 자동 Binding하지 않는다. 버전 범위 불일치의 warning 정책은 ADR-0010을 따른다.

`bind`는 현재 Runtime을 수정하지 않는다. 추후 실제 배포에서만 연결 설정을 적용한다.

## Release 선택, 편집, 배포

목표 사용자 흐름:

```text
manafield use A
manafield module bind echo-prod state main-postgres
manafield deploy B
```

- `use A`는 기존 불변 Release A의 YAML을 **수정 가능한 temp 작업본**으로 불러오고 기준 Release를 기록한다. 실행 중인 배포에는 아무 변화가 없다. 저장하지 않은 작업이 있으면 조용히 덮어쓰지 않는다.
- `module bind` 및 추후 설정 편집 명령은 **temp 작업본만 변경**한다. `instance.yaml`을 직접 편집하는 경로도 지원한다. 작업본을 사용 중이라면 CLI 편집과 수동 편집은 같은 작업본을 대상으로 한다.
- `deploy B`에서 B가 **미사용 Release ID**라면, 작업본을 검증하고 B의 불변 YAML로 먼저 저장한 뒤 해당 Release 배포를 시도한다. 배포 실패 후에도 B 스냅샷은 남아 재시도할 수 있다.
- B가 **기존 Release ID**라면 이미 저장된 B를 그대로 재배포한다. 이때 작업본에 미저장 변경이 있다면 이를 묵살하거나 B를 덮어쓰지 않고 충돌 오류를 낸다.
- ID 없이 `deploy`하면 신규 Release ID를 자동 발급하는 방식도 지원 대상으로 둔다. 생성된 ID를 출력한다.
- `use A`는 배포/롤백 명령이 아니다. `deploy A`처럼 기존 Release를 지정해야 실제 목표 구성이 바뀐다.

명령 표기는 **목표 설계**이며, 현재 CLI에 구현돼 있다는 뜻이 아니다. 명시적인 `release create` 명령은 필수가 아니다. 필요한 경우 나중에 '저장만 하고 배포하지 않기' 기능으로 추가할 수 있다.

## 파일 배치와 ID

Instance마다 사유 영역에 다음을 둔다. 사용자 기준 루트는 `<instance-root>`이며, 아래 경로는 **목표 구조**다.

```text
<instance-root>/
└── manafield/
    ├── release/
    │   ├── v1_20261008T070000Z.yaml
    │   └── v2_20261008T080000Z.yaml
    └── temp/
        └── working.yaml
```

- ID 기본형: `v<양의 정수>_<UTC 타임스탬프 YYYYMMDDTHHMMSSZ>` (예: `v2_20261008T080000Z`).
- 숫자는 해당 Instance의 단조 증가 순번이며, 동일 ID를 재사용하거나 덮어쓰지 않는다. 충돌 시 새 순번을 발급한다.
- 작업본은 재실행에도 남을 수 있는 사유 파일이며, 비밀값은 포함하지 않는다. 실제 Secret은 별도 Secret Store/경로로 관리한다.
- Release YAML에는 연결 대상 Instance ID와 구체적 버전/소스 참조를 고정한다. 동작 재현을 위해 빌드 결과의 이미지 digest/manifest가 필요한 경우 별도 불변 메타데이터로 참조할 수 있다. **Release ID는 Platform/Module 버전 번호가 아니다.**
- 스냅샷 작성은 임시 파일과 원자적 rename 등으로 중간 쓰기 파일을 Release로 노출하지 않도록 한다. 성공/실패 배포 이력은 불변 Release YAML 자체를 수정하지 않고 별도 기록한다.

현재 Jenkins의 `<instance-root>/releases/<BUILD_NUMBER>/`는 Compose 배포 산출물 디렉터리다. 목표인 `manafield/release/<id>.yaml`와 동일한 개념이 **아니며**, 이전 구현에서 자동 이전되지 않는다.

## 배포와 Resource 영속성

배포는 지정 Release의 **목표 구성**을 현재 Runtime에 적용하는 작업이다.

- 같은 Resource Instance ID는 기존 PostgreSQL, named volume, DB schema/role 및 데이터를 **기본적으로 재사용**한다. Release 변경으로 파괴·재초기화하지 않는다.
- 첫 설치 시 존재하지 않는 Resource만 생성·등록하고, 이미 있는 Resource는 health/identity를 확인한다. Module 패키지는 독립적으로 만들어지고 설치된다. Platform `build all`은 이 패키지들을 다시 빌드하지 않는다.
- 전체 Release끼리의 일반적인 JSON/YAML diff 기능은 **v0 필수 조건이 아니다**. 그러나 배포 엔진은 현재 상태와 목표를 최소한으로 조정할 수 있어야 한다(기존 Resource 식별, 변경 적용, 멱등성). diff가 필요 없다는 말이 '모든 컨테이너를 지우고 다시 만든다'는 뜻은 아니다.
- 제거 또는 교체로 데이터 손실 가능성이 있는 작업은 기본적으로 자동 수행하지 않는다. 현재 Jenkins의 `docker compose up --remove-orphans`도 이 정책 관점에서 재검토 대상이다.
- 데이터베이스 엔진의 큰 버전 변경, 스키마 데이터 마이그레이션, 백업/복구, 서비스별 재시작 정책은 이 ADR의 범위 밖이다. 다만 데이터 영속성 및 명시적 파괴 승인이라는 공통 경계는 지킨다.

## 실패 시 재시도와 복구의 범위

YAML 스냅샷을 남기면 같은 목표 구성으로 **재배포를 재시도**하거나 이전 Release를 선택할 수 있다. 하지만 YAML만으로 이전 런타임 상태가 즉시 돌아오는 것은 아니다.

예를 들어 일부 컨테이너만 갱신된 상태, 외부 서비스 변경, 데이터베이스에 기록된 최신 데이터는 YAML로 역복원되지 않는다. 필요한 경우 별도 작업 이력, 헬스 검증, 백업/복원 또는 명시적 운영 절차가 필요하다. v0에 자동 데이터 롤백을 약속하지 않는다.

## 이후 작업

- 별도 `manafield` CLI와 `manafield-core` 서버 바이너리 + 공통 관리/배포 코드로 분리
- `build` 명령군을 **Manafield 플랫폼만** 빌드하도록 재설계. 현재 `manafield build WORKSPACE REVISION CORE_IMAGE`는 Jenkins 호환 과도기 기능이며 외부 Module 이미지를 함께 빌드하므로 목표 계약과 다르다
- 작업본 관리(`use`, `module bind`), 불변 Release YAML 저장, `deploy <release-id>`와 Resource reuse 구현
- 기존 Jenkins 파이프라인은 전환 과정의 frontend로 유지; 현행 `plan`/CLI `build`/CLI `deploy RELEASE_DIR`를 새 명령과 혼동하지 않도록 문서화
