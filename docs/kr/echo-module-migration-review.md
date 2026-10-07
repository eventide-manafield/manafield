# Echo Module Migration Review

> 기존 private `manafield-echo` 서비스를 현재 Manafield Module 구조에 맞추기 전에 수행한 비교 검토입니다.
>
> 이 문서는 기존 Echo를 in-place 변환하는 구현 계획이 아니라, 두 번째 실제 서비스 사례를 통해 Module Template 경계를 검증하기 위한 기록입니다.
>
> **현재 결정:** Echo v2 backend/deployment는 새 Architecture 기준으로 새로 만들고, 기존 private Echo에서는 재사용 가치가 높은 CSS/JS/정적 FE 자산만 선별합니다.

## 1. 현재 Echo의 성격

현재 Echo는 다음 특징을 가진 독립 Spring Boot 서비스입니다.

- Java 21
- Spring Boot 4.1
- Gradle Wrapper
- Spring MVC / Validation / Security
- PostgreSQL + JPA
- JWT 인증
- Scheduler
- 자체 REST API
- 자체 정적 Web UI

Reference Module과 달리 DB, Secret, 권한, Background job, Web UI가 모두 존재합니다.

따라서 Template 요구사항 검증 대상으로 적합하지만, 아직 현재 Module Protocol에 직접 맞춰진 Module은 아닙니다.

## 2. 현재 새 Module Protocol과의 차이

현재 Echo에는 다음이 없습니다.

- `manafield.module.json`
- `healthOperation`
- `/manafield/health` 또는 이에 준하는 선언된 Health Operation
- Build Plan 기반 exposure 선언

현재 배포는 자체 `docker-compose.yml`에서 다음 인프라를 직접 알고 있습니다.

- external DB network
- external `proxy` network
- Traefik labels
- host port publish
- JWT public key host mount

이 구성은 예전 서버 배선 방식이며 현재 Manafield 방향에서는 대부분 Instance / Runtime / Ingress Provider 책임으로 이동해야 합니다.

## 3. Template 관점에서 확인된 공통점

Reference와 Echo가 기술 스택이 전혀 다른데도 공통으로 필요한 것은 매우 적습니다.

### 확실한 공통 계약

- Module identity
- Module version
- Operation contracts
- Health visibility
- 독립 build contract
- runtime listen port
- secret을 source에 넣지 않는 규칙

이는 특정 Framework Template보다 Module Protocol이 중심이어야 한다는 현재 방향을 뒷받침합니다.

### 언어별 build tooling은 Template별 선택

Reference:

```text
Node.js / npm / TypeScript / Vite
```

Echo:

```text
Java / Gradle / Spring Boot
```

따라서 `npm run check`, Gradle Wrapper, React, Spring annotations 등은 공통 Module 요구사항이 아닙니다.

## 4. Echo에서 Template으로 가져가지 않을 요소

### Database wiring

현재 Echo는 PostgreSQL connection 정보를 직접 받습니다.

DB가 필요한 Module은 존재할 수 있지만 모든 Module에 DB가 필요한 것은 아닙니다.

따라서 다음은 공통 Template에 고정하지 않습니다.

- PostgreSQL
- JPA
- DB network name
- DB credential variable names

Database/Storage dependency는 [ADR-0010](adr/0010-capability-dependency-resolution.md)에 따라 Module Capability와 분리된 **Resource Requirement**로 표현합니다. Echo v2는 PostgreSQL을 전제로 하므로 `database.postgresql` Resource를 요구하는 첫 실제 사례가 될 수 있습니다.

### Authentication coupling

현재 Echo는 자체 JWT verifier와 기존 Manafield API session validation endpoint를 사용합니다.

또한 Web frontend에도 `https://manafield.studio`가 직접 들어간 코드가 존재합니다.

이는 기존 Manafield 서비스 구조와의 결합이며 새로운 Module Template의 기본 패턴으로 가져가지 않습니다.

Echo v2가 계정/Identity 기능을 요구하게 되면 특정 계정 Module ID에 고정하기보다 `manafield.identity` 같은 Capability Contract를 요구하는 방향을 우선합니다. 실제 Identity 구현체는 Instance binding이 선택합니다.

### Traefik / proxy wiring

현재 Docker Compose의 Traefik labels와 `proxy` network는 Template에서 제거 대상입니다.

