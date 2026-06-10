//! Read the colocation report back from Solana via JSON-RPC `getAccountInfo`
//! and decode the Anchor account (8-byte discriminator + borsh body).

use anyhow::{anyhow, Context, Result};
use base64::Engine;
use borsh::BorshDeserialize;
use serde::Serialize;

use coloc_shared::regions::region_by_code;
use coloc_shared::report::Method;
use coloc_shared::wire::{from_micro, unpad, MAX_VENUES};

#[derive(BorshDeserialize)]
struct VenueRecordWire {
    exchange: [u8; 12],
    region: [u8; 16],
    lat_micro: i64,
    lon_micro: i64,
    radius_m: u32,
    rest_latency_ms: u32,
    ws_latency_ms: u32,
    confidence_bps: u16,
    method_code: u8,
    sample_count: u16,
}

#[derive(BorshDeserialize)]
struct ColocationReportWire {
    authority: [u8; 32],
    last_update: i64,
    generated_unix: i64,
    schema_version: u32,
    num_venues: u8,
    primary_index: u8,
    venues: [VenueRecordWire; MAX_VENUES],
    #[allow(dead_code)]
    bump: u8,
}

/// JSON shape served to the dashboard front-end.
#[derive(Serialize)]
pub struct OnChainReport {
    pub source: String,
    pub cluster: String,
    pub program_id: String,
    pub account: String,
    pub slot: u64,
    pub authority: String,
    pub last_update: i64,
    pub generated_unix: i64,
    pub schema_version: u32,
    pub primary_index: u8,
    pub venues: Vec<OnChainVenue>,
}

#[derive(Serialize)]
pub struct OnChainVenue {
    pub exchange: String,
    pub region: String,
    pub region_city: String,
    pub lat: f64,
    pub lon: f64,
    pub radius_m: u32,
    pub confidence: f64,
    pub method: Method,
    pub rest_median_ms: u32,
    pub ws_median_ms: u32,
    pub sample_count: u16,
    pub rationale: String,
    /// True if this venue is stored in the on-chain account; false if it is
    /// measured-only (off-chain report) pending an on-chain layout bump.
    pub anchored: bool,
    // --- stability/score enrichment, merged from the local measured report ---
    /// True when the fields below were merged from the off-chain report.json.
    pub measured: bool,
    pub stability: f64,
    pub score: f64,
    pub rest_jitter_ms: f64,
    pub ws_jitter_ms: f64,
    pub rest_p95_ms: f64,
    pub success_rate: f64,
    pub region_evidence: u32,
    pub region_ip_total: u32,
}

/// Fetch and decode the on-chain report account.
pub async fn fetch(
    client: &reqwest::Client,
    rpc_url: &str,
    cluster: &str,
    account: &str,
) -> Result<OnChainReport> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getAccountInfo",
        "params": [account, {"encoding": "base64", "commitment": "confirmed"}],
    });
    let resp: serde_json::Value = client
        .post(rpc_url)
        .json(&body)
        .send()
        .await
        .context("getAccountInfo request")?
        .json()
        .await
        .context("parse rpc json")?;

    let value = resp
        .get("result")
        .and_then(|r| r.get("value"))
        .filter(|v| !v.is_null())
        .ok_or_else(|| anyhow!("account {account} not found on {cluster}"))?;
    let slot = resp["result"]["context"]["slot"].as_u64().unwrap_or_default();
    let data_b64 = value["data"][0]
        .as_str()
        .ok_or_else(|| anyhow!("no base64 data in account"))?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data_b64)
        .context("base64 decode account")?;
    if bytes.len() < 8 {
        return Err(anyhow!("account too small"));
    }

    // Skip the 8-byte Anchor discriminator, then borsh-decode.
    let wire = ColocationReportWire::try_from_slice(&bytes[8..])
        .context("borsh decode ColocationReport")?;

    let mut venues = Vec::new();
    for v in wire.venues.iter().take(wire.num_venues as usize) {
        let region = unpad(&v.region);
        let region_city = region_by_code(&region)
            .map(|r| r.city.to_string())
            .unwrap_or_else(|| "unknown".into());
        let method = Method::from_code(v.method_code);
        let confidence = v.confidence_bps as f64 / 10_000.0;
        venues.push(OnChainVenue {
            exchange: unpad(&v.exchange),
            region: region.clone(),
            region_city,
            lat: from_micro(v.lat_micro),
            lon: from_micro(v.lon_micro),
            radius_m: v.radius_m,
            confidence,
            method,
            rest_median_ms: v.rest_latency_ms,
            ws_median_ms: v.ws_latency_ms,
            sample_count: v.sample_count,
            rationale: rationale_for(method, &region, confidence),
            anchored: true,
            measured: false,
            stability: 0.0,
            score: 0.0,
            rest_jitter_ms: 0.0,
            ws_jitter_ms: 0.0,
            rest_p95_ms: 0.0,
            success_rate: 0.0,
            region_evidence: 0,
            region_ip_total: 0,
        });
    }

    Ok(OnChainReport {
        source: "onchain".into(),
        cluster: cluster.to_string(),
        program_id: coloc_shared::PROGRAM_ID.to_string(),
        account: account.to_string(),
        slot,
        authority: bs58_encode(&wire.authority),
        last_update: wire.last_update,
        generated_unix: wire.generated_unix,
        schema_version: wire.schema_version,
        primary_index: wire.primary_index,
        venues,
    })
}

