# Manafield Core Logging v0

Core의 기본 로깅은 **Core에 내장**되어 있습니다. 외부 Module, DB, Role과 연결되지 않아도 콘솔 출력은 항상 동작해야 합니다.

## 기록 형식

콘솔은 `(warn)[source] 메시지` 형식으로 출력하며, 저장 형식은 JSON Lines입니다.

```text
(info)[core] Manafield Core is listening
(warn)[logging] optional PostgreSQL log sink unavailable; console/file logging continues
(info)[api] Module registered
```

```json
{"timestamp_ms":1791540000000,"level":"info","source":"api","event":"audit.module.registered","message":"Module registered","fields":{"module_id":"manafield-home"}}
```

`level`은 `trace, debug, info, warn, error`, `source`는 이벤트 원본, `event`는 선택적인 기계 판독 ID입니다. **감사(Audit)는 로그 레벨이 아니라 `event=audit.*` 네임스페이스**이며, 일반 운영 로그와 다른 조회 Permission이 필요합니다.

최초 이벤트는 Core의 시작·종료, Module/Resource Registry 등록·제거 등을 포함합니다. Account Core, Manage 등 외부 Go Module에서 발생하는 이벤트를 Core로 수집하는 전송 프로토콜은 별도 작업입니다. 현재 외부 Module의 Docker stdout을 Core가 자동으로 가져오지는 않습니다.

## 기본 저장소

- stdout: 항상 활성
- `MANAFIELD_LOG_FILE`: 설정되면 소유자 전용(0600) JSONL 파일에 append. 읽기 가능한 심볼릭 링크나 타인에게 공개된 파일이면 시작을 거부합니다.
- 공식 Core Docker Compose는 `/var/lib/manafield/logs/core.jsonl`을 설정하고 **`manafield-core-logs` 영구 볼륨**을 사용합니다.
- Core 컨테이너는 root 권한이 없고 볼륨 이외에는 읽기 전용 파일시스템을 유지합니다.
- Docker 로그도 계속 확인할 수 있습니다.

조회는 **호스트 Operator가 Core 컨테이너 안에서 CLI를 실행**하는 방식입니다.

```sh
docker exec manafield-core-1 /usr/local/bin/manafield log
docker exec manafield-core-1 /usr/local/bin/manafield log --level warn --limit 100
docker exec manafield-core-1 /usr/local/bin/manafield log --source api --since 1h
docker exec manafield-core-1 /usr/local/bin/manafield log --audit --json
```

별도의 신뢰된 Operator가 0600 파일에 접근 가능한 경우에는 `manafield log --file /private/core.jsonl`을 사용할 수 있습니다.

**로깅 API는 공개하지 않았습니다.** 특히 현재 Core의 기존 `/modules` 및 `/resources` API만으로 사용자 Identity 인증을 보장하지 않으므로 `GET /logs` 등 우회 가능한 조회 엔드포인트를 제공해서는 안 됩니다. Docker 접근 및 로그 볼륨 접근은 Operator 전용으로 제한해야 합니다.

## 선택적 DB 저장 (Instance Binding)

다음 **두 환경변수를 모두** 설정하면 지정한 PostgreSQL 전용 Schema에 추가 저장합니다.

```text
MANAFIELD_LOG_POSTGRES_DSN_FILE=/run/manafield/logging/postgres-dsn
MANAFIELD_LOG_POSTGRES_SCHEMA=mf_core_logs
```

`DSN_FILE`은 비밀 정보가 담긴 파일의 경로로만 전달하고, **환경변수 값에 비밀번호/DSN을 직접 넣거나 로그에 출력하지 않습니다**. 운영자가 Resource Provider에서 발급한 최소 권한 DB role과 그 role 소유 Schema를 연결해야 합니다. Core는 할당되지 않은 Schema를 생성하지 않으며 해당 Schema 내 `core_log_events` 테이블만 초기화합니다.

