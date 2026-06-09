#!/usr/bin/env bash
# Build the off-chain binaries and the on-chain program.
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

need cargo

log "building off-chain crates (release) …"
cargo build --release -p coloc-shared -p coloc-probe -p coloc-publisher -p coloc-dashboard

if command -v cargo-build-sbf >/dev/null 2>&1 || cargo build-sbf --version >/dev/null 2>&1; then
  log "building on-chain program (sbf) …"
  cargo build-sbf --manifest-path "$ROOT/programs/coloc_oracle/Cargo.toml"
else
  warn "cargo build-sbf not found; skipping on-chain program build"
fi

log "build complete."
