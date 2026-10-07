# Web Surface v0

> Manafield Web Shell과 Module Web Exposure를 연결하기 위한 초기 Web Surface 설계입니다.
>
> Public URL ownership은 ADR-0009, Official Web Shell 구조는 ADR-0011을 따릅니다.

## 1. 목적

Module이 HTTP server를 가진다는 사실만으로는 다음을 구분할 수 없습니다.

- 사용자가 직접 보는 화면인가
- 외부 client가 호출하는 API인가
- 둘 다인가
- 내부 통신용 HTTP endpoint일 뿐 public Web surface는 아닌가

따라서 Web exposure용 metadata는 **화면과 API surface를 구분**합니다.

## 2. Surface kind

초기 Web Surface kind는 두 가지입니다.

```text
page
→ user-facing 화면
→ Web Shell navigation에 노출될 수 있음

api
→ externally exposed HTTP API
→ navigation entry는 만들지 않음
```

한 Module은 둘 다 제공할 수 있습니다.

예:

```yaml
web:
  surfaces:
    - id: page
      kind: page
      title: Echo
      description: 짧은 메시지를 남기고 관찰하는 공간

    - id: api
      kind: api
      description: Echo public HTTP API
```

정확한 manifest field name은 구현 시 조정될 수 있지만 `page`와 `api` 의미 구분은 유지합니다.

## 3. Operation과 Web API는 같은 개념이 아니다

`api` Web Surface는 public HTTP exposure intent를 나타냅니다.

Operation은 Manafield가 discover/call하는 protocol contract입니다.

따라서:

```text
HTTP endpoint
≠ automatically Operation
≠ automatically public API
```

Module은 내부 endpoint, Operation, public API surface를 필요에 따라 구분할 수 있습니다.

## 4. Public path는 Instance가 결정한다

Module은 `/echo` 같은 public path를 authoritative하게 소유하지 않습니다.

Instance가 concrete path를 binding합니다.

개념 예:

```yaml
exposure:
  type: prefix
  host: manafield.studio
  prefix: /echo
  targetPort: 8080
```

Web Surface metadata의 title/description/icon 같은 정보는 Module이 제공할 수 있지만 public path는 Instance-owned policy입니다.

## 5. Prefix는 strip하지 않는다

Manafield의 기본 same-origin prefix routing은 **public prefix를 제거하지 않고 그대로 Module에 전달**합니다.

```text
public request
GET /echo/foo

module receives
GET /echo/foo
```

다음처럼 바꾸지 않습니다.

```text
GET /echo/foo
→ GET /foo   ❌
```

이 원칙은 개발환경과 실제 배포환경의 route 의미를 같게 유지하기 위한 것입니다.

## 6. Module base path

Public prefix가 Instance-owned이므로 Module은 자신에게 할당된 base path를 runtime configuration으로 받을 수 있어야 합니다.

초기 Container convention 후보:

```text
MANAFIELD_WEB_BASE_PATH=/echo
```

최종 injection schema는 Runtime/Build Plan 설계에서 확정합니다.

Module은 base path를 기준으로:

- handler route
- static asset URL
- redirect target
- cookie Path
- browser-side API URL

을 일관되게 구성해야 합니다.

## 7. Root Shell

통합 Web Instance의 기본 shape:

```text
/              → manafield-web
/account/*     → Account page/api surfaces
/echo/*        → Echo page/api surfaces
```

`manafield-web`은 Module route를 대신 proxy하지 않습니다.

Ingress가 same-origin route를 해당 Module에 직접 전달합니다.

## 8. Navigation

Web Shell은 `kind: page` surface만 navigation 후보로 사용합니다.

`kind: api` surface는 navigation item을 만들지 않습니다.

Page surface metadata 후보:

- title
- description
- icon
- navigation group
- navigation order

이 metadata는 presentation용이며 functional Capability matching과 무관합니다.

## 9. API-only Module

화면 없이 public API만 제공하는 Module도 정상입니다.

예:

```text
Module
├─ page surface 없음
└─ api surface 있음
```

반대로 internal Operations만 제공하고 public Web exposure가 전혀 없는 Module도 정상입니다.

## 10. 아직 미정인 항목

- 최종 manifest JSON/YAML field names
- 여러 page surface를 가진 Module의 navigation 표현
- icon format
- WebSocket-specific metadata
- CORS 정책
- cookie/session protocol
- reserved root path 최종 목록
