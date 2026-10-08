# Manafield CLI / Docker v0 실행 가이드

> 기준: Manafield v0, 2026-10-08. 현재 구현 범위와 최종 목표를 구분합니다.
>
> 한국어 문서를 우선합니다. 상세한 구조 결정은 [ADR-0013](adr/0013-docker-v0-cli-execution.md)을 참고합니다.

## 작업본 및 Release CLI — 구현 / 미구현 구분

[ADR-0014](adr/0014-instance-working-release.md)의 명령 구조야. **`build all`, `use`, `module bind`는 구현됐고, Release ID 기반 `deploy`는 미구현이야.**

```bash
manafield build all
manafield use v1_20261008T070000Z
manafield module bind echo-prod state main-postgres
manafield deploy v2_20261008T080000Z
manafield deploy v1_20261008T070000Z
```

- `build all`은 **Manafield 플랫폼**만 빌드하며 별도 배포된 Module은 재빌드하지 않음
- `use A`는 기존 불변 YAML을 `<instance-root>/manafield/temp/working.yaml`로 복사해 선택함. 명시적인 `use`로 전환하면 기존 미저장 편집을 폐기함
- `module bind A B C`는 A의 Requirement slot B가 대상 Module/Resource Instance C를 가리키도록 temp YAML만 수정함. 즉시 배포나 Resource 복제를 하지 않음
- `deploy B`에서 B가 새 ID면 작업본 검증 → `<instance-root>/manafield/release/B.yaml` 불변 저장 → 배포. 기존 ID이면 저장된 Release를 그대로 재적용하며 미저장 편집이 있을 경우 오류로 처리
- 기본 Release ID는 `v<자연수>_<UTC YYYYMMDDTHHMMSSZ>`이고, Instance 내에서 재사용하거나 덮어쓸 수 없음
- 기존 PostgreSQL과 Volume/스키마/Role/데이터는 Release가 바뀌어도 재사용. YAML 재배포만으로 데이터 백업/복원이 되지는 않음
- Release 간 전체 diff 엔진은 당장 필수가 아니지만, 배포 엔진의 멱등성·기존 Resource 식별·비파괴 변경 처리는 필요함

현재 `manafield build WORKSPACE REVISION CORE_IMAGE`는 Jenkins 호환 과도기 기능으로 **외부 Module까지 빌드**하며, `manafield deploy RELEASE_DIR`는 이미 준비된 Compose 산출물만 실행한다. 위 목표 명령과 섞어 생각하면 안 돼.

## Release 작업본 선택 — Docker 불필요

```bash
manafield use v1_20261008T070000Z --instance-root /path/to/instance
manafield use --instance-root /path/to/instance
```

`--instance-root`를 생략하면 환경변수 `MANAFIELD_INSTANCE_ROOT`, 그것도 없으면 현재 디렉터리를 Instance root로 사용해. 읽는 Release는 `manafield/release/<id>.yaml`이고 선택한 내용은 `manafield/temp/working.yaml`, 기준 정보는 `manafield/temp/context.json`에 저장해.

명시적 `use A`는 기존 미저장 작업본을 버리고 A를 불러오는 작업이야. 다만 지정한 Release가 없거나 YAML이 잘못됐으면 이전 작업본을 유지해. `use`만 실행하면 선택 상태와 수정 여부를 조회해. 이 단계에서는 **실제 Module/Resource 또는 Docker를 전혀 변경하지 않아.**

이제 `module bind`는 `use` 없이도 기존 작업본 → `manafield/state/active-release.json`의 마지막 배포 Release → 초기 `instance.yaml` 순서로 작업본을 선택하거나 생성해. 없으면 오류를 반환하고 빈 YAML을 만들지 않아. 현재는 `active-release.json`에 `{"release_id":"vN_YYYYMMDDTHHMMSSZ"}` 형식을 사용할 예정이며, 해당 상태 파일을 실제 배포 성공 시 쓰는 부분은 아직 미구현이야.

## Module Binding 수정 — Docker 불필요

```bash
manafield module bind echo state main-postgres --instance-root /path/to/instance
manafield use --instance-root /path/to/instance
```

`module bind A B C`는 Instance YAML의 `modules[].bindings.B: C`를 **temp 작업본에서만** 바꿔. 대상 C는 구성에 포함된 활성 Module 또는 Resource Instance ID여야 하고, Module A는 정확히 하나 존재해야 해. B는 `[A-Za-z_][A-Za-z0-9_]*` 형식이야.

- 미선택 시 temp가 있으면 계속 사용하고, 없으면 마지막 배포 Release, 그것도 없으면 `<instance-root>/instance.yaml`에서 작업본 생성
- 미존재/비활성 대상이나 잘못된 YAML은 오류이며 새 temp 작업본을 생성하지 않음
- 작업본은 `manafield/temp/working.yaml`, 상태는 `manafield/temp/context.json`에 저장
- 여러 번 실행해도 기존 Binding을 유지하며, 실행 중인 Module/Resource는 바뀌지 않음
- 현 단계는 YAML 파싱/직렬화 방식이라 **원본 작업본의 주석과 서식은 보존되지 않을 수 있어**. 수동 편집이 필요하다면 결과를 확인해줘
- Requirement의 Capability 버전/적합성 검사는 이 명령만으로 완결되지 않으며 후속 검증 및 배포 로직에서 수행해야 해

## 플랫폼 전체 빌드 — Jenkins 없이 실행 가능

소스 저장소 루트에서 실행하면 CLI, Core, Build Plan 호환 도구, Traefik ingress adapter **4개 실행파일**을 생성하고, Docker가 준비돼 있으면 Core/공식 PostgreSQL Provider 이미지를 빌드해.

```bash
cargo run --bin manafield -- build all
# 또는 먼저 CLI를 만든 뒤:
cargo build --locked --bin manafield
./target/debug/manafield build all
```

생성 결과는 기본적으로 `dist/platform/<버전>-<Git 커밋>` 아래 `bin/`과 `manifest.json`으로 묶여. `manifest.json`에는 플랫폼 버전, 소스 리비전, dirty 여부, 빌드된 Docker 이미지 태그가 기록돼. **이미지는 Docker daemon에 남으며 dist 안에 이미지 데이터가 들어가지는 않아.**

```bash
manafield build all --no-docker --output ./dist/local-binaries
manafield build all --source /path/to/manafield --output /tmp/manafield-test
```

출력 경로가 이미 존재하면 덮어쓰지 않고 오류로 종료해. `--no-docker`는 Rust 실행파일만 만들고 Docker daemon 접근은 요구하지 않아. 기본 실행은 Docker CLI/daemon 접근 권한이 필요해. Instance YAML, Module 이미지, DB 데이터는 다루지 않아.

현재 Jenkins의 기존 `build WORKSPACE REVISION CORE_IMAGE` 호출은 호환을 위해 유지하지만, 새 플랫폼 `build all`의 책임과는 분리돼 있어.

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
| `manafield module bind A B C [--instance-root DIR]` | 구현 | temp YAML의 Module Requirement Binding 설정; 실행 환경 변경 없음 |
| `manafield use [RELEASE_ID] [--instance-root DIR]` | 구현 | 불변 Release YAML을 작업본으로 선택하거나 현재 선택 상태 확인; 기존 미저장 temp는 명시적 전환 시 폐기 |
| `manafield build all [--source DIR] [--output DIR] [--no-docker]` | 구현 | Manafield 플랫폼 실행파일 배포 묶음 생성. 기본값은 Docker 이미지(Core/공식 PostgreSQL Provider)도 빌드 |
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
