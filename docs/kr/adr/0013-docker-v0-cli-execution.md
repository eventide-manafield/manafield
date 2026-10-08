# ADR-0013 — Docker-required v0 Deployment / CLI-first Execution

- 상태: Accepted
- 날짜: 2026-10-08

## Context

Manafield는 Core의 일반 Registry / Protocol 모델과 실제 실행 기술을 Runtime Provider 경계로 분리합니다.

하지만 v0에서 Docker 외의 Process / Remote / Kubernetes 실행기를 동등하게 지원하려고 하면 lifecycle, network, filesystem, secret, restart, isolation을 Manafield가 직접 다시 구현해야 하는 범위가 지나치게 커집니다.

또한 현재 Jenkins Pipeline 안에 Manafield 고유의 build / deploy 의미론이 계속 쌓이면 Jenkins가 사실상의 필수 Runtime이 될 수 있습니다.

## Decision

Manafield v0의 지원되는 **전체 Instance 구축 / 배포에는 Docker가 필요합니다.**

이는 Docker를 Core 내부 모델에 결합한다는 뜻이 아닙니다.

```text
Manafield model
  Instance / Module / Resource / Capability / Binding
          ↓
manafield CLI
          ↓
Docker-based v0 executor
```

Core binary는 Docker 없이도 Registry, validation, API 같은 독립 기능을 실행할 수 있습니다. 하지만 완전한 v0 Instance의 build / materialization / deployment는 Docker가 설치된 환경을 기준으로 합니다.

Manafield CLI를 Instance 실행의 canonical entrypoint로 둡니다.

```text
manafield plan
manafield build
manafield deploy
manafield verify
manafield rebuild
```

각 기능은 단계적으로 구현합니다. Jenkinsfile에 이미 존재하는 Manafield-specific build / deployment 의미론은 한 번에 재작성하지 않고 CLI / reusable executor로 조금씩 이동합니다.

Jenkins는 **필수 구성요소가 아니라 첫 공식 remote CI/CD frontend**입니다.

```text
Local terminal ─┐
                ├─> manafield CLI ─> Docker
Jenkins ────────┘
```

Jenkins는 trigger, credential integration, approval, build history, remote UI 같은 CI 책임을 가질 수 있지만, 가능한 한 Manafield 고유의 build / deploy 규칙을 직접 소유하지 않습니다.

## Consequences

- Docker는 v0 전체 Instance build / deploy 요구사항에 포함됩니다.
- Docker가 없어도 Core 개발 / validation / 일부 headless 기능은 가능하지만 공식 v0 Instance deployment 대상은 아닙니다.
- 초기 Module build contract는 Dockerfile 중심으로 유지합니다.
- Jenkins Pipeline stage는 점진적으로 `manafield` CLI command 호출로 축소합니다.
- credential store, approval UI, webhook 같은 CI 기능은 executor-neutral Manafield model과 분리합니다.
- Process / Remote / Kubernetes Runtime Provider는 장기 확장 가능성으로 남지만 v0 호환성 요구사항은 아닙니다.

## Non-goals

이 결정은 Core에 `docker.sock`을 직접 mount하거나, Capability / Binding 모델에 Docker container ID를 넣거나, Jenkins 설치를 Manafield 사용 조건으로 만드는 것을 의미하지 않습니다.

Docker는 **v0의 의도적인 deployment dependency**이고, Jenkins는 **선택 가능한 orchestration frontend**입니다.
