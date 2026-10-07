# Echo Module Migration Review

> 기존 private `manafield-echo` 서비스를 현재 Manafield Module 구조에 맞추기 전에 수행한 비교 검토입니다.
>
> 이 문서는 Echo를 즉시 변환하는 구현 계획이 아니라, 두 번째 실제 서비스 사례를 통해 Module Template 경계를 검증하기 위한 기록입니다.

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

장기적으로 Module package/runtime requirement가 구체화될 때 Database/Storage dependency를 선언하는 별도 모델을 검토합니다.

### Authentication coupling

현재 Echo는 자체 JWT verifier와 기존 Manafield API session validation endpoint를 사용합니다.

또한 Web frontend에도 `https://manafield.studio`가 직접 들어간 코드가 존재합니다.

이는 기존 Manafield 서비스 구조와의 결합이며 새로운 Module Template의 기본 패턴으로 가져가지 않습니다.

향후 Identity / Permission 모델이 정의되면 Echo 인증 흐름을 다시 연결해야 합니다.

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

## 6. Echo를 현재 Manafield Module로 전환할 때 필요한 최소 작업

### Protocol

- `manafield.module.json` 추가
- Health Operation 추가
- 최소한 핵심 callable API를 Operation Contract로 표현
- Descriptor validation 통과

### Container

- runtime non-root user
- Docker HEALTHCHECK
- host port publish 제거 또는 development-only로 이동
- production deployment용 자체 Traefik labels 제거

### Instance Definition

Echo 자체 Repository의 production Compose가 아니라 Instance Definition에서 다음을 구성합니다.

- source / version
- build contract
- Web exposure
- Database / Secret requirement는 현재 가능한 범위에서 별도 주입

### Legacy integration

기존 JWT / Manafield API 직접 호출은 초기 migration bridge로 유지할 수 있지만, Module Template 요구사항으로 승격하지 않습니다.

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

따라서 Echo migration 시 다음을 검증해야 합니다.

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

Echo 분석 자체는 완료했지만, **두 번째 실제 Module 검증은 아직 완료로 처리하지 않습니다.**

Echo에 현재 Module Protocol을 실제로 적용하고 Core Registry에서 동작시키는 단계까지 완료한 뒤:

```text
두 번째 실제 Module에서 Template 요구사항 재검증
```

Roadmap 항목을 완료 처리합니다.
