//! Static catalogue of the exchanges we probe and documented colocation facts.
//!
//! Only **public, unauthenticated** spot-market endpoints are listed here.

/// A probe target for one exchange.
pub struct VenueSpec {
    /// Short id used as the on-chain key and in JSON.
    pub id: &'static str,
    pub display: &'static str,
    /// Public REST endpoints (returns quickly, no auth). First is primary.
    pub rest_urls: &'static [&'static str],
    /// Public WSS endpoints (no auth). First is primary.
    pub ws_urls: &'static [&'static str],
    /// Extra hostnames resolved for AWS-region evidence only (not latency-probed
    /// because they need auth / FIX / IP-allowlisting, but their DNS reveals the
    /// real hosting region). Format: "host:port".
    pub resolve_hosts: &'static [&'static str],
    /// First message to send after WS connect to trigger a server response,
    /// or empty to just measure the handshake + first frame.
    pub ws_subscribe: &'static str,
    /// Documented AWS region of the matching engine (the `curated` fallback,
    /// used only when no endpoint exposes a real AWS IP).
    pub curated_region: &'static str,
    /// Confidence (0..1) we assign to the curated fallback for this venue.
    pub curated_confidence: f64,
    /// Human note explaining the curated choice.
    pub curated_note: &'static str,
}

pub const VENUES: &[VenueSpec] = &[
    VenueSpec {
        id: "binance",
        display: "Binance (spot)",
        rest_urls: &[
            "https://api.binance.com/api/v3/ticker/price?symbol=BTCUSDT",
            "https://api1.binance.com/api/v3/ticker/price?symbol=BTCUSDT",
            "https://data-api.binance.vision/api/v3/ticker/price?symbol=BTCUSDT",
        ],
        ws_urls: &["wss://stream.binance.com:9443/ws/btcusdt@trade"],
        // ws-api + the public market-data mirror — all in ap-northeast-1.
        resolve_hosts: &[
            "ws-api.binance.com:443",
            "data-stream.binance.vision:443",
            "api2.binance.com:443",
        ],
        ws_subscribe: "",
        curated_region: "ap-northeast-1",
        curated_confidence: 0.80,
        curated_note: "Binance spot matching engine is documented in AWS Tokyo (ap-northeast-1).",
    },
    VenueSpec {
        id: "coinbase",
        display: "Coinbase (spot)",
        rest_urls: &["https://api.exchange.coinbase.com/products/BTC-USD/ticker"],
        ws_urls: &["wss://ws-feed.exchange.coinbase.com"],
        // The public ws-feed/api are Cloudflare-fronted, but the direct market
        // data feed and FIX gateway expose Coinbase's real AWS us-east-1 IPs.
        resolve_hosts: &[
            "ws-direct.exchange.coinbase.com:443",
            "fix.exchange.coinbase.com:4198",
        ],
        ws_subscribe: "{\"type\":\"subscribe\",\"product_ids\":[\"BTC-USD\"],\"channels\":[\"ticker\"]}",
        curated_region: "us-east-1",
        curated_confidence: 0.85,
        curated_note: "Coinbase Exchange runs in AWS N. Virginia (us-east-1).",
    },
    VenueSpec {
        id: "kraken",
        display: "Kraken (spot)",
        rest_urls: &["https://api.kraken.com/0/public/Ticker?pair=XBTUSD"],
        ws_urls: &["wss://ws.kraken.com/"],
        resolve_hosts: &[],
        ws_subscribe: "{\"event\":\"subscribe\",\"pair\":[\"XBT/USD\"],\"subscription\":{\"name\":\"ticker\"}}",
        // Every Kraken public endpoint is behind Cloudflare anycast (104.17.x);
        // the origin AWS region is NOT network-detectable from public data.
        curated_region: "eu-west-1",
        curated_confidence: 0.45,
        curated_note: "All Kraken public endpoints are behind Cloudflare anycast; the origin region is not network-detectable. Documented infra region (AWS Europe) shown — verify by deploying probes inside candidate regions.",
    },
];

pub fn venue_by_id(id: &str) -> Option<&'static VenueSpec> {
    VENUES.iter().find(|v| v.id == id)
}
