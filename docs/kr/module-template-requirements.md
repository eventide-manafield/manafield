# Module Template Requirements v0

> 이 문서는 현재 `manafield-reference` 구현에서 추출한 **초기 Module Template 요구사항**입니다.
>
> Module Protocol 자체의 규격과 특정 언어/Framework Template의 편의 규칙을 구분합니다.
> 아직 pre-alpha이므로 실제 두 번째/세 번째 Module 구현을 거치며 조정될 수 있습니다.

## 1. 목적

Module Template은 새 Module이 Manafield Core 전체를 clone하거나 내부 구현에 의존하지 않고 시작할 수 있는 최소 골격을 제공합니다.

Template은 다음을 목표로 합니다.

- Protocol contract를 빠르게 만족
- 빌드/실행 가능한 기본 구조 제공
- Health 확인 가능
- Core와 구현 언어를 분리
- 특정 Reference Module의 기능을 불필요하게 복제하지 않음

> **The protocol is the contract.**

Template은 Manafield Core SDK를 필수 의존성으로 만들지 않습니다.

## 2. 모든 Module에 필요한 계약

### 필수

현재 Module discovery 기준으로 Module Repository에는 다음 Descriptor를 제공해야 합니다.

```text
manafield.module.json
```

Descriptor는 최소한 다음 정보를 포함합니다.

- `id`
- `name`
- `version`
- `operations`

검색/목록 UI를 위해 다음 표시용 metadata를 선택적으로 둘 수 있습니다.

- Module `description`
- Operation `description`

Description은 nullable/optional이며 dependency 또는 compatibility 판정에는 사용하지 않습니다.

각 Operation은 현재 지원되는 Binding과 Codec 규칙을 따라야 합니다.

HTTP Binding의 현재 검증 규칙:

- path는 `/`로 시작
- 지원 method는 현재 Core 구현 범위에 맞아야 함
- 최소 하나 이상의 Codec 선언
- Input / Output은 Manafield `DataSchema`로 표현

### Health Operation

`healthOperation`은 Protocol상 선택 사항이지만, 일반 실행형 Module Template에는 기본 제공을 권장합니다.

권장 초기 형태:

```text
GET /manafield/health
```

단, 이 경로 자체가 Protocol의 강제 규칙은 아닙니다. 실제 계약은 Descriptor의 Operation Binding입니다.

Health Operation을 선언할 경우:

- 실제 `operations` 안에 존재해야 함
- `input`은 `null`이어야 함

## 3. Build Contract

초기 CI/CD는 Repository root의 Dockerfile 또는 Instance Definition에서 명시한 Dockerfile을 Module build contract로 사용합니다.

따라서 Container 기반 Template은 다음을 만족해야 합니다.

- CI가 source repository만으로 image를 build 가능
- runtime image가 Module 자체 실행에 필요한 파일을 포함
- 실행 port를 Instance/Runtime에서 지정 가능
- 가능한 경우 non-root user 사용
- application-level Health endpoint 제공

Dockerfile은 Manafield Core source를 필요로 하지 않아야 합니다.

## 4. Runtime Configuration

다음은 **Protocol 필수 항목이 아니라 초기 Container Template convention**입니다.

### `PORT`

HTTP server를 실행하는 Template에서는 `PORT` 환경변수로 listen port를 override할 수 있도록 권장합니다.

예:

```text
PORT=8080
```

### `MANAFIELD_CORE_URL`

Core API를 직접 사용할 필요가 있는 Module에만 선택적으로 사용합니다.

```text
MANAFIELD_CORE_URL=http://core:8080
```

모든 Module이 Core URL을 알아야 하는 것은 아닙니다.

장기적으로 Core 호출 방식이 Runtime/Service Discovery 모델로 발전할 수 있으므로 Template 필수 환경변수로 고정하지 않습니다.

## 5. Web Exposure는 Template 계약이 아님

Module Template은 public hostname 또는 public path를 하드코딩하지 않습니다.

다음 정보는 Module Repository가 아니라 private Instance Definition에서 결정합니다.

```yaml
exposure:
  type: host
  host: example.manafield.studio
  targetPort: 8080
```

따라서 Template에는 다음을 넣지 않습니다.

- 특정 `*.manafield.studio` hostname
- Traefik / nginx 전용 route
- Instance-specific TLS 설정
- Cloudflare Tunnel 설정

Module은 자신의 HTTP server가 내부 port에서 정상 동작하도록만 구성합니다.

자세한 경계는 ADR-0009를 참고합니다.

## 6. Template 계층과 구현 Profile

공통 Module Template은 특정 Framework 하나를 표준으로 강제하지 않습니다.

