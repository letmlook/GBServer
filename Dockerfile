# syntax=docker/dockerfile:1.7
# =============================================================================
# WVP GB28181 Server — multi-stage Dockerfile
# Stage 1: build Vue 2 frontend (web/dist)
# Stage 2: build Rust backend (gbserver)
# Stage 3: minimal runtime image (debian-slim + binary + frontend assets)
# =============================================================================

# -----------------------------------------------------------------------------
# Stage 1: Frontend builder
# -----------------------------------------------------------------------------
FROM node:18.20-bookworm-slim AS frontend-builder

WORKDIR /build/web

# Install OS build tools required by node-gyp / sass
RUN apt-get update \
    && apt-get install -y --no-install-recommends python3 build-essential \
    && rm -rf /var/lib/apt/lists/*

# Cache npm install layer
COPY web/package.json web/package-lock.json* ./
RUN npm install --no-audit --no-fund --prefer-offline

# Build production assets
COPY web/ ./
RUN npm run build:prod \
    && test -d dist \
    && echo "frontend build OK: $(du -sh dist | cut -f1)"


# -----------------------------------------------------------------------------
# Stage 2: Backend builder (Rust)
# -----------------------------------------------------------------------------
FROM rust:1.82-bookworm AS backend-builder

# System deps for sqlx-postgres + OpenSSL
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        pkg-config \
        libssl-dev \
        libpq-dev \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Pre-fetch dependency layer (so source edits don't bust the cargo cache)
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src tests \
    && echo 'fn main() { println!("stub"); }' > src/main.rs \
    && echo '' > tests/.gitkeep \
    && cargo build --release \
    && rm -rf src tests target/release/gbserver* target/release/deps/gbserver-*

# Real build
COPY src ./src
COPY tests ./tests
COPY config ./config
COPY database ./database
RUN cargo build --release \
    && strip target/release/gbserver \
    && echo "backend build OK: $(du -sh target/release/gbserver | cut -f1)"


# -----------------------------------------------------------------------------
# Stage 3: Runtime image
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# Runtime libs: libpq5 (postgres), libssl3 (reqwest rustls is static but keep ssl for compat),
# tini (proper signal handling), tzdata for log timestamps, ca-certificates for TLS
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        libssl3 \
        libpq5 \
        tini \
        tzdata \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -r -g 10001 gbserver \
    && useradd -r -u 10001 -g gbserver -m -d /app -s /usr/sbin/nologin gbserver

WORKDIR /app

# Backend binary
COPY --from=backend-builder --chown=gbserver:gbserver /build/target/release/gbserver /app/gbserver

# Config files and DB init scripts (mounted into postgres container too)
COPY --chown=gbserver:gbserver config /app/config
COPY --chown=gbserver:gbserver database /app/database

# Frontend built assets (served as static files by the Rust backend)
COPY --from=frontend-builder --chown=gbserver:gbserver /build/web/dist /app/web/dist

USER gbserver

# Default exposed ports (overridable via docker-compose):
#   18080  — HTTP API + static frontend
#   5060/udp — SIP signaling
#   5061    — SIP over TCP
EXPOSE 18080 5060/udp 5061

# Healthcheck hits the public /api/health endpoint (port follows WVP__SERVER__PORT override)
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
    CMD sh -c 'wget -qO- "http://127.0.0.1:${WVP__SERVER__PORT:-18080}/api/health" || exit 1'

# tini reaps zombies and forwards signals (Ctrl-C, docker stop)
ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/app/gbserver"]