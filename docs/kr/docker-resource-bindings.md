# Docker Instance 네트워크와 PostgreSQL Binding

Module 개발자는 `manafield.module.json`에 `requires.capabilities`를 선언하고, Instance 운영자는 `instance.yaml`에서 Resource를 활성화한 뒤 `modules[].bindings.<slot>: <resource-instance-id>`를 명시한다. 호환 후보가 한 개여도 자동 Binding하지 않는다.

직접 CLI 배포는 이미 `manafield-<instance-id>` Compose 프로젝트와 `<deployment.modulesNetwork>-<instance-id 소문자>` 네트워크를 생성하고 Core, Module, 관리형 PostgreSQL DB·Provider를 연결한다. Jenkins도 동일한 이름 규칙을 사용한다. 개발자가 Module용 Compose/network 설정을 작성할 필요는 없다. edge는 별도 external 네트워크이며 Core/DB는 연결하지 않는다. ADR-0008의 내부 plane은 신뢰 경계 구분이다. Docker `internal: true`를 강제하거나 Module의 인터넷 outbound를 차단한다는 의미가 아니다.

제한적인 umask에서도 non-root Core가 읽을 수 있도록 공개 Module descriptor 디렉터리는 0755, 파일은 0644로 기록한다. Binding 비밀 파일은 private 호스트 디렉터리 아래 0444로 유지하여 non-root Module의 읽기 전용 bind mount에서 읽을 수 있게 하며 관리자 비밀번호는 0600을 유지한다. Provider는 할당 파일을 생성한 UID/GID로 실행한다. 따라서 `cap_drop: ALL`에서도 0700 호스트 비밀 디렉터리와 0600 manifest를 읽을 수 있으며 root의 DAC 우회 권한을 추가하지 않는다. 사용자 namespace를 별도 매핑하는 Docker 환경은 마운트 파일의 컨테이너 내 소유 UID/GID에 맞춰 Provider UID/GID를 설정해야 한다.

명시적 PostgreSQL Binding에만 기존 materializer가 host/port/database/schema/username과 `MANAFIELD_BINDING_<SLOT>_SECRET_PASSWORD_FILE`을 주입한다. 비밀번호는 Instance의 private 디렉터리에 보존하며 해당 Module에 읽기 전용 마운트한다. Module은 이 계약을 읽는 PostgreSQL 드라이버 코드를 제공해야 한다. 배포 시스템이 임의의 앱 설정 형식을 추측하지 않는다.

PostgreSQL에 Binding된 Module은 Provider healthcheck를 기다린다. Provider는 할당 manifest의 모든 역할·스키마 구성이 성공하고 Core 등록을 확인한 뒤 tmpfs에 준비 표시를 만든다. 실패 시 준비되지 않은 상태를 유지한다. 새 Release는 새 manifest 경로를 사용하므로 기존 Compose 동작에 따라 Provider가 재생성되고 새 할당을 처리한다. Binding 없는 Module에는 DB 접속정보나 비밀 파일을 전달하지 않는다. 이 시작 순서 보장은 실행 중 DB 장애 시 Module을 자동 중단하거나 전체 복구를 수행하는 정책은 아니다.

관리형 Resource의 생성·연결은 Instance Definition과 CLI/Jenkins 배포 책임이다. Core의 `/resources` 등록 API만 호출해 임의의 외부 DB/컨테이너를 등록하는 것으로 Docker 연결을 변경하지 않는다. 현재 직접 배포는 PostgreSQL Resource 한 개 및 외부 노출 없는 Module까지 지원한다. 다중 Resource, 외부 DB 관리 어댑터는 이번 변경에 포함하지 않는다.

## Nexus 반영

Studio에서 이 브랜치의 CLI 및 변경된 PostgreSQL Provider를 새 태그로 빌드·검증하고 게시 승인 후 Registry에 게시한다. 기존 릴리스 Provider는 준비 표시를 만들지 않으므로 새 healthcheck와 함께 사용할 수 없다. Core 런타임을 재빌드할 필요는 없다. 새로운 CLI, Compose 템플릿, Provider를 함께 반영한 staged release를 Studio에서 준비하고, Nexus에는 이미지 digest와 완성된 배포 산출물만 전달한다. Nexus에서 `--source` 직접 빌드 경로를 실행하지 않는다. staged build-plan은 immutable Release와 같아야 하고 경로·secret mount·Core healthcheck는 Nexus 및 선택한 런타임에 맞아야 한다. 현재 CLI의 새 Release apply 후 health 확인은 curl을 호출하므로 curl 없는 보안 강화 Core 이미지에는 그대로 사용할 수 없다. 해당 이미지를 쓸 때는 준비된 Core healthcheck를 `/usr/local/bin/manafield-healthcheck`로 지정하고, 배포 승인 후 `docker compose --env-file <release>/release.env -f <release>/compose.yml -f <release>/modules.compose.yml --profile postgresql up -d --no-build`를 사용한다. 기존 릴리스의 수명주기 기록과 검증 절차도 별도로 유지한다. 이 호환성 제한은 이번 네트워크·Binding 개선에서 변경하지 않았다.

Jenkins의 기존 고정 프로젝트/네트워크에서 새 Instance별 프로젝트로 전환하면 새 DB 볼륨을 만들 수 있다. 기존 Studio/Nexus에 그대로 적용하지 않는다. 이전 프로젝트·DB 볼륨·네트워크 소유권과 백업/이관을 별도 검토한 뒤 승인받아 적용한다. 이번 작업은 운영 파일, 기존 개발 컨테이너·DB·볼륨을 변경하지 않는다.
