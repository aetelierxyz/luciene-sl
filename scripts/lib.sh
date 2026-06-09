#!/usr/bin/env bash
# Shared helpers + config for the colocation oracle scripts.
set -euo pipefail

# Repo root (this file lives in <root>/scripts).
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# --- configurable via environment -------------------------------------------
export CLUSTER="${CLUSTER:-devnet}"
export RPC_URL="${RPC_URL:-https://api.devnet.solana.com}"
export REPORT="${REPORT:-$ROOT/dashboard/report.json}"
export DASH_BIND="${DASH_BIND:-127.0.0.1:8080}"
export PROGRAM_KEYPAIR="${PROGRAM_KEYPAIR:-$ROOT/target/deploy/coloc_oracle-keypair.json}"
export PROBE_INTERVAL="${PROBE_INTERVAL:-300}" # seconds between probe loops

log()  { printf '\033[1;36m[coloc]\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m[coloc]\033[0m %s\n' "$*" >&2; }
die()  { printf '\033[1;31m[coloc]\033[0m %s\n' "$*" >&2; exit 1; }

need() { command -v "$1" >/dev/null 2>&1 || die "missing required tool: $1"; }
