# Multi-stage build for http-client-pro web server.
# Uses the release profile (lto + strip + opt-level=s) for a small binary.

FROM rust:1-bookworm AS builder

WORKDIR /app

# Copy the entire workspace (Cargo.toml, Cargo.lock, crates/, spec.md).
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY spec.md ./

# Build the web binary in release mode. The release profile in the
# workspace Cargo.toml enables LTO, single codegen unit, and stripping.
RUN cargo build --release --bin http-client-pro-web

# ---- runtime ----
FROM debian:bookworm-slim

# ca-certificates: rustls needs root certs to verify TLS chains.
# curl: used by the HEALTHCHECK probe.
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl \
 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/http-client-pro-web /usr/local/bin/http-client-pro-web

ENV HTTP_WEB_ADDR=0.0.0.0:8080
EXPOSE 8080

# Liveness probe: hit /healthz (no auth required) every 30s.
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -fsS http://127.0.0.1:8080/healthz || exit 1

ENTRYPOINT ["/usr/local/bin/http-client-pro-web"]
