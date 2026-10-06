# syntax=docker/dockerfile:1.7

FROM rust:1.99-bookworm AS source

WORKDIR /src

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY src ./src

RUN rustup component add rustfmt clippy

FROM source AS verify

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo fmt --check \
    && cargo check --locked \
    && cargo test --locked \
    && cargo clippy --locked -- -D warnings

FROM source AS core-builder

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release --bin manafield \
    && cp /src/target/release/manafield /tmp/manafield

FROM source AS build-plan-builder

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release --features build-plan --bin manafield-build-plan \
    && cp /src/target/release/manafield-build-plan /tmp/manafield-build-plan

FROM debian:bookworm-slim AS core-runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /var/lib/manafield/modules

COPY --from=core-builder /tmp/manafield /usr/local/bin/manafield

ENV MANAFIELD_BIND=0.0.0.0:8080 \
    MANAFIELD_MODULES_DIR=/var/lib/manafield/modules \
    RUST_LOG=manafield=info

WORKDIR /var/lib/manafield

USER 10001:10001

EXPOSE 8080

HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=5 \
    CMD ["curl", "--fail", "--silent", "--show-error", "http://127.0.0.1:8080/health"]

ENTRYPOINT ["/usr/local/bin/manafield"]

FROM debian:bookworm-slim AS build-plan-runtime

COPY --from=build-plan-builder /tmp/manafield-build-plan /usr/local/bin/manafield-build-plan

USER 10001:10001

ENTRYPOINT ["/usr/local/bin/manafield-build-plan"]
