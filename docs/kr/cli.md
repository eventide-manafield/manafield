# Manafield CLI / Docker v0 실행 가이드

> 기준: Manafield v0, 2026-10-08. 현재 구현 범위와 최종 목표를 구분합니다.
>
> 한국어 문서를 우선합니다. 상세한 구조 결정은 [ADR-0013](adr/0013-docker-v0-cli-execution.md)을 참고합니다.

## 작업본 및 Release CLI — 구현 / 미구현 구분

[ADR-0014](adr/0014-instance-working-release.md)의 명령 구조야. **`build all`, `use`, `module bind`, Release ID 기반 스냅샷 저장 및 준비된 산출물 적용은 구현됐어.** **지원되는 v0 구성에서는 CLI가 Module Checkout·이미지 빌드·Compose 생성까지 직접 수행해.** Ingress 공개 Module 등 미지원 기능은 명확히 거부해.

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
- `deploy B`에서 B가 새 ID면 작업본 검증 → `<instance-root>/manafield/release/B.yaml` 불변 저장 → 준비된 산출물 적용 시도. 기존 ID이면 저장된 Release를 그대로 재적용하며 미저장 편집이 있을 경우 오류로 처리
- 기본 Release ID는 `v<자연수>_<UTC YYYYMMDDTHHMMSSZ>`이고, Instance 내에서 재사용하거나 덮어쓸 수 없음
- 기존 PostgreSQL과 Volume/스키마/Role/데이터는 Release가 바뀌어도 재사용. YAML 재배포만으로 데이터 백업/복원이 되지는 않음
- Release 간 전체 diff 엔진은 당장 필수가 아니지만, 배포 엔진의 멱등성·기존 Resource 식별·비파괴 변경 처리는 필요함

현재 `manafield build WORKSPACE REVISION CORE_IMAGE`는 Jenkins 호환 과도기 기능으로 **외부 Module까지 빌드**해. `manafield deploy RELEASE_DIR` 역시 기존 Compose 경로 전용인 반면, 새 `deploy <release-id> --staged-dir DIR`는 Release YAML과 준비된 산출물의 Build Plan을 비교하고 적용해. `--staged-dir`를 생략하면 지원되는 구성은 CLI가 직접 산출물을 준비해.

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

## 확장 가능한 Module CLI (Operator 환경)

Manafield CLI는 Rust Core의 도메인 지식을 늘리지 않고 외부 Module이 새 명령 Namespace를 제공할 수 있도록 **명시적으로 활성화된 CLI 확장 디렉터리**를 지원합니다.

예: `manafield account role list`는 신뢰할 수 있는 `$MANAFIELD_CLI_EXTENSIONS_DIR/manafield-account` 실행 파일에 `role list`를 전달합니다.

```bash
export MANAFIELD_CLI_EXTENSIONS_DIR="$HOME/.local/libexec/manafield"
manafield account role list
```

- 이 환경변수가 설정되지 않은 경우 미등록 명령은 기존처럼 거부합니다.
- 확장은 `manafield-<namespace>` 실행 파일로, 직접 실행하며 별도의 셸 해석이 없습니다. 심볼릭 링크는 거부합니다.
- 확장 디렉터리에는 관리자만 신뢰하는 실행 파일을 배치하세요. **임의의 Module 웹 UI가 CLI 실행 파일을 자동으로 설치하도록 하지 않습니다.**
- 확장은 Core 내부 API가 아니라 **호스트 Operator CLI에서만 실행**됩니다. Docker Runtime 권한은 사용하는 어댑터에 필요한 만큼 별도 부여합니다.
- Account Role v0는 `manafield-account` 어댑터를 통해 컨테이너 내부의 `manafield-account-role role ...` 명령을 사용합니다. 아직 권한 검증이나 임의 명령을 HTTP로 원격 실행하는 API는 제공하지 않습니다.

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
| `manafield deploy [RELEASE_ID] [--instance-root DIR] [--snapshot-only] [--staged-dir DIR]` | 구현 (단계적) | YAML 스냅샷 확정, 준비된 Compose 산출물 일치 검증, Docker 적용 및 Core 헬스 확인 |
| `manafield deploy RELEASE_DIR` | 기존 호환 | 기존 Jenkins 방식의 준비된 Compose 디렉터리 적용 |
| `manafield build WORKSPACE REVISION CORE_IMAGE` | 구현 | Jenkins가 준비한 workspace의 Core/Module/Resource Docker 이미지 빌드 |
| `manafield verify` / `rebuild` | 미구현 | Jenkins에서 단계적으로 이전할 예정 |

