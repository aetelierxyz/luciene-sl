# RUNBOOK — CEX → AWS Colocation Oracle

End-to-end instructions to build, deploy and run the service. See
[ARCHITECTURE.md](ARCHITECTURE.md) for how it works and why.

## Components

```
crates/shared      types, AWS region tables, scoring, wire layout
crates/probe       bin: measures CEX REST/WSS latency, geolocates, recommends
crates/publisher   bin: writes the recommendation on-chain (Solana devnet)
crates/dashboard   bin: local map UI, reads the report back from chain
programs/coloc_oracle   Anchor program storing the ColocationReport PDA
scripts/*.sh       orchestration (build, deploy, probe, publish, dashboard, loop)
Dockerfile, docker-compose.yml
```

## Prerequisites

- Rust (>= 1.84), `cargo`
- Solana CLI + `cargo build-sbf` (only needed to (re)build/deploy the program)
- A funded **devnet** keypair (the Solana CLI config keypair is used by default)
- Docker (optional, for the containerised path)

## Quick start (native)

```bash
# 1. build off-chain binaries (+ the SBF program if the toolchain is present)
scripts/build.sh

# 2. probe → publish on devnet → serve the dashboard on http://127.0.0.1:8080
scripts/run-all.sh
```

Then open <http://127.0.0.1:8080>. The dashboard shows, per exchange, the
recommended AWS region, lat/lon and a 10 km zone, with a provenance banner
proving the data came from the on-chain account.

### Step by step

```bash
scripts/probe.sh       # writes dashboard/report.json (real measurements)
scripts/publish.sh     # sends set_colocation to devnet, updates report.json
scripts/dashboard.sh   # serves the map, reading the PDA back via JSON-RPC
```

### Continuous "service" mode

```bash
PROBE_INTERVAL=300 scripts/oracle-loop.sh   # re-probe + republish every 5 min
# in another shell:
scripts/dashboard.sh
```

## Deploying the program yourself

The program is already deployed to devnet at
`GH9e1ZjDTRo4WKgJYxE1TZHSFFqgXxsDVTB4manBvvh1`. To deploy your own copy:

```bash
# generate a program keypair (or reuse target/deploy/coloc_oracle-keypair.json)
solana-keygen new -o target/deploy/coloc_oracle-keypair.json

# put its pubkey in BOTH:
#   programs/coloc_oracle/src/lib.rs  -> declare_id!(...)
#   crates/shared/src/lib.rs          -> PROGRAM_ID
cargo build-sbf --manifest-path programs/coloc_oracle/Cargo.toml
scripts/deploy.sh        # uses --max-len so rent fits a modest balance
```

> Devnet airdrops are rate-limited. A ~200 KB program needs ~1.45 SOL of rent
> when deployed with `--max-len` (vs ~2.9 SOL for the default 2× sizing).

## Docker

```bash
# builds the off-chain binaries into a 175 MB image and starts two services:
#   oracle    -> probe + publish loop (needs your keypair)
#   dashboard -> map UI on :8080 (read-only, no keypair)
SOLANA_KEYPAIR=$HOME/.config/solana/id.json docker compose up --build
```

Override the cluster/account/RPC via env:

```bash
CLUSTER=devnet \
RPC_URL=https://api.devnet.solana.com \
COLOC_ACCOUNT=9cxKGbaLaeuU3sa9Z3X2DihSCBwUwwRcnm1t7EqhtbKa \
SOLANA_KEYPAIR=$HOME/.config/solana/id.json \
docker compose up --build
```

Run just the dashboard against the already-published account:

```bash
docker build -t coloc-oracle .
docker run -p 8080:8080 coloc-oracle \
  dashboard --bind 0.0.0.0:8080 --cluster devnet \
  --account 9cxKGbaLaeuU3sa9Z3X2DihSCBwUwwRcnm1t7EqhtbKa
```

## Tests

```bash
cargo test -p coloc-shared -p coloc-probe -p coloc-publisher
```

Covers: haversine + nearest-region mapping, CIDR matching, latency median,
micro-degree round-tripping, and — importantly — that the publisher's borsh wire
record is exactly the on-chain `VenueRecord::LEN` (61 bytes) and the Anchor
instruction discriminator is stable.

## Configuration (env vars honoured by the scripts)

| var              | default                              | meaning                       |
|------------------|--------------------------------------|-------------------------------|
| `CLUSTER`        | `devnet`                             | cluster label                 |
| `RPC_URL`        | `https://api.devnet.solana.com`      | Solana RPC                    |
| `REPORT`         | `dashboard/report.json`              | report path                   |
| `DASH_BIND`      | `127.0.0.1:8080`                     | dashboard bind addr           |
| `REST_SAMPLES`   | `5`                                  | REST latency samples/endpoint |
| `WS_SAMPLES`     | `3`                                  | WSS latency samples/endpoint  |
| `ONLY`           | (all)                                | e.g. `binance,coinbase`       |
| `KEYPAIR`        | Solana CLI config keypair            | publisher signer              |
| `PROBE_INTERVAL` | `300`                                | seconds between loop cycles   |

## Notes & honest caveats

- **One spot can't serve all three.** Binance (Tokyo), Coinbase (N. Virginia)
  and Kraken (Ireland) live on three continents — the dashboard therefore shows
  a 10 km recommendation **per exchange**, plus a "primary pick" (highest
  confidence). Colocating for all three means three deployments.
- **Latency is a weak region signal from a laptop.** Measured RTT mostly
  reflects proximity to CDN edges. Region selection is driven by AWS IP-range
  matches (authoritative) and documented engine locations; latency is shown for
  transparency. Run the probe *inside* a candidate AWS region for meaningful RTT.
- **Public data only.** No API keys, no authenticated endpoints, devnet only.
