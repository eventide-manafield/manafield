# Image-only Instance release — 2026.10.22-image-instance.1

glibc 기반 Linux amd64에서 게시된 이미지만 Pull하여 단일 Instance를 배포한다. CLI 릴리스는 `2026.10.22-network-bindings.1`, 원본 커밋은 `60214d2c15061855ab4c9dac993642f2db8244f1`이다. CLI 바이너리나 Rust 코드를 변경하거나 다시 빌드하지 않는다. 운영 호스트에는 Docker Engine/Compose와 Python 3만 필요하다. 필요한 Registry 읽기 인증은 호스트의 Docker credential store로 제공한다. 소스, Rust, Cargo, npm, Jenkins는 필요하지 않다.

## 실행

배포 패키지를 압축 해제하거나 GHCR 패키지 이미지의 `/release`에서 추출한 뒤 실행한다:

```
./deploy.sh --instance-id example --instance-root "$PWD/instance-example" --reference-port 8080
```

이 명령은 최초 실행에 빈 Instance root를 요구한다. 비밀 파일을 실행 호스트에서 생성하고 digest 고정 staged release를 작성한 다음, `docker compose pull`과 기존 CLI의 `manafield deploy /absolute/instance-example/staged`만 실행한다. 이미지 build, 소스 clone/fetch, 운영 설정 추측, 기존 서비스 접근은 수행하지 않는다. 같은 인자로 재실행하면 기존 staged 파일과 비밀번호·DB 볼륨을 재사용한다. 다른 내용으로 기존 staged release를 덮어쓰지 않는다. 실패한 배포의 컨테이너·DB를 자동 삭제하지 않는다.

배포 없이 준비만 하려면:

```
python3 prepare.py --instance-id example --instance-root "$PWD/instance-example" --reference-port 8080
```

준비한 디렉터리를 수동으로 적용하는 동등한 명령:

```
STAGE="$PWD/instance-example/staged"
env -u COMPOSE_PROJECT_NAME -u COMPOSE_FILE -u COMPOSE_PROFILES \
  docker compose --env-file "$STAGE/release.env" -f "$STAGE/compose.yml" -f "$STAGE/modules.compose.yml" pull
env -u COMPOSE_PROJECT_NAME -u COMPOSE_FILE -u COMPOSE_PROFILES \
  ./manafield-linux-amd64 deploy "$STAGE"
```

수동 실행에서는 `MANAFIELD_*`, `COMPOSE_*` 환경변수로 고정 설정을 덮어쓰지 않아야 한다. `deploy.sh`는 관련 상속 환경변수를 제거하고 적용할 이미지 digest와 build 항목 부재를 검사한다. `deploy --source`는 빌드 모드이므로 사용하지 않는다. Release-ID + `--staged-dir` 모드는 staged Build Plan과 immutable Release가 일치해야 하며 apply 후 Core 내부 curl을 호출하므로 shell/curl 없는 현재 Core에 그대로 사용하지 않는다. 기존 디렉터리 배포 경로는 curl을 요구하지 않으며, 이 패키지는 Core/Reference의 native healthcheck와 Provider readiness를 사용한다. CLI를 새로 구현할 필요가 없는 이유다.

GitHub checkout으로 파일을 확보한 경우에도 같은 명령을 쓴다. CLI 실행 파일이 없으면 prepare.py가 lock에 지정된 CLI 이미지만 Pull하여 바이너리를 추출하고 SHA256을 확인한다. 소스를 빌드하지 않는다. 배포 패키지에는 검증된 CLI 실행 파일이 이미 포함된다.

## 파일과 고정 이미지

`images.lock.json`은 CLI, Core, Reference, PostgreSQL 18-alpine, Provider의 Registry digest를 고정한다. 네 가지 runtime service의 Compose에는 build 항목이 없다. CLI/Provider는 `2026.10.22-network-bindings.1`, Core/Reference는 기존 보안 개선 이미지의 digest를 그대로 사용한다. 원본 Git ref는 provenance metadata이며 실행 중 checkout하지 않는다.