## Build Plan 생성

```bash
manafield plan instance.yaml \
  --output build-plan.json \
  --ci-output ci-plan
```

## Jenkins 없는 직접 Instance 배포 — 지원되는 v0 구성

```bash
cd ~/manafield-build
git pull --ff-only

# 테스트용 사유 Instance (운영 Instance와 분리)
mkdir -p ~/manafield-direct-test
cp deploy/instance.bootstrap.yaml ~/manafield-direct-test/instance.yaml

# CLI 직접 실행: Release 자동 발급 → Core/Module 빌드 → Compose 생성 → 적용
cargo run --locked --bin manafield -- deploy \
  --instance-root ~/manafield-direct-test \
  --source ~/manafield-build
```

`--source DIR`는 **Manafield Core 소스 저장소** 경로이며, 생략하면 현재 디렉터리를 사용해. `--modules-root DIR`는 `source.type: dir`인 Module을 `DIR/<module-id>`에서 찾는 옵션이야(기본: `<instance-root>/modules`). Git 기반 Module은 YAML의 URL/ref를 자동 Clone하고, 참조 모듈의 빌드 이미지 태그에 실제 Git 커밋 단축 SHA를 사용해.

이 모드는 `<instance-root>/manafield/staged/<release-id>/`에 Compose 설정, Core·Module 매니페스트, Build Plan, PostgreSQL Binding 파일을 직접 만들고, Docker 이미지를 빌드한 뒤 적용해. PostgreSQL Provider가 1개 있으면 공식 Provider 이미지를 함께 빌드하고, Binding별 schema/role/password 파일을 만들며 기존 password 파일은 재사용해. `release.env`는 0600 권한으로 생성돼.

**현시점의 직접 배포 범위:** Docker Runtime Provider, 비공개(Module Exposure 없음) Git/dir Module, PostgreSQL Resource **최대 1개**. 외부 공개/Ingress 구성 및 PostgreSQL 이외 Provider는 아직 지원하지 않으며 배포 전에 오류로 종료해. 직접 배포에서는 Compose 프로젝트명을 `manafield-<instance-id>`로, 내부 네트워크를 Instance별로 분리해. 같은 Instance의 Release는 같은 볼륨을 재사용하지만, 기존 Jenkins 프로젝트와는 **서로 다른 컨테이너·볼륨**을 사용해. Core의 호스트 포트도 Instance ID로부터 20000~39999 범위에서 결정해 18080 기본 운영 포트와 겹치지 않도록 했어. 원하는 포트가 있으면 `MANAFIELD_CORE_PORT=19189` 환경변수로 배포할 때 지정하면 결과 `release.env`에 보존돼.

새 Release는 Docker 빌드 실패와 관계없이 불변 YAML로 남아. 같은 Release를 재시도하면 유효한 기존 준비 산출물을 재사용하며 데이터 볼륨이나 비밀번호 파일을 지우지 않아. Resource 재사용은 **볼륨과 인증정보를 유지한다는 의미**로, PostgreSQL 데이터/계정 마이그레이션이나 기존 Jenkins 설치의 인증정보 자동 이전을 보증하지 않아. 기존 Jenkins PostgreSQL Volume의 데이터는 새 Instance로 자동 이전되지 않으니 이관 시 별도 절차가 필요해.

Core `/health`까지 통과하면 활성 Release를 기록하지만, 모든 Module/Resource의 등록/실제 가용성은 별도로 확인해야 해. 이 기능은 이미지 digest 잠금, 이전 배포 자동 롤백, Ingress 반영, 파괴적 리소스 제거를 포함하지 않아. **실제 Docker 엔진 통합 검증은 아직 필요해.**

## 불변 Release 확정과 적용 — 단계적 구현

```bash
# 1. 기존 use/bind 작업본을 새 Release로 저장 (Docker 호출 없음)
manafield deploy v2_20261008T080000Z --snapshot-only --instance-root /path/to/instance

# 2. 준비된 산출물에 Build Plan이 맞는 경우에만 실제 배포
manafield deploy v2_20261008T080000Z \
  --instance-root /path/to/instance \
  --staged-dir /path/to/prepared-release

# 3. ID 생략 시 새 vN_UTC타임스탬프 자동 발급
manafield deploy --snapshot-only --instance-root /path/to/instance
```

