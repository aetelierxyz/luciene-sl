#!/usr/bin/env bash
# One-shot: probe → publish → serve dashboard.
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

"$ROOT/scripts/probe.sh"
"$ROOT/scripts/publish.sh"
exec "$ROOT/scripts/dashboard.sh"