`compose.yml`, `modules.compose.yml`, `release.env.example`, `compose-profiles.txt`, `instance.example.yaml`, `reference.module.json`이 비밀 값 없는 staged template이다. prepare.py가 아래 완성 파일을 만든다:

- `<instance-root>/instance.yaml`: 실제 Instance ID의 Definition.
- `<instance-root>/staged/{compose.yml,modules.compose.yml,release.env,compose-profiles.txt,images.lock.json}`: 기존 CLI 디렉터리 배포 입력.
- `staged/{instance.release.yaml,build-plan.json,release-manifest.json}`: Definition snapshot, 기존 CLI가 해석한 Build Plan, 고정 이미지/Instance identity.
- `staged/modules/manafield-reference/manafield.module.json`: Core가 읽는 공개 메타데이터.
- `staged/secrets/postgres-admin`: 실행 호스트에서 생성한 0600 비밀 파일. 패키지나 Git에는 포함하지 않는다.
- `staged/postgresql-allocations.tsv`: 비어 있는 할당 manifest. Reference는 DB Requirement가 없으므로 자동 Binding하거나 DB 자격증명을 전달하지 않는다.
- `<instance-root>/deployment-state.json`: 네 서비스의 health 확인 후 기록. CLI의 Release-ID active-state 갱신을 대신했다고 주장하지 않는다.

Instance별 Compose 프로젝트 `manafield-<id>`와 네트워크 `manafield-modules-<id>`를 사용한다. 첫 준비 때 동일 프로젝트가 이미 존재하면 원래 Instance root를 사용하도록 거부한다. legacy CLI의 `--remove-orphans`는 이 Instance 프로젝트 범위이므로 다른 프로젝트를 재사용하지 않는다. DB는 프로젝트별 named volume에 보존하고, Core 로그는 이 범용 예제에서 tmpfs이다. Reference는 `127.0.0.1:<reference-port>`에만 노출한다. Core·DB에는 host port가 없다. 외부 ingress와 데이터 보존/백업 정책은 운영 담당자의 별도 범위다.

Provider UID/GID는 마운트되는 할당 파일 소유자로 설정한다. rootless/userns Docker에서 호스트와 컨테이너의 UID 매핑이 다르면 운영 담당자가 마운트 파일의 실제 컨테이너 내 소유자와 일치시키고 적용해야 한다. 이 패키지는 운영 호스트의 UID, 경로, ingress, DNS를 가정하지 않는다.

이 템플릿은 제시한 단일 Instance 예제다. 임의 Module/Resource를 배포하는 새 범용 resolver를 구현한 것은 아니다. 추가 Module의 Requirement/Binding과 실제 allocation은 기존 CLI materializer를 개발 환경에서 사용해 준비하고, 게시 이미지와 완성 staged 파일을 별도 릴리스로 제공한다. 호환 Resource가 존재한다는 이유로 Binding하지 않는다.

## Git 원격 상태

원본 릴리스 커밋 60214d2는 이 작업 시작 시 GitHub 조회에서 422(원격 미반영)였다. 이 커밋을 조상으로 포함한 별도 `feat/image-only-instance-release-20261022` 브랜치를 원격에 반영하고 원본 SHA의 GitHub 조회 성공을 확인한다. main 병합이나 main 직접 수정은 하지 않는다. 최종 확인 결과와 소스 HEAD는 배포 패키지의 `git-remote-state.json`에 기록한다.

확인 URL:
- https://github.com/eventide-manafield/manafield/commit/60214d2c15061855ab4c9dac993642f2db8244f1
- https://github.com/eventide-manafield/manafield/tree/feat/image-only-instance-release-20261022/release/image-instance

## Studio에서 패키지 제작

기존 CLI/Provider/Reference/Core를 재빌드할 필요가 없다. `package.py`가 digest의 원격 존재를 확인하고 게시 CLI에서 실행 파일을 추출하여 운영 비밀 값 없는 패키지와 checksum을 작성한다. `package.Dockerfile`은 배포 산출물을 담는 scratch 이미지이며 런타임 이미지를 빌드하지 않는다. Jenkins 없이 동일한 Python/Docker 명령으로 제작한다. 운영 담당자는 완성된 패키지만 확보한다.
