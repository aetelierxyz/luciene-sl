//! Latency sampling for REST (reqwest) and WSS (tokio-tungstenite) endpoints,
//! plus DNS resolution of endpoint hosts.

use std::net::IpAddr;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use tokio::net::lookup_host;
use tokio::time::timeout;
use url::Url;

/// Outcome of probing a single endpoint URL.
pub struct EndpointMeasure {
    pub host: String,
    pub resolved_ips: Vec<IpAddr>,
    pub median_ms: f64,
    pub min_ms: f64,
    pub p95_ms: f64,
    /// Standard deviation of the samples (jitter, ms).
    pub jitter_ms: f64,
    /// successful_samples / attempts.
    pub success_rate: f64,
    /// Successful samples.
    pub samples: u32,
    pub attempts: u32,
}

/// Summary statistics over a set of latency samples.
struct Stats {
    median: f64,
    min: f64,
    p95: f64,
    stddev: f64,
}

fn stats(times: &[f64]) -> Stats {
    if times.is_empty() {
        return Stats { median: 0.0, min: 0.0, p95: 0.0, stddev: 0.0 };
    }
    let mut v = times.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    let median = if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) / 2.0 };
    let min = v[0];
    let p95 = v[((n as f64 * 0.95).ceil() as usize).saturating_sub(1).min(n - 1)];
    let mean = v.iter().sum::<f64>() / n as f64;
    let var = v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64;
    Stats { median, min, p95, stddev: var.sqrt() }
}

fn host_and_port(url: &str) -> Result<(String, u16)> {
    let u = Url::parse(url)?;
    let host = u
        .host_str()
        .ok_or_else(|| anyhow!("no host in {url}"))?
        .to_string();
    let port = u.port_or_known_default().unwrap_or(443);
    Ok((host, port))
}

/// Resolve a host to a deduplicated list of IPs.
pub async fn resolve(host: &str, port: u16) -> Result<Vec<IpAddr>> {
    let mut ips: Vec<IpAddr> = lookup_host((host, port))
        .await?
        .map(|sa| sa.ip())
        .collect();
    ips.sort();
    ips.dedup();
    Ok(ips)
}

#[cfg(test)]
fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

/// Measure REST latency: full request → body read, `samples` times.
pub async fn measure_rest(
    client: &reqwest::Client,
    url: &str,
    samples: u32,
) -> Result<EndpointMeasure> {
    let (host, port) = host_and_port(url)?;
    let resolved_ips = resolve(&host, port).await.unwrap_or_default();

    // One warm-up to amortise TLS session setup, then timed samples.
    let _ = client.get(url).send().await.and_then(|r| r.error_for_status());

    let mut times = Vec::new();
    let mut attempts = 0u32;
    for _ in 0..samples {
        attempts += 1;
        let start = Instant::now();
        match timeout(Duration::from_secs(8), client.get(url).send()).await {
            Ok(Ok(resp)) => {
                if resp.error_for_status_ref().is_ok() && resp.bytes().await.is_ok() {
                    times.push(start.elapsed().as_secs_f64() * 1000.0);
                }
            }
            _ => tracing::warn!(%url, "rest sample failed"),
        }
        tokio::time::sleep(Duration::from_millis(120)).await;
    }
    if times.is_empty() {
        return Err(anyhow!("all rest samples failed for {url}"));
    }
    let s = stats(&times);
    Ok(EndpointMeasure {
        host,
        resolved_ips,
        median_ms: s.median,
        min_ms: s.min,
        p95_ms: s.p95,
        jitter_ms: s.stddev,
        success_rate: times.len() as f64 / attempts as f64,
        samples: times.len() as u32,
        attempts,
    })
}

/// Measure WSS latency: time from connect-start to first inbound frame.
/// Runs `samples` independent connections and takes the median.
pub async fn measure_ws(url: &str, subscribe: &str, samples: u32) -> Result<EndpointMeasure> {
    let (host, port) = host_and_port(url)?;
    let resolved_ips = resolve(&host, port).await.unwrap_or_default();

    let mut times = Vec::new();
    let mut attempts = 0u32;
    for _ in 0..samples {
        attempts += 1;
        match timeout(Duration::from_secs(10), ws_first_frame(url, subscribe)).await {
            Ok(Ok(ms)) => times.push(ms),
            Ok(Err(e)) => tracing::warn!(%url, error = %e, "ws sample failed"),
            Err(_) => tracing::warn!(%url, "ws sample timed out"),
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    if times.is_empty() {
        return Err(anyhow!("all ws samples failed for {url}"));
    }
    let s = stats(&times);
    Ok(EndpointMeasure {
        host,
        resolved_ips,
        median_ms: s.median,
        min_ms: s.min,
        p95_ms: s.p95,
        jitter_ms: s.stddev,
        success_rate: times.len() as f64 / attempts as f64,
        samples: times.len() as u32,
        attempts,
    })
}

/// Resolve a "host:port" string for region-evidence only (no latency probe).
pub async fn resolve_host_port(host_port: &str) -> Result<(String, Vec<IpAddr>)> {
    let (host, port) = match host_port.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse::<u16>().unwrap_or(443)),
        None => (host_port.to_string(), 443),
    };
    let ips = resolve(&host, port).await.unwrap_or_default();
    Ok((host, ips))
}

async fn ws_first_frame(url: &str, subscribe: &str) -> Result<f64> {
    let start = Instant::now();
    let (mut ws, _resp) = tokio_tungstenite::connect_async(url).await?;
    if !subscribe.is_empty() {
        ws.send(tokio_tungstenite::tungstenite::Message::Text(
            subscribe.to_string(),
        ))
        .await?;
    }
    // Pull frames until we get a data frame (skip pings/pongs).
    while let Some(msg) = ws.next().await {
        let msg = msg?;
        use tokio_tungstenite::tungstenite::Message::*;
        match msg {
            Text(_) | Binary(_) => break,
            Ping(_) | Pong(_) | Frame(_) => continue,
            Close(_) => return Err(anyhow!("ws closed before data")),
        }
    }
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
    let _ = ws.close(None).await;
    Ok(elapsed)
}

#[cfg(test)]
mod tests {
    use super::median;

    #[test]
    fn median_odd_even() {
        assert_eq!(median(vec![3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(vec![1.0, 2.0, 3.0, 4.0]), 2.5);
    }
}
