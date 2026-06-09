//! Serde report types shared across probe / publisher / dashboard.
//!
//! These are the *off-chain* representation (human-friendly JSON). The on-chain
//! layout is a compact byte-packed mirror — see `wire.rs` for the exact field
//! sizes that the Anchor program and the publisher must agree on.

use serde::{Deserialize, Serialize};

/// How a venue's region was determined, in descending order of confidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Method {
    /// Endpoint IP falls inside an AWS published CIDR block (authoritative).
    AwsIpRange,
    /// IP geolocated to a city; nearest AWS region picked by haversine.
    IpGeoNearest,
    /// Endpoint hidden behind a CDN; documented colocation knowledge used.
    Curated,
}

impl Method {
    pub fn code(self) -> u8 {
        match self {
            Method::AwsIpRange => 0,
            Method::IpGeoNearest => 1,
            Method::Curated => 2,
        }
    }
    pub fn from_code(c: u8) -> Method {
        match c {
            0 => Method::AwsIpRange,
            1 => Method::IpGeoNearest,
            _ => Method::Curated,
        }
    }
}

/// A single resolved endpoint with its measured latency and stability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointProbe {
    pub kind: String, // "rest" | "ws" | "resolve"
    pub url: String,
    pub host: String,
    pub resolved_ips: Vec<String>,
    /// Median round-trip latency in milliseconds over all successful samples.
    pub median_ms: f64,
    pub min_ms: f64,
    /// 95th percentile latency (tail) in ms.
    #[serde(default)]
    pub p95_ms: f64,
    /// Jitter = standard deviation of the samples (ms). Lower is more stable.
    #[serde(default)]
    pub jitter_ms: f64,
    /// Fraction of attempts that succeeded (0..=1). Higher is more stable.
    #[serde(default = "one")]
    pub success_rate: f64,
    /// Successful samples used for the stats.
    pub samples: u32,
    /// Total attempts made.
    #[serde(default)]
    pub attempts: u32,
    /// AWS region inferred from the *first* resolved IP, if any.
    pub aws_region_from_ip: Option<String>,
    /// City/country reported by ip geolocation, if any.
    pub geo_city: Option<String>,
    pub geo_lat: Option<f64>,
    pub geo_lon: Option<f64>,
}

fn one() -> f64 {
    1.0
}

/// The colocation recommendation for one exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VenueRecommendation {
    /// Short exchange id: "binance" | "coinbase" | "kraken".
    pub exchange: String,
    /// Recommended AWS region code, e.g. "ap-northeast-1".
    pub region: String,
    pub region_city: String,
    /// Recommended colocation centre.
    pub lat: f64,
    pub lon: f64,
    /// Radius of the recommended zone, in metres (10 km = 10000).
    pub radius_m: u32,
    pub method: Method,
    /// Confidence 0.0..=1.0.
    pub confidence: f64,
    /// Median REST latency observed from the probe host (ms).
    pub rest_median_ms: f64,
    /// Median WSS latency observed from the probe host (ms).
    pub ws_median_ms: f64,
    pub sample_count: u32,
    /// REST jitter (stddev, ms) — stability indicator.
    #[serde(default)]
    pub rest_jitter_ms: f64,
    /// WSS jitter (stddev, ms).
    #[serde(default)]
    pub ws_jitter_ms: f64,
    /// REST 95th-percentile latency (ms).
    #[serde(default)]
    pub rest_p95_ms: f64,
    /// Fraction of all probe attempts that succeeded (0..=1).
    #[serde(default = "one")]
    pub success_rate: f64,
    /// Composite stability score 0..=1 (success rate × low-jitter).
    #[serde(default)]
    pub stability: f64,
    /// Composite recommendation score 0..=1 (region confidence + stability).
    #[serde(default)]
    pub score: f64,
    /// AWS compute-region IPs found / total IPs resolved (region evidence).
    #[serde(default)]
    pub region_evidence: u32,
    #[serde(default)]
    pub region_ip_total: u32,
    /// Human-readable rationale.
    pub rationale: String,
    pub endpoints: Vec<EndpointProbe>,
}

/// Top-level report produced by the probe and rendered by the dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub schema_version: u32,
    /// RFC3339 timestamp of when the probe ran.
    pub generated_at: String,
    /// Unix seconds (mirrors `generated_at`, convenient for on-chain).
    pub generated_unix: i64,
    pub probe_host_note: String,
    pub venues: Vec<VenueRecommendation>,
    /// The single best overall pick (lowest-latency high-confidence venue).
    pub primary_pick: Option<String>,
    /// On-chain coordinates once published (filled by the publisher).
    #[serde(default)]
    pub onchain: Option<OnChainRef>,
}

/// Where the report was anchored on Solana.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnChainRef {
    pub cluster: String,
    pub program_id: String,
    pub account: String,
    pub signature: String,
    pub slot: u64,
}

impl Report {
    pub fn to_pretty_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("Report serialises")
    }
}
