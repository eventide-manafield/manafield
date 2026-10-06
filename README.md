# Manafield

<p align="center">
  <strong>A modular personal platform.</strong><br/>
  기록, 서비스, 도구와 외부 시스템을 하나의 환경으로 조립하는 개인용 모듈형 플랫폼.
</p>

<p align="center">
  <a href="https://buymeacoffee.com/manafield">
    <img src="https://img.shields.io/badge/Buy_Me_a_Coffee-Support_Manafield-FFDD00?style=for-the-badge&logo=buymeacoffee&logoColor=000000" alt="Buy Me a Coffee"/>
  </a>
</p>

> [!IMPORTANT]
> **Pre-alpha / Design Stage**  
> Manafield는 현재 Core architecture와 Module Protocol을 설계하고 있는 초기 단계야.

[한국어](#한국어) · [English](#english) · [Architecture](docs/architecture.md) · [Module Protocol](docs/module-protocol.md) · [Roadmap](docs/roadmap.md)

---

# 한국어

## 이런 걸 만들고 있어

**Manafield**는 여러 프로그램과 서비스를 하나의 방식으로 연결하고 관리하는 **개인용 모듈형 플랫폼**이야.

핵심은 단순해.

> **Core는 엄격하게, Module은 자유롭게.**

Manafield Core는 **Rust**로 작고 엄격하게 만들고, Module은 Python, Go, Node.js, Java, Rust 등 **어떤 언어든 상관없이 API 계약만 지키면 연결**할 수 있는 구조를 목표로 해.

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
│ Registry · Lifecycle · Runtime · Health   │
│ Permission · Settings · Discovery · Event │
└───────┬───────────────┬───────────────┬───┘
        │               │               │
        ▼               ▼               ▼
     Module          Service        Integration
     Python            Node             Go
```

## 왜 만들고 있나?

개인 홈페이지에 기능 하나를 추가할 때마다 전체 프로젝트를 뜯어고치는 대신,

- 필요한 기능만 골라 붙이고
- 독립 서비스도 같은 방식으로 관리하고
- Web UI가 없어도 동작하고
- 필요하면 Web, CLI, Mobile 같은 다른 View를 붙이고
- 나중에는 Module Package를 설치하는 것만으로 기능을 추가하는

그런 플랫폼을 직접 만들어보는 게 목표야.

`manafield.studio`는 Manafield 자체가 아니라 **Eventide가 운영하는 하나의 Manafield 인스턴스**가 될 예정이야.

## 목표 구조

- **Headless Core** — Web UI가 없어도 동작
- **Language-independent Modules** — 언어와 Framework에 종속되지 않음
- **API-first Protocol** — Core와 Module은 API 계약으로 연결
- **Docker Runtime** — 초기 실행 환경
- **Optional Web Views** — Module이 필요할 때만 Web UI 제공
- **Isolated Modules** — Core 프로세스와 Module 실행환경 분리
- **Runtime abstraction** — 먼 미래에는 Kubernetes도 연결 가능

## 중간 보스

### Misskey를 Manafield Module로 얹기

Misskey처럼 자체 Web, DB, Cache, Storage와 Lifecycle을 가진 서비스를 Manafield가 설치하고 관리할 수 있다면, Module Host로서의 구조가 제대로 동작한다고 볼 수 있을 거야.

```text
Manafield
   ↓
Docker Runtime
   ↓
Misskey Service Module
├─ Application
├─ PostgreSQL
├─ Redis
└─ Storage
```

## 더 보기

설계 세부사항은 README에서 분리해서 관리해.

- [Architecture](docs/architecture.md)
- [Module Protocol](docs/module-protocol.md)
- [Roadmap](docs/roadmap.md)

## License

[Apache License 2.0](LICENSE)

수정·재배포할 때는 원본의 라이선스와 attribution을 유지하고, 수정된 파일에는 변경 사실을 명시해야 해. 자세한 내용은 [NOTICE](NOTICE)를 참고해.

---

# English

## What is Manafield?

**Manafield** is a modular personal platform for composing independent services, tools, modules, and integrations into one environment.

Its guiding principle is:

> **Strict Core, free Modules.**

The Core is planned to be implemented in **Rust**, while Modules may use any language or framework as long as they implement the Manafield Module Protocol.

## Highlights

- **Headless Core** — no mandatory Web UI
- **Language-independent Modules**
- **API-first Module Protocol**
- **Docker as the initial Runtime**
- **Optional Web Contributions**
- **Isolated Module execution**
- **Runtime abstraction** with Kubernetes as a long-term target

`manafield.studio` is not Manafield itself. It is planned to become **one Manafield instance operated by Eventide**.

## Intermediate Goal

A major milestone is to:

> **Run and manage Misskey as a Manafield Service Module.**

This will validate real-world lifecycle management involving an application runtime, database, cache, storage, networking, health checks, and a Web service.

## Documentation

- [Architecture](docs/architecture.md)
- [Module Protocol](docs/module-protocol.md)
- [Roadmap](docs/roadmap.md)

## Status

**Pre-alpha / Design Stage**

The architecture and protocol are still evolving.

## License

[Apache License 2.0](LICENSE) · [NOTICE](NOTICE)
