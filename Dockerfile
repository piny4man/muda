# syntax=docker/dockerfile:1
# Railway auto-detects a root Dockerfile (capital D) and builds this image.

FROM rust:1-bookworm AS builder

ENV CARGO_TERM_COLOR=always \
    CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse

RUN apt-get update \
    && apt-get install -y --no-install-recommends binaryen pkg-config ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Prebuilt cargo-leptos (compiling it from crates.io is much slower).
ADD https://github.com/cargo-bins/cargo-binstall/releases/latest/download/cargo-binstall-x86_64-unknown-linux-musl.tgz /tmp/cargo-binstall.tgz
RUN tar -xzf /tmp/cargo-binstall.tgz -C /usr/local/cargo/bin cargo-binstall \
    && rm /tmp/cargo-binstall.tgz \
    && cargo binstall cargo-leptos --version 0.3.7 -y

WORKDIR /app
COPY rust-toolchain.toml ./
RUN rustup show \
    && rustup target add wasm32-unknown-unknown

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Pin the cargo-leptos output dir so the copy below does not depend on
# how site-root is resolved in a workspace.
ENV LEPTOS_SITE_ROOT=/app/site
WORKDIR /app/crates/muda-web
RUN cargo leptos build --release

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home /app --shell /usr/sbin/nologin muda

WORKDIR /app
COPY --from=builder --chown=10001:10001 /app/target/release/muda-web /app/muda-web
COPY --from=builder --chown=10001:10001 /app/site /app/site
RUN chmod +x /app/muda-web

USER muda

# Railway overrides PORT. bind_addr() in main.rs listens on 0.0.0.0:$PORT.
ENV RUST_LOG=info \
    LEPTOS_ENV=PROD \
    LEPTOS_OUTPUT_NAME=muda-web \
    LEPTOS_SITE_ROOT=/app/site \
    LEPTOS_SITE_PKG_DIR=pkg \
    LEPTOS_SITE_ADDR=0.0.0.0:8080 \
    PORT=8080

EXPOSE 8080
CMD ["/app/muda-web"]
