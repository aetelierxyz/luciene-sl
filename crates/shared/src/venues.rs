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
    VenueSpec {
        id: "bybit",
        display: "Bybit (spot)",
        rest_urls: &["https://api.bybit.com/v5/market/tickers?category=spot&symbol=BTCUSDT"],
        ws_urls: &["wss://stream.bybit.com/v5/public/spot"],
        resolve_hosts: &[],
        ws_subscribe: "{\"op\":\"subscribe\",\"args\":[\"tickers.BTCUSDT\"]}",
        // Every Bybit public endpoint is behind AWS CloudFront (GLOBAL edge IPs);
        // the origin compute region is not network-detectable. Bybit's spot
        // engine is documented in AWS Singapore.
        curated_region: "ap-southeast-1",
        curated_confidence: 0.50,
        curated_note: "All Bybit public endpoints are behind AWS CloudFront (GLOBAL edges); the origin region is not network-detectable. Documented infra region (AWS Singapore, ap-southeast-1) shown — verify by deploying probes inside candidate regions.",
    },
    VenueSpec {
        id: "bitfinex",
        display: "Bitfinex (spot)",
        rest_urls: &["https://api-pub.bitfinex.com/v2/ticker/tBTCUSD"],
        ws_urls: &["wss://api-pub.bitfinex.com/ws/2"],
        resolve_hosts: &[],
        ws_subscribe: "{\"event\":\"subscribe\",\"channel\":\"ticker\",\"symbol\":\"tBTCUSD\"}",
        // api-pub.bitfinex.com is Cloudflare-fronted; origin not detectable and
        // the public AWS region is not officially disclosed — low confidence.
        curated_region: "ap-northeast-1",
        curated_confidence: 0.35,
        curated_note: "Bitfinex public endpoints are behind Cloudflare; origin region is not network-detectable and not officially disclosed. Low-confidence documented guess (AWS Tokyo) — verify by deploying probes inside candidate regions.",
    },
    VenueSpec {
        id: "okx",
        display: "OKX (spot)",
        rest_urls: &["https://www.okx.com/api/v5/market/ticker?instId=BTC-USDT"],
        ws_urls: &["wss://ws.okx.com:8443/ws/v5/public"],
        resolve_hosts: &[],
        ws_subscribe: "{\"op\":\"subscribe\",\"args\":[{\"channel\":\"tickers\",\"instId\":\"BTC-USDT\"}]}",
        // www.okx.com / ws.okx.com are Cloudflare-fronted; OKX's documented
        // matching-engine region is AWS Hong Kong.
        curated_region: "ap-east-1",
        curated_confidence: 0.50,
        curated_note: "OKX public endpoints are behind Cloudflare; the origin region is not network-detectable. Documented infra region (AWS Hong Kong, ap-east-1) shown — verify by deploying probes inside candidate regions.",
    },
    VenueSpec {
        id: "gemini",
        display: "Gemini (spot)",
        rest_urls: &["https://api.gemini.com/v1/pubticker/btcusd"],
        // Gemini's public market-data WS streams immediately on connect (no auth).
        ws_urls: &["wss://api.gemini.com/v1/marketdata/BTCUSD"],
        resolve_hosts: &[],
        ws_subscribe: "",
        // api.gemini.com resolves to real AWS us-east-1 IPs → fully empirical.
        curated_region: "us-east-1",
        curated_confidence: 0.85,
        curated_note: "Gemini runs in AWS N. Virginia (us-east-1).",
    },
    VenueSpec {
        id: "bitget",
        display: "Bitget (spot)",
        rest_urls: &["https://api.bitget.com/api/v2/spot/market/tickers?symbol=BTCUSDT"],
        ws_urls: &["wss://ws.bitget.com/v2/ws/public"],
        resolve_hosts: &[],
        ws_subscribe: "{\"op\":\"subscribe\",\"args\":[{\"instType\":\"SPOT\",\"channel\":\"ticker\",\"instId\":\"BTCUSDT\"}]}",
        // api is Cloudflare, ws is AWS CloudFront — origin not detectable.
        curated_region: "ap-southeast-1",
        curated_confidence: 0.45,
        curated_note: "Bitget public endpoints are behind Cloudflare / AWS CloudFront; the origin region is not network-detectable. Documented infra region (AWS Singapore, ap-southeast-1) shown — verify by deploying probes inside candidate regions.",
    },
    VenueSpec {
        id: "gateio",
        display: "Gate.io (spot)",
        rest_urls: &["https://api.gateio.ws/api/v4/spot/tickers?currency_pair=BTC_USDT"],
        ws_urls: &["wss://api.gateio.ws/ws/v4/"],
        resolve_hosts: &["ws.gate.io:443"],
        ws_subscribe: "{\"channel\":\"spot.tickers\",\"event\":\"subscribe\",\"payload\":[\"BTC_USDT\"]}",
        // api.gateio.ws + ws.gate.io resolve to real AWS ap-northeast-1 IPs.
        curated_region: "ap-northeast-1",
        curated_confidence: 0.80,
        curated_note: "Gate.io runs in AWS Tokyo (ap-northeast-1).",
    },
    VenueSpec {
        id: "kucoin",
        display: "KuCoin (spot)",
        // KuCoin's WS needs a token from POST /bullet-public, so we probe REST
        // latency and use the ws host for region evidence only.
        rest_urls: &["https://api.kucoin.com/api/v1/market/orderbook/level1?symbol=BTC-USDT"],
        ws_urls: &[],
        resolve_hosts: &["ws-api-spot.kucoin.com:443"],
        ws_subscribe: "",
        // api is Cloudflare, ws-api-spot is AWS CloudFront — origin not detectable.
        curated_region: "ap-southeast-1",
        curated_confidence: 0.50,
        curated_note: "KuCoin public endpoints are behind Cloudflare / AWS CloudFront; the origin region is not network-detectable. Documented infra region (AWS Singapore, ap-southeast-1) shown — verify by deploying probes inside candidate regions.",
    },
    VenueSpec {
        id: "htx",
        display: "HTX / Huobi (spot)",
        rest_urls: &["https://api.huobi.pro/market/detail/merged?symbol=btcusdt"],
        ws_urls: &["wss://api.huobi.pro/ws"],
        resolve_hosts: &["api-aws.huobi.pro:443"],
        ws_subscribe: "{\"sub\":\"market.btcusdt.ticker\",\"id\":\"id1\"}",
        // Both api.huobi.pro and api-aws.huobi.pro are AWS CloudFront edges —
        // origin not detectable. HTX is documented in AWS Tokyo.
        curated_region: "ap-northeast-1",
        curated_confidence: 0.55,
        curated_note: "HTX (Huobi) public endpoints are behind AWS CloudFront (GLOBAL edges); the origin region is not network-detectable. Documented infra region (AWS Tokyo, ap-northeast-1) shown — verify by deploying probes inside candidate regions.",
    },
    VenueSpec {
        id: "mexc",
        display: "MEXC (spot)",
        rest_urls: &["https://api.mexc.com/api/v3/ticker/price?symbol=BTCUSDT"],
        ws_urls: &["wss://wbs-api.mexc.com/ws"],
        resolve_hosts: &["wbs.mexc.com:443"],
        ws_subscribe: "{\"method\":\"SUBSCRIPTION\",\"params\":[\"spot@public.deals.v3.api@BTCUSDT\"]}",
        // REST is Akamai, WS is AWS CloudFront — origin not detectable.
        curated_region: "ap-northeast-1",
        curated_confidence: 0.45,
        curated_note: "MEXC REST is behind Akamai and its WS behind AWS CloudFront; the origin region is not network-detectable. Documented infra region (AWS Tokyo, ap-northeast-1) shown — verify by deploying probes inside candidate regions.",
    },
    VenueSpec {
        id: "bitmart",
        display: "BitMart (spot)",
        rest_urls: &["https://api-cloud.bitmart.com/spot/quotation/v3/ticker?symbol=BTC_USDT"],
        ws_urls: &["wss://ws-manager-compress.bitmart.com/api?protocol=1.1"],
        resolve_hosts: &[],
        ws_subscribe: "{\"op\":\"subscribe\",\"args\":[\"spot/ticker:BTC_USDT\"]}",
        // Cloudflare-fronted; origin region not detectable nor officially disclosed.
        curated_region: "ap-southeast-1",
        curated_confidence: 0.40,
        curated_note: "BitMart public endpoints are behind Cloudflare; origin region not network-detectable and not officially disclosed. Low-confidence documented guess (AWS Singapore) — verify by deploying probes inside candidate regions.",
    },
    VenueSpec {
        id: "bitstamp",
        display: "Bitstamp (spot)",
        rest_urls: &["https://www.bitstamp.net/api/v2/ticker/btcusd/"],
        ws_urls: &["wss://ws.bitstamp.net"],
        resolve_hosts: &[],
        ws_subscribe: "{\"event\":\"bts:subscribe\",\"data\":{\"channel\":\"live_trades_btcusd\"}}",
        // ws.bitstamp.net resolves to real AWS eu-central-1 IPs → empirical.
        curated_region: "eu-central-1",
        curated_confidence: 0.85,
        curated_note: "Bitstamp runs in AWS Frankfurt (eu-central-1).",
    },
    VenueSpec {
        id: "cryptocom",
        display: "Crypto.com (spot)",
        rest_urls: &["https://api.crypto.com/exchange/v1/public/get-tickers?instrument_name=BTC_USDT"],
        ws_urls: &["wss://stream.crypto.com/exchange/v1/market"],
        resolve_hosts: &[],
        ws_subscribe: "{\"id\":1,\"method\":\"subscribe\",\"params\":{\"channels\":[\"ticker.BTC_USDT\"]}}",
        // Cloudflare-fronted; Crypto.com is Singapore-based.
        curated_region: "ap-southeast-1",
        curated_confidence: 0.50,
        curated_note: "Crypto.com public endpoints are behind Cloudflare; the origin region is not network-detectable. Documented infra region (AWS Singapore, ap-southeast-1) shown — verify by deploying probes inside candidate regions.",
    },
    VenueSpec {
        id: "bitso",
        display: "Bitso (spot)",
        rest_urls: &["https://api.bitso.com/v3/ticker/?book=btc_mxn"],
        ws_urls: &["wss://ws.bitso.com"],
        resolve_hosts: &[],
        ws_subscribe: "{\"action\":\"subscribe\",\"book\":\"btc_mxn\",\"type\":\"trades\"}",
        // Cloudflare-fronted; Bitso (LatAm) infra documented in AWS us-east-1.
        curated_region: "us-east-1",
        curated_confidence: 0.45,
        curated_note: "Bitso public endpoints are behind Cloudflare; the origin region is not network-detectable. Documented infra region (AWS N. Virginia, us-east-1, serving LatAm) shown — verify by deploying probes inside candidate regions.",
    },
    VenueSpec {
        id: "bitflyer",
        display: "bitFlyer (spot)",
        rest_urls: &["https://api.bitflyer.com/v1/ticker?product_code=BTC_JPY"],
        ws_urls: &["wss://ws.lightstream.bitflyer.com/json-rpc"],
        resolve_hosts: &[],
        ws_subscribe: "{\"method\":\"subscribe\",\"params\":{\"channel\":\"lightning_ticker_BTC_JPY\"},\"id\":1}",
        // REST is Akamai, WS is Microsoft Azure (Japan East) — not on AWS.
        curated_region: "ap-northeast-1",
        curated_confidence: 0.55,
        curated_note: "bitFlyer runs on Azure Japan East (Tokyo) behind Akamai — not on AWS. Nearest AWS region (ap-northeast-1, Tokyo) shown for colocation reference.",
    },
    VenueSpec {
        // id kept <= 12 bytes (on-chain EXCHANGE_LEN); display name is separate.
        id: "mercado",
        display: "Mercado Bitcoin (spot)",
        rest_urls: &["https://api.mercadobitcoin.net/api/v4/tickers?symbols=BTC-BRL"],
        ws_urls: &[],
        resolve_hosts: &[],
        ws_subscribe: "",
        // Cloudflare-fronted; Brazil-based → nearest AWS is São Paulo.
        curated_region: "sa-east-1",
        curated_confidence: 0.50,
        curated_note: "Mercado Bitcoin public endpoints are behind Cloudflare; the origin region is not network-detectable. Documented infra region (AWS São Paulo, sa-east-1) shown — verify by deploying probes inside candidate regions.",
    },
    VenueSpec {
        id: "ndax",
        display: "NDAX (spot)",
        // NDAX uses the AlphaPoint API (no simple GET ticker); this public
        // GetInstruments call resolves the OVHcloud-Canada origin for region+latency.
        rest_urls: &["https://api.ndax.io/AP/GetInstruments?OMSId=1"],
        ws_urls: &[],
        resolve_hosts: &[],
        ws_subscribe: "",
        // api.ndax.io is OVHcloud Canada (Beauharnois) — not AWS; geolocation
        // snaps it to the nearest AWS region (ca-central-1, Montreal).
        curated_region: "ca-central-1",
        curated_confidence: 0.50,
        curated_note: "NDAX is hosted on OVHcloud Canada (not AWS); nearest AWS region (ca-central-1, Montreal) shown for colocation reference.",
    },
    VenueSpec {
        id: "bitvavo",
        display: "Bitvavo (spot)",
        rest_urls: &["https://api.bitvavo.com/v2/ticker/price?market=BTC-EUR"],
        ws_urls: &["wss://ws.bitvavo.com/v2/"],
        resolve_hosts: &[],
        ws_subscribe: "{\"action\":\"subscribe\",\"channels\":[{\"name\":\"ticker\",\"markets\":[\"BTC-EUR\"]}]}",
        // Cloudflare-fronted; Amsterdam-based → documented EU region uncertain.
        curated_region: "eu-central-1",
        curated_confidence: 0.45,
        curated_note: "Bitvavo public endpoints are behind Cloudflare; the origin region is not network-detectable. Low-confidence documented guess (AWS Frankfurt, eu-central-1) — verify by deploying probes inside candidate regions.",
    },
    VenueSpec {
        id: "bithumb",
        display: "Bithumb (spot)",
        rest_urls: &["https://api.bithumb.com/public/ticker/BTC_KRW"],
        ws_urls: &["wss://pubwss.bithumb.com/pub/ws"],
        resolve_hosts: &[],
        ws_subscribe: "{\"type\":\"ticker\",\"symbols\":[\"BTC_KRW\"],\"tickTypes\":[\"24H\"]}",
        // api.bithumb.com resolves to real AWS ap-northeast-2 IPs (Seoul).
        curated_region: "ap-northeast-2",
        curated_confidence: 0.85,
        curated_note: "Bithumb runs in AWS Seoul (ap-northeast-2).",
    },
];

pub fn venue_by_id(id: &str) -> Option<&'static VenueSpec> {
    VENUES.iter().find(|v| v.id == id)
}