```text
Module Template Contract
├─ Go Web profile
├─ TS / React Web profile
├─ Java / Spring Web profile
└─ future implementation profiles
```

공통 Contract는 Protocol, build/runtime boundary, Health, optional description metadata, dependency/resource declaration 규칙을 정의합니다.

각 구현 Profile은 같은 Contract를 해당 언어/Framework에서 빠르게 만족하기 위한 골격만 제공합니다.

### Go Web profile 후보

경량 HTTP/Web Module과 Official Web Shell에 우선 검토합니다.

권장 후보:

- Go single executable
- standard library `net/http` 우선
- `embed.FS` static assets
- runtime-configurable port
- non-root runtime user
- Docker HEALTHCHECK
- minimal runtime image
- `manafield.module.json`
- Protocol Health Operation
- DB가 필요하지 않으면 DB dependency 없음

더 복잡한 router/framework는 실제 필요가 생길 때 추가합니다. React/Vite 같은 frontend build tool을 사용하더라도 production runtime은 Go binary 하나로 유지할 수 있습니다.

### TypeScript / React Web profile 후보

현재 `manafield-reference`에서 재사용 가치가 있는 기본 골격:

```text
manafield.module.json
Dockerfile
package.json
package-lock.json

server/
  index.ts

src/
  main.tsx
  App.tsx

index.html
vite.config.ts
tsconfig.app.json
tsconfig.server.json
```

권장 scripts:

```text
npm run check
npm run build
npm start
```

현재 기준:

- Node.js 22
- TypeScript strict mode
- React
- Vite
- Node/Express server

이 기술 조합은 **TS/React Template의 구현 선택**이며 Module Protocol 요구사항이 아닙니다.

### Java / Spring Web profile 후보

Echo v2 같은 stateful Web Module을 새로 설계할 수 있도록 다음 기본값을 후보로 둡니다.

- Java toolchain 고정
- Gradle Wrapper
- Spring Boot Web
- JDK build / JRE runtime multi-stage image
- non-root runtime user
- Docker HEALTHCHECK
- environment-driven application port
- `manafield.module.json`
- Protocol Health Operation
- Capability binding을 application configuration으로 materialize할 수 있는 경계

PostgreSQL/JPA 자체는 모든 Java Web Module의 필수 기능으로 고정하지 않습니다. DB가 필요한 Module만 `database.postgresql` 같은 Capability를 요구합니다.

## 7. Reference에서 Template으로 가져가지 않을 것

현재 `manafield-reference`의 다음 요소는 Reference 기능이며 일반 Template 기본값으로 만들지 않습니다.

### Registry Observer UI

```text
Registry Observer
ModuleCard
Operation 목록 렌더링
```

Reference의 목적 자체이므로 일반 Template과 무관합니다.

### Core Registry Proxy

```text
GET /api/core/modules
```

이 endpoint와 `MANAFIELD_CORE_URL` 사용은 Core Registry를 관찰하기 위한 Reference 기능입니다.

일반 Module이 Core Registry를 proxy할 필요는 없습니다.

### Manafield TypeScript 타입 복제

현재 Reference의:

```text
src/types/manafield.ts
```

는 Registry 응답을 사용하기 위한 로컬 타입입니다.

초기 Template 필수 구성으로 넣지 않습니다.

장기적으로 TypeScript SDK 또는 generated contract가 필요해지면 별도 패키지/도구로 제공하는 방향을 검토합니다.

### Vite `/api` dev proxy

현재 local development 편의를 위한 Reference-specific 설정입니다.

Web API를 가진 TS/React Template 예제로는 사용할 수 있지만 Protocol requirement는 아닙니다.

## 8. Reference에서 확인된 좋은 기본값

현재 구현에서 Template에도 적용할 가치가 있는 기본값:

- runtime container에서 non-root user 사용
- Docker HEALTHCHECK 제공
- production dependency만 runtime stage에 설치
- TypeScript strict mode
- frontend / server type-check 모두 CI 전에 수행
- production build와 development server 분리
- application Health endpoint와 Docker Healthcheck 연결

Node Template에서는 lockfile을 사용한 reproducible build를 위해 `npm install`보다 `npm ci`를 기본으로 권장합니다.

## 9. 아직 Template에 고정하지 않을 것

다음은 실제 Module 사례가 더 쌓이기 전까지 공통 요구사항으로 고정하지 않습니다.

- Settings endpoint
- Permission declaration의 최종 schema
- Capability Descriptor / provider-source 표현의 최종 JSON/YAML schema
- Capability version range / negotiation 문법
- Capability binding / Secret injection의 최종 schema
- Runtime requirement schema
- WebSocket / Event convention
- Core SDK 의존성
- 특정 Web framework
- 특정 HTTP server framework

