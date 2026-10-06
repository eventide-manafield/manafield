# Manafield

> **A modular personal platform.**  
> 기록, 서비스, 도구와 외부 시스템을 하나의 환경으로 조립하는 개인용 모듈형 플랫폼.

[한국어](#한국어) · [English](#english)

> [!IMPORTANT]
> Manafield is currently in the **pre-alpha / design stage**.  
> 현재는 구현보다 Core architecture와 Module Protocol을 먼저 정의하고 있으며, 아래 내용은 개발 과정에서 변경될 수 있습니다.

---

# 한국어

## Manafield란?

**Manafield**는 서로 독립적인 모듈, 서비스, 도구와 외부 시스템을 하나의 개인 환경으로 구성하기 위한 **모듈형 개인 플랫폼**이다.

Manafield 자체는 특정 웹사이트나 애플리케이션 하나를 의미하지 않는다.

작은 개인용 서비스에서 시작해 기록 시스템, 봇, 독립 웹 서비스, 외부 플랫폼 연동 등을 하나의 환경에서 연결하고 관리할 수 있는 기반을 목표로 한다.

`manafield.studio`는 Manafield 자체가 아니라 **Eventide가 Manafield를 이용해 구성하고 운영하는 하나의 인스턴스**다.

## 핵심 철학

### Headless Core

Manafield의 중심에는 작은 **Headless Core**가 존재한다.

Core는 특정 Web UI를 필수 구성요소로 가지지 않는다. 대신 API를 제공하며 Web, CLI, Mobile Client 등은 같은 Core 위에 독립적으로 연결될 수 있다.

```text
Manafield Core
├─ Manafield Web
├─ CLI
├─ Mobile Client
└─ Other Clients
```

공식 Web View를 제공할 수 있지만, Web이 없어도 Core와 Module은 동작할 수 있어야 한다.

### Strict Core, Free Modules

Core는 Manafield에서 가장 엄격해야 하는 영역이다.

Module lifecycle, protocol validation, runtime abstraction, 권한과 상태 관리는 Core가 일관되게 책임진다.

반대로 Module의 내부 구현은 최대한 자유롭게 둔다.

```text
                    ┌─────────────────┐
                    │  Manafield Web  │
                    │    optional     │
                    └────────┬────────┘
                             │ Core API
                             ▼
┌────────────────────────────────────────────┐
│              Manafield Core               │
│                   Rust                    │
│                                            │
│ Registry       Lifecycle      Settings    │
│ Permission     Health         Discovery   │
│ Runtime        Routing        Events      │
└───────┬───────────────┬───────────────┬───┘
        │               │               │
        │ Module API    │ Module API    │ Module API
        ▼               ▼               ▼
     Module          Service        Integration
```

Core는 현재 **Rust**로 구현하는 것을 목표로 한다.

### Language-independent Modules

Manafield Module은 특정 프로그래밍 언어나 Framework에 종속되지 않는다.

Core와 Module은 직접적인 코드 의존성이 아니라 **Module Protocol**을 통해 통신한다.

```text
Rust Core
    │
    │ HTTP / WebSocket / Message
    │
    ├──── Python Module
    ├──── Go Module
    ├──── Node.js Module
    ├──── Java Module
    └──── Anything that implements the protocol
```

SDK는 개발 편의를 위해 제공될 수 있지만 필수 사항이 아니다.

> **The protocol is the contract.**

Core 구현 언어가 바뀌더라도 동일한 Protocol version을 지원하는 기존 Module은 계속 동작할 수 있는 구조를 목표로 한다.

## Module Types

초기 설계에서는 Manafield에 연결되는 구성요소를 크게 세 종류로 구분한다.

### Module

Manafield에 직접적인 기능을 제공한다.

예:

- Observe
- Records
- Dashboard

### Service

독립적인 프로세스 또는 컨테이너로 실행되는 서비스를 Manafield에서 관리한다.

예:

- Echo
- Misskey

### Integration

외부 플랫폼이나 시스템을 Manafield와 연결한다.

예:

- ProtoDuck
- Discord
- External API integrations

이 구분은 실행 형태와 역할을 표현하기 위한 것이며, 가능한 한 동일한 Module Protocol을 공유한다.

## Module Protocol

초기 Module Protocol은 작은 공통 API 계약에서 시작한다.

예:

```http
GET /manafield/health
GET /manafield/info

GET /manafield/settings/schema
GET /manafield/settings
PUT /manafield/settings
```

예시 응답:

```json
{
  "id": "example",
  "name": "Example Module",
  "version": "0.1.0",
  "apiVersion": "v1",
  "capabilities": [
    "example.read"
  ]
}
```

정확한 API와 Schema는 구현 과정에서 별도의 Protocol 문서로 관리한다.

## Module Manifest

Module은 Core가 이해할 수 있는 Manifest를 제공한다.

현재 구상 중인 예:

```yaml
id: example
name: Example Module
version: 0.1.0

manafieldApi: v1
type: module

runtime:
  type: container

container:
  image: ghcr.io/example/example-module:0.1.0
  port: 8080

health:
  method: GET
  path: /manafield/health

web:
  mode: proxy
  route: /example

permissions:
  - example.read
```

Manifest 형식은 아직 확정되지 않았다.

## Runtime

Manafield는 Container Runtime이나 Orchestrator 자체를 새로 구현하지 않는다.

대신 Docker, Kubernetes 등의 기존 Runtime 위에서 Module lifecycle을 관리하는 **경량 Control Plane / Module Host**를 목표로 한다.

초기 Runtime target은 **Docker**다.

```text
Manafield Core
      │
      │ Runtime Adapter
      ▼
┌───────────────┐
│    Docker     │  ← initial target
└───────────────┘

┌───────────────┐
│  Kubernetes   │  ← long-term target
└───────────────┘
```

Kubernetes는 장기적인 architecture validation 목표이며, 초기 개발 범위에는 포함하지 않는다.

Core는 Runtime 구현에 직접 결합되지 않고 추상화된 Runtime interface를 통해 접근하는 것을 목표로 한다.

개념적인 예:

```rust
trait ModuleRuntime {
    async fn install(&self, module: &ModuleSpec) -> Result<()>;
    async fn start(&self, id: &ModuleId) -> Result<()>;
    async fn stop(&self, id: &ModuleId) -> Result<()>;
    async fn status(&self, id: &ModuleId) -> Result<ModuleStatus>;
    async fn remove(&self, id: &ModuleId) -> Result<()>;
}
```

## Web Contributions

Module은 Web UI를 반드시 제공할 필요가 없다.

Web이 필요한 경우 Module은 자신의 Web View에 대한 정보를 Core에 제공할 수 있다.

초기에는 다음과 같은 방식을 고려한다.

```text
none
    UI 없음

proxy
    Module 자체 Web UI를 Manafield를 통해 노출

external
    외부 URL로 연결
```

향후 필요에 따라 더 밀접한 Web View 통합 방식을 추가할 수 있다.

## Module Development

Manafield Core와 Module Template은 별도의 Repository로 관리한다.

예정 구조:

```text
manafield

manafield-module-template-go
manafield-module-template-python
manafield-module-template-node
```

Module 개발자는 Manafield Core의 내부 구현을 알 필요 없이 Template Repository에서 새 프로젝트를 만들고 Module Protocol만 구현할 수 있어야 한다.

언어별 SDK를 제공할 수 있지만 SDK는 선택 사항이다.

## Isolation

Module은 가능한 한 Core와 독립적으로 실행한다.

```text
Manafield Core
      │
      │ API
      ▼
Module Process / Container
```

사용자가 설치한 임의 코드를 Core 프로세스 내부에서 직접 실행하는 방식을 기본 모델로 삼지 않는다.

장기적으로 third-party Module은 Container 또는 Sandbox를 통해 추가적인 격리를 제공하는 것을 목표로 한다.

## Initial Roadmap

### Phase 0 — Foundation

- [ ] Rust Core bootstrap
- [ ] Core API
- [ ] Module state model
- [ ] Module Registry
- [ ] Manifest parser
- [ ] Module Protocol v0
- [ ] Reference Headless Module

### Phase 1 — Docker Runtime

- [ ] Docker Runtime Adapter
- [ ] Module container lifecycle
- [ ] Health checks
- [ ] Start / Stop / Restart
- [ ] Runtime status
- [ ] Basic log access

### Phase 2 — Web

- [ ] Official Manafield Web View
- [ ] Dynamic Module navigation
- [ ] Module Web Contribution
- [ ] Settings UI
- [ ] Module status UI

### Phase 3 — Reference Modules

- [ ] Observe
- [ ] ProtoDuck Integration
- [ ] Echo Service

### Intermediate Goal

- [ ] **Run and manage Misskey as a Manafield Service Module**

Misskey는 독립적인 runtime, database, cache, storage, network와 lifecycle을 가진 실제 서비스를 Manafield가 관리할 수 있는지 검증하기 위한 주요 중간 목표다.

### Long-term

- [ ] Module package format
- [ ] Module install / update / remove
- [ ] Drag & Drop Module installation
- [ ] Permission review
- [ ] Dependency management
- [ ] Module Registry
- [ ] Compatibility Test Kit
- [ ] Kubernetes Runtime Adapter
- [ ] Third-party Module isolation

## Non-goals

Manafield는 다음을 새로 구현하는 것을 목표로 하지 않는다.

- Container runtime
- Container isolation mechanism
- Database engine
- Reverse proxy implementation from scratch
- Kubernetes replacement

이미 존재하는 기술을 대체하는 대신, 이들을 **Module이라는 공통 모델로 연결하고 관리하는 Control Plane**을 목표로 한다.

## Development Status

**Pre-alpha / Design Stage**

현재 Manafield는 Core architecture와 Module Protocol을 정의하는 단계다.

API, Manifest Schema, Runtime interface, Web integration과 Repository structure는 개발 과정에서 변경될 수 있다.

## License

Manafield is licensed under the **Apache License 2.0**.

Modified and redistributed versions must preserve the applicable license and attribution notices, and modified files must carry prominent notices stating that they were changed.

See [LICENSE](./LICENSE) and [NOTICE](./NOTICE).

---

# English

## What is Manafield?

**Manafield** is a modular personal platform for composing independent modules, services, tools, and external systems into a single environment.

Manafield itself is not a website or a single application.

It aims to provide a small control plane capable of connecting and managing personal services, record systems, bots, web applications, and external integrations.

`manafield.studio` is not Manafield itself. It is **one Manafield instance configured and operated by Eventide**.

## Architecture

Manafield is built around a small **headless Core**, currently planned to be implemented in Rust.

The Core handles:

- Module registry and discovery
- Manifest validation
- Module lifecycle
- Runtime abstraction
- Health checks
- Authentication and permissions
- Settings
- Service discovery
- Routing metadata
- Events and messaging

Modules communicate with the Core through a language-independent protocol.

```text
                    ┌─────────────────┐
                    │  Manafield Web  │
                    └────────┬────────┘
                             │
                             ▼
┌────────────────────────────────────────────┐
│              Manafield Core               │
│                   Rust                    │
└───────┬───────────────┬───────────────┬───┘
        │               │               │
        ▼               ▼               ▼
     Module          Service        Integration
```

## Language Independent

Modules do not need to use the same language or framework as Manafield Core.

A Module may be implemented in Python, Go, Node.js, Java, Rust, or any other environment capable of implementing the Module Protocol.

SDKs are optional.

> **The protocol is the contract.**

## Runtime

Manafield does not attempt to replace container runtimes or orchestrators.

Instead, it provides a lightweight control plane above existing runtime environments.

The initial runtime target is **Docker**.

Kubernetes support is considered a long-term architectural goal.

```text
Manafield
   ↓
Runtime Adapter
   ↓
Docker

Future:
Manafield
   ↓
Runtime Adapter
   ↓
Kubernetes
```

## Headless Core

Manafield Core does not require a built-in Web UI.

Web, CLI, mobile applications, or other clients may communicate with the same Core API.

An official Manafield Web View may be provided separately.

## Module Development

Module templates are maintained independently from the Core repository.

Planned examples:

```text
manafield-module-template-go
manafield-module-template-python
manafield-module-template-node
```

A developer should be able to create a Module without cloning or depending on the internal implementation of Manafield Core.

## Intermediate Goal

A major milestone is:

> **Running and managing Misskey as a Manafield Service Module.**

This milestone is intended to validate Docker lifecycle management, configuration, networking, health checks, storage, databases, caches, and independent Web services through the Manafield Module model.

## Status

**Pre-alpha / Design Stage**

Manafield is currently being redesigned around:

- a Rust-based Core
- a language-independent Module Protocol
- a headless architecture
- Docker as the initial Runtime
- optional Web Views
- isolated Module execution

The architecture is expected to evolve significantly during early development.

## License

Manafield is licensed under the **Apache License 2.0**.

See [LICENSE](./LICENSE) and [NOTICE](./NOTICE) for attribution and redistribution requirements.
