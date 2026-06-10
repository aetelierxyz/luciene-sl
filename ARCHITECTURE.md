# Luciene SL — CEX Colocation Oracle

A service that probes the **public** REST and WSS APIs of centralized exchanges
(Binance, Coinbase, Kraken, Bybit, Bitfinex, OKX, Gemini, Bitget, Gate.io,
KuCoin, HTX, MEXC, BitMart, Bitstamp, Crypto.com, Bitso, bitFlyer,
Mercado Bitcoin, NDAX, Bitvavo, Bithumb — spot markets),
determines where each exchange's
infrastructure physically lives (AWS region / availability zone), and emits an
on-chain recommendation of the best place to colocate a trading server.

The end deliverable is a **local dashboard** that shows, for each exchange and
for the aggregate, a latitude / longitude point plus a **10 km radius** marking
the best colocation spot — sourced from data stored **on Solana devnet**.

## Why an oracle pattern?

Solana programs cannot make outbound network calls. So the system is split:

```
   ┌─────────────┐   public REST/WSS    ┌──────────────────────┐
   │   probe     │ ───────────────────► │  Binance / Coinbase  │
   │ (off-chain) │ ◄─────────────────── │  / Kraken (spot)     │
   └──────┬──────┘   latency samples    └──────────────────────┘
          │
          │  resolve endpoint IPs → match AWS ip-ranges.json
          │  → geolocate (ip-api.com) → map to AWS region
          │  → score & recommend (region centroid + 10km radius)
          ▼
   report.json  (local cache, "tangible values")
          │
          ▼
   ┌─────────────┐   Anchor ix (borsh)  ┌──────────────────────┐
   │  publisher  │ ───────────────────► │  Solana devnet        │
   │ (off-chain) │                      │  luciene program      │
   └─────────────┘                      │  ColocationReport PDA │
                                        └──────────┬───────────┘
                                                   │ getAccountInfo (JSON-RPC)
                                                   ▼
                                        ┌──────────────────────┐
                                        │  dashboard (tokio)    │
                                        │  Leaflet map + cards  │
                                        └──────────────────────┘
```

## Components

| Path                  | Kind          | Stack                                            |
|-----------------------|---------------|--------------------------------------------------|
| `programs/luciene_sl` | Anchor program| `anchor-lang` — adds `ColocationReport` account  |
| `crates/shared`       | lib           | `serde`, region tables, scoring math, discriminators |
| `crates/probe`        | bin           | `reqwest`, `tokio-tungstenite`, `async-rate-limiter` |
| `crates/publisher`    | bin           | `solana-client`, `solana-sdk`, `borsh`           |
| `crates/dashboard`    | bin           | `tokio` (hand-rolled HTTP), `reqwest` (RPC read) |

## Methodology & confidence

For each exchange we resolve **every** REST, WSS and direct/FIX hostname to its
IPs, then determine the hosting AWS region by **evidence tally**, in order of
confidence:

1. **`aws-ip-range`** (HIGH, fully empirical) — IPs that fall inside an AWS
   *compute-region* CIDR (`https://ip-ranges.amazonaws.com/ip-ranges.json`). We
   count how many of the resolved IPs land in each region and pick the winner;
   `confidence = 0.75 + 0.20 · (region_IPs / total_IPs)`. CloudFront/"GLOBAL"
   edge ranges are excluded — they are CDN, not the engine.
2. **`curated`** (LOW) — every endpoint is behind a CDN (we detect Cloudflare
   ranges directly), so the origin region is **not network-detectable**. We fall
   back to documented knowledge and say so, with low confidence.
3. **`ip-geo-nearest`** (MEDIUM) — a non-AWS, non-CDN host: geolocate via
   `ip-api.com` (no token) and snap to the nearest AWS region centroid.

What this yields for the twenty-one venues (verified live):