새 구조에서는:

```text
Instance Definition
→ Build Plan
→ Ingress Provider
```

가 public exposure를 결정합니다.

### Host secret mount

JWT public key를 host path에서 직접 mount하는 방식 역시 공통 Template 요구사항이 아닙니다.

Secret requirement / injection은 Instance / Runtime 계층에서 다룰 대상입니다.

## 5. Echo에서 확인된 좋은 기본값

### Gradle Wrapper

Repository 자체가 build tool version을 고정하므로 재현 가능한 build에 유리합니다.

Java Template에서는 Wrapper 포함을 기본 권장으로 볼 수 있습니다.

### Environment-driven application port

현재 Echo는 `ECHO_APP_PORT`로 application port를 override할 수 있습니다.

이는 Container Template의 port override 요구사항을 확인해 줍니다.

단, 공통 convention은 장기적으로 `PORT`처럼 구현 언어와 무관한 이름을 우선 검토합니다.

### Multi-stage Docker build

JDK build image와 JRE runtime image를 분리한 현재 Dockerfile 방향은 좋은 기본값입니다.

다만 현재 Echo Dockerfile에는 다음이 추가로 필요합니다.

- non-root runtime user
- Docker HEALTHCHECK

## 6. Echo v2를 새 Module로 설계하기 전에 필요한 Template 경계

Echo v2 private Repository를 만들기 전에 공통 Template Contract와 Java/Spring Web profile에서 다음을 설명할 수 있어야 합니다.

### Protocol

- `manafield.module.json`
- optional Module / Operation `description`
- Health Operation
- 핵심 callable API의 Operation Contract
- Capability Requirement / Provided Capability의 경계

### Resource

- `database.postgresql` Resource Requirement
- concrete PostgreSQL Provider / allocation은 Instance가 선택
- DB credential은 Secret injection으로 전달
- Echo는 JDBC/PostgreSQL driver로 DB에 직접 연결

### Container

- runtime non-root user
- Docker HEALTHCHECK
- runtime-configurable port
- production host port / Traefik label을 Module Repository에 고정하지 않음

### Instance Definition

Instance가 다음 concrete wiring을 결정합니다.

- source / version
- build contract
- Web exposure
- Capability binding
- Resource Provider binding
- Secret injection

## 7. Operation Contract 관련 관찰

Echo에는 이미 다수의 REST endpoint가 존재합니다.

예:

```text
GET  /api/echoes
POST /api/echoes
POST /api/echoes/{echoId}/replies
POST /api/echoes/{echoId}/resonate
GET  /api/admin/echoes
...
```

모든 Framework endpoint를 자동으로 Operation으로 취급하는 것은 적절하지 않을 수 있습니다.

Operation은 단순 HTTP route 목록이 아니라 **Manafield가 discover/call할 공용 기능 계약**입니다.

따라서 Echo v2 설계 시 다음을 검증해야 합니다.

- 어떤 HTTP endpoint를 Operation으로 공개할 것인가
- Web UI 내부 전용 endpoint와 Operation을 어떻게 구분할 것인가
- Descriptor와 Framework route 간 drift를 어떻게 방지할 것인가

Annotation/code generation 같은 자동화는 아직 Template 필수 요구사항으로 고정하지 않습니다.

## 8. Template 요구사항에 반영할 결론

Echo 검토로 다음 경계가 강화되었습니다.

- Web framework는 공통 요구사항이 아님
- Database/Storage는 optional runtime requirement
- Authentication 구현은 Module별/향후 Permission model 영역
- public exposure는 Instance/Ingress 책임
- Secret mount는 Runtime/Instance 책임
- Dockerfile build contract와 Health visibility는 공통성이 높음
- 언어별 Template은 build/runtime 편의만 제공하고 Protocol을 대체하지 않음

## 9. 상태

기존 Echo 분석 자체는 완료했지만, **두 번째 실제 Module 검증은 아직 완료로 처리하지 않습니다.**

먼저 Capability/Resource dependency와 Java/Spring Web profile을 포함한 Template Contract를 정리합니다. 그 뒤 새 private Echo v2 Repository를 만들고 실제 Core Registry에서 동작시키는 단계까지 완료한 뒤:

```text
두 번째 실제 Module에서 Template 요구사항 재검증
```

Roadmap 항목을 완료 처리합니다.