DB Mirror는 최대 1024개 bounded channel과 별도 스레드로 동작합니다. DB가 아직 준비되지 않았다면 초기 이벤트를 큐에 유지하고, 연결 성공 후 순서대로 저장합니다. 연결/쓰기 실패는 stdout 및 JSONL 로깅을 막지 않으며, 장기 장애로 큐가 가득 차면 DB 미러 이벤트가 손실될 수 있습니다. 원본은 기존 JSONL이며, 네트워크 오류 직후 재시도한 이벤트는 중복 저장될 수도 있습니다. Instance Definition에서 Core의 `loggingState` 슬롯을 PostgreSQL Resource에 연결하면, Jenkins가 **Core 전용 Schema와 최소 권한 DB Role을 할당**하고 비밀번호로 DSN 파일을 생성해 Core 컨테이너에 읽기 전용으로 마운트합니다. Credential은 Build Plan, Release 환경변수, Jenkins 로그에 들어가지 않습니다.

```yaml
core:
  source:
    repository: https://github.com/eventide-manafield/manafield.git
    ref: main
  bindings:
    loggingState: manafield-postgres
```

`loggingState`를 생략하면 DB 접속 없이 stdout과 JSONL 볼륨으로만 동작합니다. 설정된 target은 활성 PostgreSQL Resource여야 하며 다른 Provider나 비활성 대상을 가리키면 Build Plan 검증이 실패합니다. 현재 DB Binding 자동 프로비저닝 경로는 **Jenkins 릴리스 파이프라인**에 구현되어 있습니다. 수동 `MANAFIELD_LOG_POSTGRES_DSN_FILE`/`SCHEMA` 환경설정도 계속 지원합니다.

운영 중 DB가 연결되지 않거나 아직 Provider가 Schema를 만들지 않았다면, Core는 정상 기동하고 10초 간격으로 Sink 연결을 다시 시도합니다. 연결되지 않은 동안 받은 이벤트는 bounded queue에 보관하고 재연결 시 저장합니다. 기존 JSONL 전체를 검색해서 다시 가져오는 자동 DB 백필 기능은 아직 없습니다. Account Core 등 외부 모듈의 로그인/로그아웃 이벤트는 별도 전달 프로토콜이 없어 자동 저장되지 않습니다.

## Role 기반 열람 정책

로그 열람 정책 함수 `visible_with_permissions`은 Allow만 사용하며 기본 거부입니다.

| Permission | 의미 |
| --- | --- |
| `log.read.account` | 일반 Account 영역 로그 |
| `log.read.manage` | 일반 Manage 영역 로그 |
| `log.read` / `log.read.*` | 일반 로그 전체 |
| `log.audit.read` | 감사 이벤트 |
| `log.*` / `*` | 전체 로그 |

`log.read.*`를 가진 사용자에게도 감사 이벤트는 보이지 않습니다. **Manage의 `/security/login-history`는 별도 Account Core 로그인 감사 이력을 검증된 Identity의 Account Role `log.audit.read` Permission 검사 후 보여주는 읽기 전용 화면으로 구현됐습니다.** 그러나 이는 **Core의 JSONL/`core_log_events`를 보여주는 로그 뷰어가 아니며**, Core 로그의 Role 검증 웹 열람은 아직 미구현입니다. 추후 신뢰된 Identity를 인증한 서버에서 Account Role의 유효 Permission을 조회해 이 정책에 전달하고, 각 요청에서 서버 측 필터링을 적용해야 합니다. CLI는 Role을 가장하지 않으며 로컬 Operator 권한에 의해 접근합니다. Account Core 로그인 이력은 `/manafield-account-core account history`에서 별도로 조회합니다.

## 프라이버시·보존

- password/token/cookie/authorization/credential/DSN 등 명백한 필드 키는 구조화된 레코드에서 제외합니다.
- 일반 `message`나 임의 다른 필드에 개인정보·세션 값이 포함되지 않도록 **이벤트 작성자가 반드시 주의**해야 합니다.
- JSONL 용량/회전/보존기간/DB 이전분 재처리 기능은 v0 이후 작업입니다. 디스크 용량을 운영자가 관리해야 합니다.
- Audit는 **논리적 구분**일 뿐, 현재 저장소가 변경 불가능한 WORM Audit Store는 아닙니다.
