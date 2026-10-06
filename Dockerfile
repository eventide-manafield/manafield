# syntax=docker/dockerfile:1.7

FROM rust:1.99-bookworm AS builder

WORKDIR /src

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY src ./src

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release --bin manafield \
    && cp /src/target/release/manafield /tmp/manafield

FROM debian:bookworm-slim AS core-runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /var/lib/manafield/modules

COPY --from=builder /tmp/manafield /usr/local/bin/manafield

ENV MANAFIELD_BIND=0.0.0.0:8080 \
    MANAFIELD_MODULES_DIR=/var/lib/manafield/modules \
    RUST_LOG=manafield=info

WORKDIR /var/lib/manafield

USER 10001:10001

EXPOSE 8080

HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=5 \
    CMD ["curl", "--fail", "--silent", "--show-error", "http://127.0.0.1:8080/health"]

ENTRYPOINT ["/usr/local/bin/manafield"]
