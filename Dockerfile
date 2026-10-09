# syntax=docker/dockerfile:1.7

FROM rust:1.99-bookworm AS source

WORKDIR /src

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY src ./src
COPY deploy/instance.bootstrap.yaml ./deploy/instance.bootstrap.yaml

RUN rustup component add rustfmt clippy

FROM source AS verify

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo fmt --check \
    && cargo check --locked --all-features --all-targets \
    && cargo test --locked --all-features \
    && cargo clippy --locked --all-features --all-targets -- -D warnings

FROM source AS core-builder

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release --bin manafield --bin manafield-core \
    && cp /src/target/release/manafield /tmp/manafield \
    && cp /src/target/release/manafield-core /tmp/manafield-core

FROM source AS build-plan-builder

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release --features build-plan --bin manafield-build-plan \
    && cp /src/target/release/manafield-build-plan /tmp/manafield-build-plan

FROM source AS ingress-traefik-builder

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release --features ingress-traefik --bin manafield-ingress-traefik \
    && cp /src/target/release/manafield-ingress-traefik /tmp/manafield-ingress-traefik

FROM debian:bookworm-slim AS core-runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /var/lib/manafield/modules /var/lib/manafield/logs \
    && chown 10001:10001 /var/lib/manafield/logs

COPY --from=core-builder /tmp/manafield /usr/local/bin/manafield
COPY --from=core-builder /tmp/manafield-core /usr/local/bin/manafield-core

ENV MANAFIELD_BIND=0.0.0.0:8080 \
    MANAFIELD_MODULES_DIR=/var/lib/manafield/modules \
    RUST_LOG=manafield=info \
    MANAFIELD_LOG_FILE=/var/lib/manafield/logs/core.jsonl

WORKDIR /var/lib/manafield

USER 10001:10001

EXPOSE 8080

HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=5 \
    CMD ["curl", "--fail", "--silent", "--show-error", "http://127.0.0.1:8080/health"]

ENTRYPOINT ["/usr/local/bin/manafield-core"]

FROM debian:bookworm-slim AS build-plan-runtime

COPY --from=build-plan-builder /tmp/manafield-build-plan /usr/local/bin/manafield-build-plan

USER 10001:10001

ENTRYPOINT ["/usr/local/bin/manafield-build-plan"]

FROM debian:bookworm-slim AS ingress-traefik-runtime

COPY --from=ingress-traefik-builder /tmp/manafield-ingress-traefik /usr/local/bin/manafield-ingress-traefik

USER 10001:10001

ENTRYPOINT ["/usr/local/bin/manafield-ingress-traefik"]
