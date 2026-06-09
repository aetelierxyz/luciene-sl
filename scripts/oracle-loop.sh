#!/usr/bin/env bash
# Continuous oracle: re-probe and re-publish on chain every PROBE_INTERVAL secs.
# This is the "service" loop — run it alongside the dashboard.
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

log "oracle loop started (interval=${PROBE_INTERVAL}s, cluster=$CLUSTER)"
while true; do
  if "$ROOT/scripts/probe.sh"; then
    "$ROOT/scripts/publish.sh" || warn "publish failed; will retry next cycle"
  else
    warn "probe failed; will retry next cycle"
  fi
  log "sleeping ${PROBE_INTERVAL}s …"
  sleep "$PROBE_INTERVAL"
done
