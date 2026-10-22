FROM rust:1.99-bookworm@sha256:edaa2a8cc254d5936207af267114814c61cceea40b46eaa4e976068af55e20e7 AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY src ./src
COPY deploy/instance.bootstrap.yaml ./deploy/instance.bootstrap.yaml
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/src/target cargo build --locked --release --bin manafield && cp /src/target/release/manafield /tmp/manafield
FROM gcr.io/distroless/cc-debian13:nonroot@sha256:b7e81cb1af38d9345680e68bcf564d388f96304e25222030fc8c051faf8565b3 AS libs
FROM gcr.io/distroless/base-nossl-debian13:nonroot@sha256:83df5862465415c1d5f3ade85adc4d58a922db94e0b4d26c83a1bf96d5adf72d AS cli-content
COPY --from=libs /usr/lib/x86_64-linux-gnu/libgcc_s.so.1 /usr/lib/x86_64-linux-gnu/libgcc_s.so.1
COPY --from=libs /var/lib/dpkg/status.d/libgcc-s1* /var/lib/dpkg/status.d/
COPY --from=libs /var/lib/dpkg/status.d/gcc-14-base* /var/lib/dpkg/status.d/
COPY --from=libs /usr/share/doc/gcc-14-base/ /usr/share/doc/gcc-14-base/
COPY --from=build --chmod=0755 /tmp/manafield /usr/local/bin/manafield
FROM debian:trixie-slim@sha256:62bd423aeee912639ca8ee681c2b762cb3e25137e0a92b20be455898a9f034ad AS trust-store
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates=20250419+deb13u1 && rm -rf /var/lib/apt/lists/*
FROM trust-store AS assembled
COPY --from=cli-content / /rootfs/
COPY assemble-ca.sh /usr/local/bin/assemble-ca
RUN sh /usr/local/bin/assemble-ca
FROM scratch
COPY --from=assembled /rootfs/ /
WORKDIR /work
USER 65532:65532
ENTRYPOINT ["/usr/local/bin/manafield"]
