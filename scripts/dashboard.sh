#!/usr/bin/env bash
# Serve the local dashboard (reads the report back from chain).
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

BIN="$(command -v dashboard || true)"
[ -x "${BIN:-}" ] || BIN="$ROOT/target/release/dashboard"
[ -x "$BIN" ] || BIN="cargo run --release -q -p coloc-dashboard --"

log "dashboard on http://$DASH_BIND  (cluster=$CLUSTER)"
RUST_LOG="${RUST_LOG:-info}" $BIN \
  --bind "$DASH_BIND" \
  --report "$REPORT" \
  --rpc "$RPC_URL" \
  --cluster "$CLUSTER" \
  ${ACCOUNT:+--account "$ACCOUNT"}
