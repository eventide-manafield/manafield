FROM rust:1.99-bookworm@sha256:edaa2a8cc254d5936207af267114814c61cceea40b46eaa4e976068af55e20e7 AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY src ./src
COPY deploy/instance.bootstrap.yaml ./deploy/instance.bootstrap.yaml
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/src/target cargo build --locked --release --bin manafield && cp /src/target/release/manafield /tmp/manafield
FROM gcr.io/distroless/cc-debian13:nonroot@sha256:b7e81cb1af38d9345680e68bcf564d388f96304e25222030fc8c051faf8565b3 AS libs
FROM gcr.io/distroless/base-nossl-debian13:nonroot@sha256:83df5862465415c1d5f3ade85adc4d58a922db94e0b4d26c83a1bf96d5adf72d
COPY --from=libs /usr/lib/x86_64-linux-gnu/libgcc_s.so.1 /usr/lib/x86_64-linux-gnu/libgcc_s.so.1
COPY --from=libs /var/lib/dpkg/status.d/libgcc-s1* /var/lib/dpkg/status.d/
COPY --from=libs /var/lib/dpkg/status.d/gcc-14-base* /var/lib/dpkg/status.d/
COPY --from=libs /usr/share/doc/gcc-14-base/ /usr/share/doc/gcc-14-base/
COPY --from=build --chmod=0755 /tmp/manafield /usr/local/bin/manafield
WORKDIR /work
USER 65532:65532
ENTRYPOINT ["/usr/local/bin/manafield"]
