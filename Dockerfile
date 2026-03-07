# syntax=docker/dockerfile:1.7

FROM rust:1.86-slim-bookworm AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release --bin janusd --bin janus-mcp

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        git \
        openssh-client \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --create-home --shell /usr/sbin/nologin janus

WORKDIR /home/janus

COPY --from=builder /app/target/release/janusd /usr/local/bin/janusd
COPY --from=builder /app/target/release/janus-mcp /usr/local/bin/janus-mcp
COPY scripts/docker/janus-entrypoint.sh /usr/local/bin/janus-entrypoint.sh

ENV JANUS_PROXY_BIND=0.0.0.0:9080
ENV JANUS_CONTROL_SOCKET=/var/run/janus/janusd-control.sock
ENV JANUS_GIT_SSH_AUTH_SOCK=/var/run/janus/ssh-agent.sock

RUN chmod +x /usr/local/bin/janus-entrypoint.sh \
    && mkdir -p /var/run/janus \
    && chown -R janus:janus /var/run/janus /home/janus

USER janus

EXPOSE 9080

ENTRYPOINT ["/usr/local/bin/janus-entrypoint.sh"]