## 10. Echo 비교 검토에서 추가로 확인된 경계

기존 Spring Boot 기반 `manafield-echo`를 두 번째 서비스 사례로 비교 검토했습니다.

Echo는 아직 현재 Module Protocol에 맞춰 실제 등록된 Module은 아니므로 **두 번째 실제 Module 검증 완료로 보지는 않습니다.**

다만 다음 경계는 더 명확해졌습니다.

### Deployment Compose는 공통 Module 계약이 아님

Echo의 기존 Compose에는 DB network, reverse proxy network, Traefik labels, host port, host secret mount가 직접 들어 있습니다.

이런 production infrastructure wiring은 Module Template 기본값으로 복제하지 않습니다.

새 구조에서는 가능한 한 다음 계층으로 이동합니다.

```text
Instance Definition
Runtime Provider
Ingress Provider
Secret injection
```

Module Repository의 Compose가 필요하다면 local development 용도로 제한하는 방향을 우선합니다.

### Database / Storage는 optional requirement

Echo가 PostgreSQL/JPA를 사용한다고 해서 Database를 Module Template 필수 요소로 만들지 않습니다.

DB/Storage 요구도 [ADR-0010](adr/0010-capability-dependency-resolution.md)에 따라 같은 **Capability Requirement**로 표현합니다. PostgreSQL이 필요한 Module은 `database.postgresql ^1` Capability를 요구하고, Instance는 그 Capability를 제공하는 concrete Resource와 connection binding을 선택합니다.

### Secret은 source 또는 Template에 고정하지 않음

Echo는 JWT public key와 DB credential이 필요합니다.

`.env.example`처럼 **변수 이름과 placeholder만 제공하는 예제 파일**은 허용할 수 있지만:

- 실제 `.env`
- 실제 key
- 실제 password
- host-specific secret path

는 Template/Repository에 포함하지 않습니다.

### Authentication은 공통 Template 구현이 아님

Echo의 JWT verifier와 기존 Manafield API session validation은 Echo의 현재 통합 방식입니다.

향후 Manafield Identity / Permission 모델이 안정화되기 전에는 특정 인증 구현을 모든 Module Template의 기본값으로 두지 않습니다.

### Framework route와 Operation은 동일하지 않음

Echo는 다수의 REST endpoint를 이미 제공하지만, 이를 전부 자동으로 Manafield Operation으로 간주하지 않습니다.

Operation은 Manafield가 discover/call할 **공용 기능 계약**입니다.

따라서 새 Echo v2 설계에서 다음을 검증해야 합니다.

- Framework endpoint 중 어떤 것을 Operation으로 공개할지
- UI 내부 API와 Operation을 어떻게 구분할지
- Descriptor와 실제 route 간 drift를 어떻게 방지할지

Annotation/code generation은 후보이지만 아직 Template 요구사항으로 고정하지 않습니다.

### Java Template 후보에서 확인된 좋은 기본값

- Java toolchain 고정
- Gradle Wrapper 포함
- JDK build / JRE runtime multi-stage image
- runtime port 환경변수화
- application test task 제공

추가로 적용할 권장 기본값:

- non-root runtime user
- Docker HEALTHCHECK

상세 검토는 [Echo Module Migration Review](echo-module-migration-review.md)를 참고합니다.

## 11. 다음 검증

Echo의 기존 backend/deployment 구조를 그대로 migration하지 않습니다. 기존 private Echo에서는 재사용 가치가 높은 CSS/JS/정적 FE 자산만 선별하고, Echo v2 backend와 Module wiring은 현재 Architecture 기준으로 새로 설계합니다.

Echo v2 private Repository를 만들기 전에 다음을 먼저 만족시키는 것을 목표로 합니다.

```text
Module Template Contract
├─ 통합 Capability requirement / provider-source 경계 설명 가능
├─ stateless Go Web Module 설계 가능
├─ Official Go Web Shell 설계 가능
├─ Java / Spring Web Module 설계 가능
└─ database.postgresql Capability를 요구하는 stateful Module 설계 가능
```

그 뒤 실제 Echo v2를 두 번째 Module로 구현하여 Template Contract를 재검증합니다.

필요하다면 구현 Profile별 Template Repository를 분리할 수 있습니다.

```text
manafield-module-template-go-web
manafield-module-template-ts-react
manafield-module-template-java-spring
```

어떤 Template도 Manafield Protocol을 대체하거나 특정 구현 언어를 필수화하지 않습니다.
