#!/usr/bin/env bash
# Run the off-chain probe and write the JSON report.
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

BIN="$(command -v probe || true)"
[ -x "${BIN:-}" ] || BIN="$ROOT/target/release/probe"
[ -x "$BIN" ] || BIN="cargo run --release -q -p coloc-probe --"

log "probing exchanges → $REPORT"
RUST_LOG="${RUST_LOG:-info}" $BIN \
  --rest-samples "${REST_SAMPLES:-5}" \
  --ws-samples "${WS_SAMPLES:-3}" \
  --out "$REPORT" \
  ${ONLY:+--only "$ONLY"}
