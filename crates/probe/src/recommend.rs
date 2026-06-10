//! Combine measured latency/stability, AWS IP-range evidence and geolocation
//! into a per-venue colocation recommendation.

use std::collections::HashMap;
use std::net::IpAddr;

use anyhow::Result;
use coloc_shared::regions::{nearest_region, region_by_code};
use coloc_shared::report::{EndpointProbe, Method, VenueRecommendation};
use coloc_shared::venues::VenueSpec;

use crate::aws::AwsRanges;
use crate::geo;
use crate::latency::{self, EndpointMeasure};

const RADIUS_M: u32 = 10_000;

/// Probe every endpoint of a venue and derive its recommendation.
pub async fn recommend_venue(
    client: &reqwest::Client,
    ranges: &AwsRanges,
    rest_samples: u32,
    ws_samples: u32,
    spec: &VenueSpec,
) -> Result<VenueRecommendation> {
    let mut endpoints: Vec<EndpointProbe> = Vec::new();
    let mut rest: Vec<EndpointMeasure> = Vec::new();
    let mut ws: Vec<EndpointMeasure> = Vec::new();
    let mut all_ips: Vec<IpAddr> = Vec::new();

    for url in spec.rest_urls {
        match latency::measure_rest(client, url, rest_samples).await {
            Ok(m) => {
                all_ips.extend(m.resolved_ips.iter().copied());
                endpoints.push(to_probe("rest", url, &m, ranges));
                rest.push(m);
            }
            Err(e) => tracing::warn!(%url, error = %e, "rest probe failed"),
        }
    }
    for url in spec.ws_urls {
        match latency::measure_ws(url, spec.ws_subscribe, ws_samples).await {
            Ok(m) => {
                all_ips.extend(m.resolved_ips.iter().copied());
                endpoints.push(to_probe("ws", url, &m, ranges));
                ws.push(m);
            }
            Err(e) => tracing::warn!(%url, error = %e, "ws probe failed"),
        }
    }
    // Region-evidence-only hosts (resolved, not latency-probed).
    for hp in spec.resolve_hosts {
        if let Ok((host, ips)) = latency::resolve_host_port(hp).await {
            all_ips.extend(ips.iter().copied());
            let region = ips.first().and_then(|ip| ranges.region_for(*ip));
            endpoints.push(EndpointProbe {
                kind: "resolve".into(),
                url: (*hp).to_string(),
                host,
                resolved_ips: ips.iter().map(|i| i.to_string()).collect(),
                median_ms: 0.0,
                min_ms: 0.0,
                p95_ms: 0.0,
                jitter_ms: 0.0,
                success_rate: 1.0,
                samples: 0,
                attempts: 0,
                aws_region_from_ip: region.filter(|r| region_by_code(r).is_some()),
                geo_city: None,
                geo_lat: None,
                geo_lon: None,
            });
        }
    }

    // --- latency + stability aggregation ----------------------------------
    let rest_median_ms = agg(&rest, |m| m.median_ms);
    let ws_median_ms = agg(&ws, |m| m.median_ms);
    let rest_jitter_ms = agg(&rest, |m| m.jitter_ms);
    let ws_jitter_ms = agg(&ws, |m| m.jitter_ms);
    let rest_p95_ms = agg(&rest, |m| m.p95_ms);
    let success_rate = {
        let all: Vec<&EndpointMeasure> = rest.iter().chain(ws.iter()).collect();
        if all.is_empty() {
            0.0
        } else {
            all.iter().map(|m| m.success_rate).sum::<f64>() / all.len() as f64
        }
    };
    let sample_count: u32 = rest.iter().chain(ws.iter()).map(|m| m.samples).sum();

    // Stability: success rate penalised by the *relative* jitter (coefficient of
    // variation = stddev/median), so it is robust to a venue's absolute latency
    // and to one-off outliers. stability = success_rate / (1 + cv).
    let (jit, med) = if rest_median_ms > 0.0 {
        (rest_jitter_ms, rest_median_ms)
    } else {
        (ws_jitter_ms, ws_median_ms)
    };
    let cv = if med > 0.0 { jit / med } else { 0.0 };
    let stability = (success_rate / (1.0 + cv)).clamp(0.0, 1.0);

    // --- region inference (evidence-based) --------------------------------
    let (region, method, confidence, evidence, ip_total, rationale) =
        infer_region(client, ranges, spec, &all_ips, &endpoints).await;

    let reg = region_by_code(&region)
        .copied()
        .unwrap_or_else(|| *region_by_code(spec.curated_region).unwrap());

    // Composite recommendation score: region confidence dominates, then
    // stability. (Probe-host latency is geographic, not colocation latency, so
    // it is reported but not used to rank venues.)
    let score = (0.6 * confidence + 0.4 * stability).clamp(0.0, 1.0);

    Ok(VenueRecommendation {
        exchange: spec.id.to_string(),
        region: reg.code.to_string(),
        region_city: reg.city.to_string(),
        lat: reg.lat,
        lon: reg.lon,
        radius_m: RADIUS_M,
        method,
        confidence,
        rest_median_ms,
        ws_median_ms,
        sample_count,
        rest_jitter_ms,
        ws_jitter_ms,
        rest_p95_ms,
        success_rate,
        stability,
        score,
        region_evidence: evidence,
        region_ip_total: ip_total,
        rationale,
        endpoints,
    })
}

