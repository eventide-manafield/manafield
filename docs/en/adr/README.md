# Architecture Decision Records

This directory records major Manafield architecture decisions and their rationale.

ADRs focus on **long-lived boundaries and responsibilities** rather than transient implementation details.

| ADR | Decision | Status |
| --- | --- | --- |
| [0001](0001-rust-headless-core.md) | Rust Headless Core | Accepted |
| [0002](0002-operation-contract.md) | Operation Contract as the common capability contract | Accepted |
| [0003](0003-registry-snapshot.md) | Registry write model with ArcSwap snapshot reads | Accepted |
| [0004](0004-runtime-provider-boundary.md) | Separate Runtime Providers from ordinary Modules | Accepted |
| [0005](0005-docker-provider-isolation.md) | Isolate Docker Provider in a separate process | Accepted |
| [0006](0006-instance-build-plan.md) | Instance Build Plan / single Pipeline per instance | Accepted |
| [0007](0007-public-platform-private-instance.md) | Public Platform / Private Instance Configuration | Accepted |

| [0008](0008-network-planes.md) | Manafield Network Planes | Accepted |

Add new ADRs sequentially as long-term decisions are made.