`--staged-dir`에는 `build-plan.json`, `release.env`, `compose.yml`, `modules.compose.yml`, `compose-profiles.txt`가 모두 필요해. CLI는 저장한 Release에서 Build Plan을 다시 해석하고 JSON 내용이 **일치하지 않으면 Docker를 실행하지 않아**.

신규 ID는 작업본을 검증하고 불변 YAML로 확정한 뒤 적용을 시도해. `--staged-dir`를 지정하면 산출물이 준비되어 있어야 하고, 생략하면 CLI가 직접 준비해. 실패해도 **Release 스냅샷은 남아** 나중에 같은 ID로 재시도할 수 있어. 기존 ID는 작업본에 미저장 변경이 있으면 재배포를 거부해. `--snapshot-only`로 구성만 확정할 수도 있어.

배포 시에는 `docker compose up -d --no-build`, `ps`, Core 컨테이너 내부 HTTP `/health` 확인을 수행하고, 이 단계가 성공해야 `manafield/state/active-release.json`을 갱신해. `last-attempt.json`은 마지막 적용 시도 상태를 기록해. **Module/Resource 전체 헬스 검증과 운영 실패 복구, Ingress 반영은 아직 이 새 경로의 범위 밖**이야.

영속 Resource를 보호하기 위해 새 경로에서는 `--remove-orphans`를 사용하지 않아. **Jenkins의 예전 `deploy RELEASE_DIR` 경로에는 이 변경이 적용되지 않았어.** 그리고 Build Plan 일치는 이미지 digest 고정을 보장하지 않으므로, 모듈 소스 revision 및 이미지 고정은 추가 과제로 남아 있어.

## Manage 바인딩 스냅샷 (Jenkins 배포 연결)

현재 Core Registry의 `/modules`는 선언된 Requirement/Capability를 제공하지만, 실제 활성 Instance의 `modules[].bindings`를 반환하지는 않습니다. CLI는 배포 후보의 검증된 Build Plan에서 **Module ID와 Binding slot→target ID만** 뽑아 별도의 읽기 전용 스냅샷으로 내보낼 수 있습니다.

```bash
manafield bindings export \
  --plan /path/to/release/build-plan.json \
  --output /path/to/release/manage-bindings.json
```

출력 형식:

```json
{
  "modules": {
    "example-module": {"state": "main-postgres"},
    "another-module": {}
  }
}
```

CLI는 **새 파일만 생성**하며, 기존 파일을 임의로 덮어쓰지 않습니다. 출력에는 Module/Resource의 설정, 환경변수, 토큰, 인증정보가 들어가지 않습니다.

기존 Jenkins 배포 경로는 Stage Release에서 **후보 스냅샷**을 만들고, Deploy 및 Verify Deployment가 성공해 모든 예정 Module이 Registry에 등록된 뒤에야 `$INSTANCE_ROOT/manage-assets/bindings.json`으로 원자적으로 게시합니다. 검증 실패 시 후보 스냅샷을 게시하지 않습니다. 디렉터리 자체를 `manafield-manage-web` 컨테이너에 read-only로 마운트하므로 파일을 원자적으로 교체해도 Manage가 재시작 없이 최신 내용을 읽습니다.

`custom.css`는 같은 **Instance 전용** `manage-assets` 디렉터리에서 선택적으로 제공하며 공개 GitHub 저장소에 포함하지 않습니다.

> **주의:** CLI 명령 자체는 Build Plan의 데이터를 투영할 뿐, 그 Release가 실제 활성인지 판단하지 않습니다. **활성 상태 게시 시점은 배포 실행 주체(Jenkins)가 검증 이후에 결정해야 합니다.** 또한 공개 웹에 Manage를 노출하면 모듈 ID, 바인딩 대상 및 버전 등 인스턴스 구조가 공개될 수 있으므로 배포 범위와 접근 정책을 고려해야 합니다.

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

레거시 `deploy RELEASE_DIR`는 **이미 빌드된 이미지와 준비된 release**만 사용합니다.
이 레거시 경로는 checkout/빌드/Binding 준비를 수행하지 않습니다. 별도의 Release-ID 직접 배포 경로는 위에서 설명합니다.

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