fn to_probe(kind: &str, url: &str, m: &EndpointMeasure, ranges: &AwsRanges) -> EndpointProbe {
    let first_ip = m.resolved_ips.first().copied();
    let aws_region_from_ip = first_ip
        .and_then(|ip| ranges.region_for(ip))
        .filter(|r| region_by_code(r).is_some());
    EndpointProbe {
        kind: kind.to_string(),
        url: url.to_string(),
        host: m.host.clone(),
        resolved_ips: m.resolved_ips.iter().map(|i| i.to_string()).collect(),
        median_ms: m.median_ms,
        min_ms: m.min_ms,
        p95_ms: m.p95_ms,
        jitter_ms: m.jitter_ms,
        success_rate: m.success_rate,
        samples: m.samples,
        attempts: m.attempts,
        aws_region_from_ip,
        geo_city: None,
        geo_lat: None,
        geo_lon: None,
    }
}

#[allow(clippy::type_complexity)]
async fn infer_region(
    client: &reqwest::Client,
    ranges: &AwsRanges,
    spec: &VenueSpec,
    all_ips: &[IpAddr],
    endpoints: &[EndpointProbe],
) -> (String, Method, f64, u32, u32, String) {
    // Tally AWS *compute* regions across every resolved IP.
    let mut tally: HashMap<String, u32> = HashMap::new();
    let mut cdn_ips = 0u32;
    for ip in all_ips {
        match ranges.region_for(*ip) {
            // AWS IP in a real compute region → engine-region evidence.
            Some(region) if region_by_code(&region).is_some() => {
                *tally.entry(region).or_default() += 1;
            }
            // AWS IP but a non-compute service (e.g. CloudFront "GLOBAL") → CDN edge.
            Some(_) => cdn_ips += 1,
            // Non-AWS but known CDN (Cloudflare / Akamai) → CDN edge.
            None if crate::aws::is_cdn_edge(*ip) => cdn_ips += 1,
            None => {}
        }
    }
    let total = all_ips.len() as u32;

    // 1) AWS compute-region evidence (authoritative, fully empirical).
    if let Some((region, count)) = tally.iter().max_by_key(|(_, c)| **c) {
        let frac = *count as f64 / total.max(1) as f64;
        let confidence = (0.75 + 0.20 * frac).min(0.97);
        let rationale = format!(
            "{count}/{total} resolved IPs (across REST/WSS/direct endpoints) are in AWS {region} — colocating in-region minimises latency. Fully empirical (ip-ranges.json)."
        );
        return (region.clone(), Method::AwsIpRange, confidence, *count, total, rationale);
    }

    // 2) Endpoint is CDN-fronted (e.g. Cloudflare) → origin not network-visible.
    //    Geolocating an anycast edge is meaningless, so use documented region.
    if cdn_ips > 0 {
        return (
            spec.curated_region.to_string(),
            Method::Curated,
            spec.curated_confidence,
            0,
            total,
            spec.curated_note.to_string(),
        );
    }

    // 3) Non-AWS, non-CDN host → geolocate and snap to nearest AWS region.
    if let Some(ip) = all_ips.first() {
        if let Ok(Some(g)) = geo::locate(client, *ip).await {
            let (reg, dist) = nearest_region(g.lat, g.lon);
            let conf = (0.65 - (dist / 4000.0)).clamp(0.30, 0.65);
            return (
                reg.code.to_string(),
                Method::IpGeoNearest,
                conf,
                0,
                total,
                format!(
                    "Endpoint {ip} geolocated to {}, {}; nearest AWS region {} (~{:.0} km).",
                    g.city, g.country, reg.code, dist
                ),
            );
        }
    }
    let _ = endpoints;

    // 4) Last resort: documented region.
    (
        spec.curated_region.to_string(),
        Method::Curated,
        spec.curated_confidence,
        0,
        total,
        spec.curated_note.to_string(),
    )
}

/// Median of a per-measure field across endpoints (0 if none).
fn agg(ms: &[EndpointMeasure], f: impl Fn(&EndpointMeasure) -> f64) -> f64 {
    let mut v: Vec<f64> = ms.iter().map(&f).filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) / 2.0 }
}
