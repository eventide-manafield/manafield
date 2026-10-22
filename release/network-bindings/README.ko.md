# Manafield CLI + PostgreSQL Provider 2026.10.22-network-bindings.1

플랫폼은 linux/amd64, CLI 애플리케이션 버전은 0.0.1이다. 이 릴리스 식별자는 crate 버전과 별개로 배포 변경 묶음을 식별한다. 원본 구현 검증 커밋은 `66257cf37b8dd250f275d296d73a0a7fb1b1055e`이며 최종 소스 커밋과 이미지 digest는 `images.lock.json`에 기록한다.

## 변경사항과 호환성

- Jenkins에도 직접 CLI와 같은 Instance별 Compose 프로젝트 및 modules network 이름을 적용한다.
- 명시적으로 PostgreSQL에 Binding된 Module만 DB 역할·스키마 할당과 Core 등록이 완료된 Provider를 기다린다. 자동 Binding하지 않는다.
- 제한적인 umask와 non-root Module의 파일 접근을 지원하고, Provider UID/GID를 private 할당 파일의 소유자에 맞춘다. 숫자 UID로 실행하는 Provider의 스크립트 읽기 권한 및 DB readiness 인자를 보완한다.
- CLI와 Provider의 런타임·배포 소스는 기존 통합 검증 커밋과 동일하다. 최초 배포, Module 추가 배포, SQL 읽기·쓰기, 미지정 Binding 유지 및 비밀번호 재사용 결과를 재사용한다. 기존 통합 시나리오를 다시 실행하지 않는다.
- 릴리스 포장은 pinned builder/runtime 베이스와 OS 보안 패치를 적용한다. CLI 이미지는 non-root이며 shell, Docker CLI, Git, Docker socket을 포함하지 않는다. Provider 이미지는 기존 curl/psql 기반 계약을 유지하며 readiness healthcheck를 기본 제공한다.

## 사용 방법

Module의 `requires.capabilities.<slot>`에 `database.postgresql` 계약을 선언하고 Instance Definition의 `modules[].bindings.<slot>`에 Resource Instance ID를 명시한다. 같은 Instance의 관리형 PostgreSQL Resource를 활성화한다. Module은 `MANAFIELD_BINDING_<SLOT>_ENDPOINT_HOST`, `_ENDPOINT_PORT`, `_CONFIG_DATABASE`, `_CONFIG_SCHEMA`, `_CONFIG_USERNAME`, `_SECRET_PASSWORD_FILE`을 소비한다. 비밀번호 파일은 읽기 전용으로 전달된다. 네트워크는 배포 시스템이 설정한다.

CLI·Provider·Compose 템플릿은 이 릴리스의 묶음을 함께 사용한다. 이전 Provider는 준비 표시를 만들지 않으므로 새 healthcheck와 혼용하지 않는다. `deploy/compose.yml`에 Provider 이미지 digest를 지정하고 `MANAFIELD_POSTGRES_PROVIDER_UID/GID`는 실제 마운트 파일 소유자와 일치시킨다. 호스트의 private 디렉터리 권한을 넓히지 않는다. 사용자 namespace 매핑이 있는 환경은 컨테이너 안에서 보이는 UID/GID를 기준으로 설정한다.

`manafield-linux-amd64`는 Linux amd64/glibc 환경용 독립 CLI 실행 파일이다. `--version`, `--help`, `plan`, `module bind` 등의 기존 인터페이스를 유지한다. 빌드·배포 명령은 실행 호스트에 Docker CLI/Compose, Buildx, Git 및 해당 명령에 필요한 권한이 있어야 한다. 컨테이너 이미지 자체가 Docker 실행 환경을 제공하는 것은 아니다. CLI 이미지의 단순 조회 예:

```
docker run --rm --network none ghcr.io/eventide-manafield/manafield-cli:2026.10.22-network-bindings.1 --version
./manafield-linux-amd64 --help
```

Core 이미지는 이 릴리스에 포함하지 않는다. 기존 CLI의 staged apply 후 health 검사는 curl을 호출하므로 Core 런타임의 curl 제공 여부를 운영 담당자가 확인해야 한다. 이 릴리스에서 이를 변경하지 않는다. 기존 고정 프로젝트에서 Instance별 프로젝트로 전환할 경우 데이터 볼륨과 네트워크 소유권을 검토해야 한다. 배포·데이터 이관은 별도 작업이다.

## Jenkins 없이 릴리스 제작

깨끗하게 커밋된 소스 checkout, Docker Engine/Buildx(OCI image store), Python 3, Trivy 및 GHCR 쓰기 인증이 필요하다. 비밀 값은 빌드 컨텍스트나 명령 인자로 전달하지 않는다. 기존 로그인 credential store를 사용한다. 이미지 태그가 이미 다른 내용을 가리키면 덮어쓰지 않는다.

```
./release/network-bindings/release.sh build --output /absolute/artifacts --evidence /absolute/integration-evidence.json
./release/network-bindings/release.sh audit --output /absolute/artifacts --trivy /absolute/trivy
./release/network-bindings/release.sh publish --output /absolute/artifacts
./release/network-bindings/release.sh export --output /absolute/artifacts
```

보관된 최신 Trivy DB를 재사용할 때만 audit에 `--cache /absolute/trivy-cache`를 추가한다. 기존 통합 검증 JSON의 원본 SHA가 맞고 runtime/deployment 소스가 변경되지 않은 경우에만 재사용한다. CLI/Provider 이미지에는 source/revision/version OCI label이 포함된다. 빌드 컨텍스트는 소스·Cargo 파일·bootstrap manifest 또는 Provider 스크립트만 allowlist로 복사한다. audit는 취약점·Secret Scan 및 모든 레이어와 저장된 ENV/Label/History를 검사하고 Critical/High 또는 secret 후보가 있으면 게시를 중단한다. publish는 linux/amd64 manifest를 게시한 뒤 Registry 내용과 로컬 manifest를 대조하여 digest를 기록한다. 공개 범위를 변경하거나 운영 서비스를 배포하지 않는다.

Jenkins에서도 같은 네 명령을 같은 순서로 실행한다. Trivy와 GHCR credential은 Jenkins agent가 제공하며 이 파이프라인은 인증정보를 이미지에 넣지 않는다. 컨트롤러 전용 API나 Jenkins 환경변수에 의존하지 않는다.

## 산출물

`images.lock.json`, native CLI, source archive/commit 보존 Git bundle, base main부터의 변경 patch, 사용 문서, 재사용한 통합 검증 결과, 호환성·보안 결과 및 SHA256SUMS를 제공한다. release archive에는 운영용 비밀번호·secret 파일·Instance 설정을 포함하지 않는다. source archive는 소스 열람용이다. 원본 커밋 identity를 유지해 재빌드하려면 base main을 포함한 clone에서 `git fetch /absolute/source.bundle HEAD` 후 lock의 source SHA를 checkout한다. 변경 patch는 base `34672cc870482d917ffe6444b6d06b645c14a422`에 `git am`으로 적용할 수 있다. OS 저장소의 보안 패치 시점과 Trivy DB 버전에 따라 재빌드된 이미지 digest는 달라질 수 있으므로 전달·사용에는 lock의 Registry digest를 기준으로 한다.
