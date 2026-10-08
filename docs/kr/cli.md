# Manafield CLI / Docker v0 실행 가이드

> 기준: Manafield v0, 2026-10-08. 현재 구현 범위와 최종 목표를 구분합니다.
>
> 한국어 문서를 우선합니다. 상세한 구조 결정은 [ADR-0013](adr/0013-docker-v0-cli-execution.md)을 참고합니다.

## 작업본 및 Release CLI — 목표 설계 (미구현)

[ADR-0014](adr/0014-instance-working-release.md)의 제안된 사용자 명령 형식은 다음과 같아. **아직 실행 가능한 명령어는 아니야.**

```bash
manafield build all
manafield use v1_20261008T070000Z
manafield module bind echo-prod state main-postgres
manafield deploy v2_20261008T080000Z
manafield deploy v1_20261008T070000Z
```

- `build all`은 **Manafield 플랫폼**만 빌드하며 별도 배포된 Module은 재빌드하지 않음
- `use A`는 기존 불변 YAML을 `<instance-root>/manafield/temp/working.yaml`로 복사해 선택함. 이미 편집한 작업본이 있으면 확인 없이 덮어쓰지 않음
- `module bind A B C`는 A의 Requirement slot B가 대상 Module/Resource Instance C를 가리키도록 temp YAML만 수정함. 즉시 배포나 Resource 복제를 하지 않음
- `deploy B`에서 B가 새 ID면 작업본 검증 → `<instance-root>/manafield/release/B.yaml` 불변 저장 → 배포. 기존 ID이면 저장된 Release를 그대로 재적용하며 미저장 편집이 있을 경우 오류로 처리
- 기본 Release ID는 `v<자연수>_<UTC YYYYMMDDTHHMMSSZ>`이고, Instance 내에서 재사용하거나 덮어쓸 수 없음
- 기존 PostgreSQL과 Volume/스키마/Role/데이터는 Release가 바뀌어도 재사용. YAML 재배포만으로 데이터 백업/복원이 되지는 않음
- Release 간 전체 diff 엔진은 당장 필수가 아니지만, 배포 엔진의 멱등성·기존 Resource 식별·비파괴 변경 처리는 필요함

현재 `manafield build WORKSPACE REVISION CORE_IMAGE`는 Jenkins 호환 과도기 기능으로 **외부 Module까지 빌드**하며, `manafield deploy RELEASE_DIR`는 이미 준비된 Compose 산출물만 실행한다. 위 목표 명령과 섞어 생각하면 안 돼.

## CLI / Core 바이너리 분리

`manafield` CLI와 `manafield-core` HTTP 서버는 서로 다른 실행파일로 동일한 Docker Core 이미지/배포 묶음에 포함됩니다. 서버는 `manafield-core`로 시작하며 CLI 인자 생략 시 도움말을 표시합니다. Jenkins가 `/usr/local/bin/manafield`를 추출하는 기존 경로는 유지합니다.

## 구축 전제조건

지원되는 **전체 Manafield v0 Instance 구축과 배포에는 Docker가 필수**입니다.
정확히는 실행 환경에 Docker CLI, Docker Compose plugin, 접근 가능한 Docker daemon이 필요합니다.
Jenkins는 선택적인 첫 CI/CD frontend이지 Manafield 자체의 설치 조건이 아닙니다.

Core의 독립적인 API / Registry / validation과 `manafield plan`은 Docker 없이도 개발하거나 사용할 수 있습니다.
이 예외는 완전한 Instance 구축·배포를 Docker 없이 지원한다는 뜻이 아닙니다.

## CLI 구현 상태

| 명령 | 현 상태 | 의미 |
| --- | --- | --- |
| `manafield-core` | 구현 | 별도 바이너리로 Core API 서버 실행 |
| `manafield` (인자 없음) | 구현 | CLI 도움말 출력 |
| `manafield health` | 구현 | Core API 상태 조회 |
| `manafield ps` | 구현 | Core 및 등록된 Module/Resource 조회 |
| `manafield resource [id]` | 구현 | Resource 목록 또는 단일 Resource 조회 |
| `manafield plan [INSTANCE]` | 구현 | Instance Definition 검증 및 Build Plan 생성 |
| `manafield deploy RELEASE_DIR` | 구현 | 준비된 release에 한해 Docker Compose 배포 |
| `manafield build WORKSPACE REVISION CORE_IMAGE` | 구현 | Jenkins가 준비한 workspace의 Core/Module/Resource Docker 이미지 빌드 |
| `manafield verify` / `rebuild` | 미구현 | Jenkins에서 단계적으로 이전할 예정 |

## Build Plan 생성

```bash
manafield plan instance.yaml \
  --output build-plan.json \
  --ci-output ci-plan
```

## 준비된 release 배포

```bash
manafield deploy /opt/manafield/instance/releases/123
```

release 디렉터리에는 `release.env`, `compose.yml`,
`modules.compose.yml`, `compose-profiles.txt`가 모두 있어야 합니다.

CLI는 `compose-profiles.txt`에서 지정한 프로필을 반영하고 아래 동작을 수행합니다.

```text
docker compose --env-file RELEASE_DIR/release.env
  --file RELEASE_DIR/compose.yml
  --file RELEASE_DIR/modules.compose.yml
  up -d --no-build --remove-orphans

docker compose [같은 옵션] ps
```

현재 `deploy`는 **이미 빌드된 이미지와 준비된 release**만 사용합니다.
소스 checkout, 이미지 빌드, PostgreSQL binding materialization, release staging, ingress publish, 배포 후 Protocol 검증은 아직 이 명령의 책임이 아닙니다.

## Jenkins와 CLI의 역할

현재 Jenkins의 `Resolve Build Plan`, `Build Images`, `Deploy` stage는 공통 `manafield` CLI를 호출합니다.
Jenkins의 나머지 stage는 migration이 진행 중입니다.

장기적으로 Jenkins는 Git webhook, 승인, Credentials, 로그/이력 같은 CI 기능을 담당하고,
Manafield 고유의 build / materialize / stage / deploy / verify 의미론은 CLI 또는 재사용 가능한 executor가 담당합니다.

Core 서버 컨테이너에 `docker.sock`을 직접 mount하지 않습니다.
CLI의 현재 Docker 호출은 **CLI를 실행하는 호스트/CI 환경**에서 실행되며, Core API 프로세스와 별개입니다.
향후 privileged Runtime Provider 경계는 별도 설계대로 유지합니다.

현재 목표는 Jenkins 없이도 CLI + Docker로 전체 Instance를 재구축할 수 있게 만드는 것이며,
**아직 그 end-to-end 검증이 완료된 상태는 아닙니다.**

## 준비된 workspace의 이미지 빌드

```bash
manafield build "$WORKSPACE" "$CORE_SHA" "$CORE_IMAGE"
```

`module-sources.tsv` (8개 열)과 `ci-plan/resources.tsv` (2개 열, 선택)를 사용해
Core, Module, PostgreSQL Resource Provider 이미지를 Docker로 빌드합니다.
출력 파일은 기존 Jenkins 계약인 `resolved-images.env`와
`resource-providers.tsv`를 그대로 유지합니다.

이 명령은 source checkout, Build Plan 해석, binding materialization,
release staging을 수행하지 않습니다. 현재는 Jenkins가 입력을 준비해야 하므로
CLI만으로 Instance를 처음부터 구축하는 기능은 아직 구현 중입니다.
