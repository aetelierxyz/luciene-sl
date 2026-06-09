# syntax=docker/dockerfile:1
# Multi-stage build for the off-chain colocation oracle binaries.
# (The on-chain program is built/deployed separately with cargo build-sbf.)

FROM rust:1-bookworm AS builder
WORKDIR /build

# System deps that the Solana client dependency tree expects on Linux.
RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config libudev-dev protobuf-compiler clang cmake \
    && rm -rf /var/lib/apt/lists/*

# Copy the whole workspace (off-chain crates + program manifests).
COPY Cargo.toml ./
COPY crates ./crates
COPY programs ./programs

# Build only the off-chain binaries (skip the SBF program).
RUN cargo build --release \
        -p coloc-probe -p coloc-publisher -p coloc-dashboard

# ---------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/probe      /usr/local/bin/probe
COPY --from=builder /build/target/release/publisher  /usr/local/bin/publisher
COPY --from=builder /build/target/release/dashboard  /usr/local/bin/dashboard
COPY scripts ./scripts

# Shared volume for report.json.
RUN mkdir -p /app/dashboard
ENV REPORT=/app/dashboard/report.json

EXPOSE 8080
# Default: serve the dashboard. Override the command for probe/publisher/loop.
CMD ["dashboard", "--bind", "0.0.0.0:8080"]