| Venue    | Detection | Evidence |
|----------|-----------|----------|
| Binance  | `aws-ip-range` → `ap-northeast-1` | `api1/2/3`, `stream`, `ws-api`, `data-api.binance.vision` all in Tokyo (~29/30 IPs) |
| Coinbase | `aws-ip-range` → `us-east-1` | `api`/`ws-feed` are Cloudflare, but `ws-direct.exchange.coinbase.com` + `fix.exchange.coinbase.com` expose real `us-east-1` IPs |
| Gemini   | `aws-ip-range` → `us-east-1` | `api.gemini.com` resolves to real `us-east-1` IPs (12/12) |
| Gate.io  | `aws-ip-range` → `ap-northeast-1` | `api.gateio.ws` + `ws.gate.io` resolve to real Tokyo IPs |
| Bitstamp | `aws-ip-range` → `eu-central-1` | `ws.bitstamp.net` resolves to real Frankfurt IPs (REST `www` is Imperva) |
| Bithumb  | `aws-ip-range` → `ap-northeast-2` | `api.bithumb.com` resolves to real Seoul IPs (`pubwss` is Akamai) |
| NDAX     | `ip-geo-nearest` → `ca-central-1` | `api.ndax.io` is OVHcloud Canada (not AWS) → geolocates to Montreal |
| Kraken   | `curated` → `eu-west-1` (low conf) | every endpoint is Cloudflare anycast (`104.17.x`); origin not detectable |
| Bybit    | `curated` → `ap-southeast-1` (low conf) | every endpoint is AWS CloudFront (`GLOBAL` edges); origin not detectable |
| OKX      | `curated` → `ap-east-1` (low conf) | `www`/`ws.okx.com` are Cloudflare; documented engine in AWS Hong Kong |
| Bitget   | `curated` → `ap-southeast-1` (low conf) | `api` Cloudflare, `ws` AWS CloudFront; documented AWS Singapore |
| KuCoin   | `curated` → `ap-southeast-1` (low conf) | Cloudflare / CloudFront; WS needs a token, so REST-only probe |
| HTX/Huobi| `curated` → `ap-northeast-1` (low conf) | `api` + `api-aws` are CloudFront; documented AWS Tokyo |
| MEXC     | `curated` → `ap-northeast-1` (low conf) | REST Akamai, WS AWS CloudFront; documented AWS Tokyo |
| Crypto.com | `curated` → `ap-southeast-1` (low conf) | Cloudflare-fronted; Singapore-based |
| Bitso    | `curated` → `us-east-1` (low conf) | Cloudflare-fronted; LatAm infra documented in us-east-1 |
| Mercado Bitcoin | `curated` → `sa-east-1` (low conf) | Cloudflare-fronted; Brazil-based → São Paulo |
| bitFlyer | `curated` → `ap-northeast-1` (low conf) | on Azure Japan East behind Akamai (not AWS); nearest AWS is Tokyo |
| Bitvavo  | `curated` → `eu-central-1` (low conf) | Cloudflare-fronted; EU region undisclosed — flagged as a guess |
| BitMart  | `curated` → `ap-southeast-1` (very low conf) | Cloudflare-fronted; region undisclosed — flagged as a guess |
| Bitfinex | `curated` → `ap-northeast-1` (very low conf) | Cloudflare-fronted; region undisclosed — flagged as a guess |

### Latency, stability & score

Each endpoint is sampled multiple times to produce **median / min / p95 / jitter
(stddev) / success-rate**. Per venue:

- `stability = success_rate / (1 + jitter/median)`  (0..1; high = stable;
  uses *relative* jitter so it is robust to absolute latency and outliers)
- `score = 0.6 · region_confidence + 0.4 · stability`  → ranks the venues and
  selects the **primary pick**.

> The on-chain account stores a fixed `[VenueRecord; MAX_VENUES]` (MAX_VENUES =
> 21), so **all 21 venues are anchored on-chain**. Because a Solana transaction
> caps at 1232 bytes and 21 × 61-byte records exceed that, the report is written
> in chunks: `init_report` (header + allocation) then one `set_venue` per
> exchange. If the dashboard ever shows more venues than the account holds, the
> extras are flagged "measured · pending on-chain".

> Probe-host RTT measures *laptop → endpoint* (often a CDN edge), so it is
> **reported but not used to rank** venues — the colocation answer is "be in the
> same AWS region as the engine", where latency collapses toward sub-millisecond.
> Run the probe inside a candidate region for in-region RTT.

The compact core (region, lat/lon, radius, confidence, method, latency) is stored
**on-chain**; the richer stability telemetry is merged into the dashboard from the
latest measured `report.json`. Every recommendation carries `method` + numeric
`confidence` so the UI is honest about how each point was derived.

## On-chain / off-chain data boundary

- **On-chain (devnet):** only the final, compact recommendation per exchange
  (region code, lat/lon in micro-degrees, radius, latency medians, confidence,
  timestamp). Public data only — no API keys, no authenticated endpoints.
- **Off-chain:** all probing, DNS, geolocation, scoring.

See `RUNBOOK.md` for how to build and run everything (native + Docker).
