#!/usr/bin/env bash
# Deploy (or upgrade) the coloc_oracle program to the configured cluster.
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

need solana
SO="$ROOT/target/deploy/coloc_oracle.so"
[ -f "$SO" ] || die "missing $SO — run scripts/build.sh first"
[ -f "$PROGRAM_KEYPAIR" ] || die "missing program keypair $PROGRAM_KEYPAIR"

PROGRAM_ID="$(solana-keygen pubkey "$PROGRAM_KEYPAIR")"
log "deploying $PROGRAM_ID to $RPC_URL"
solana config set --url "$RPC_URL" >/dev/null

# Cap the program-data length so rent fits a modest wallet balance; bump if you
# expect to upgrade to a larger binary later.
MAX_LEN="${MAX_LEN:-208000}"

solana program deploy "$SO" \
  --program-id "$PROGRAM_KEYPAIR" \
  --max-len "$MAX_LEN"

log "deployed. program id: $PROGRAM_ID"
warn "ensure crates/shared/src/lib.rs PROGRAM_ID matches: $PROGRAM_ID"
