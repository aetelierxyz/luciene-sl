#!/usr/bin/env bash
# Publish the latest report.json to the on-chain oracle.
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

[ -f "$REPORT" ] || die "no report at $REPORT — run scripts/probe.sh first"

BIN="$(command -v publisher || true)"
[ -x "${BIN:-}" ] || BIN="$ROOT/target/release/publisher"
[ -x "$BIN" ] || BIN="cargo run --release -q -p coloc-publisher --"

log "publishing $REPORT to $CLUSTER ($RPC_URL)"
RUST_LOG="${RUST_LOG:-info}" $BIN \
  --report "$REPORT" \
  --rpc "$RPC_URL" \
  ${KEYPAIR:+--keypair "$KEYPAIR"}