/// Merge locally-measured stability/score fields from the off-chain report into
/// the on-chain report (matched by exchange id). The on-chain values remain the
/// authoritative source for region/lat/lon/confidence; this only adds the
/// stability telemetry that is too large to store on-chain.
pub fn enrich(report: &mut OnChainReport, local: &coloc_shared::report::Report) {
    for v in &mut report.venues {
        if let Some(l) = local.venues.iter().find(|x| x.exchange == v.exchange) {
            v.measured = true;
            v.stability = l.stability;
            v.score = l.score;
            v.rest_jitter_ms = l.rest_jitter_ms;
            v.ws_jitter_ms = l.ws_jitter_ms;
            v.rest_p95_ms = l.rest_p95_ms;
            v.success_rate = l.success_rate;
            v.region_evidence = l.region_evidence;
            v.region_ip_total = l.region_ip_total;
            if !l.rationale.is_empty() {
                v.rationale = l.rationale.clone();
            }
        }
    }
    // Append measured-only venues that the on-chain account can't hold yet.
    let anchored: std::collections::HashSet<&str> =
        report.venues.iter().map(|v| v.exchange.as_str()).collect();
    let extra: Vec<&coloc_shared::report::VenueRecommendation> = local
        .venues
        .iter()
        .filter(|l| !anchored.contains(l.exchange.as_str()))
        .collect();
    for l in extra {
        report.venues.push(OnChainVenue {
            exchange: l.exchange.clone(),
            region: l.region.clone(),
            region_city: l.region_city.clone(),
            lat: l.lat,
            lon: l.lon,
            radius_m: l.radius_m,
            confidence: l.confidence,
            method: l.method,
            rest_median_ms: l.rest_median_ms.round().max(0.0) as u32,
            ws_median_ms: l.ws_median_ms.round().max(0.0) as u32,
            sample_count: l.sample_count.min(u16::MAX as u32) as u16,
            rationale: l.rationale.clone(),
            anchored: false,
            measured: true,
            stability: l.stability,
            score: l.score,
            rest_jitter_ms: l.rest_jitter_ms,
            ws_jitter_ms: l.ws_jitter_ms,
            rest_p95_ms: l.rest_p95_ms,
            success_rate: l.success_rate,
            region_evidence: l.region_evidence,
            region_ip_total: l.region_ip_total,
        });
    }
}

fn rationale_for(method: Method, region: &str, confidence: f64) -> String {
    let how = match method {
        Method::AwsIpRange => "endpoint IP matched an AWS compute-region CIDR (ip-ranges.json)",
        Method::IpGeoNearest => "endpoint IP geolocated to the nearest AWS region",
        Method::Curated => "endpoint behind a CDN; documented matching-engine region used",
    };
    format!(
        "Recommended AWS {region} — {how}. Confidence {:.0}%.",
        confidence * 100.0
    )
}

/// Minimal base58 (Bitcoin alphabet) encoder for displaying the authority key.
fn bs58_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    let zeros = input.iter().take_while(|&&b| b == 0).count();
    let mut digits: Vec<u8> = Vec::new();
    for &byte in input {
        let mut carry = byte as u32;
        for d in digits.iter_mut() {
            carry += (*d as u32) << 8;
            *d = (carry % 58) as u8;
            carry /= 58;
        }
        while carry > 0 {
            digits.push((carry % 58) as u8);
            carry /= 58;
        }
    }
    let mut out = String::new();
    for _ in 0..zeros {
        out.push('1');
    }
    for &d in digits.iter().rev() {
        out.push(ALPHABET[d as usize] as char);
    }
    if out.is_empty() {
        out.push('1');
    }
    out
}
