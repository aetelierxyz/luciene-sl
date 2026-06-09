//! `probe` — measures public CEX REST/WSS latency, locates each exchange's AWS
//! region, and writes a colocation recommendation report (JSON).

mod aws;
mod geo;
mod latency;
mod recommend;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use coloc_shared::report::Report;
use coloc_shared::venues::VENUES;

#[derive(Parser, Debug)]
#[command(name = "probe", about = "CEX → AWS colocation probe")]
struct Args {
    /// Number of REST latency samples per endpoint.
    #[arg(long, default_value_t = 5)]
    rest_samples: u32,
    /// Number of WSS latency samples per endpoint.
    #[arg(long, default_value_t = 3)]
    ws_samples: u32,
    /// Output path for the JSON report.
    #[arg(long, default_value = "dashboard/report.json")]
    out: PathBuf,
    /// Restrict to a comma-separated list of venue ids (default: all).
    #[arg(long)]
    only: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let args = Args::parse();

    let client = reqwest::Client::builder()
        .user_agent("luciene-coloc-probe/0.1 (+https://iteralabs.xyz)")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .context("build reqwest client")?;

    tracing::info!("fetching AWS ip-ranges …");
    let ranges = aws::AwsRanges::fetch(&client)
        .await
        .context("fetch AWS ip-ranges")?;

    let only: Option<Vec<String>> = args
        .only
        .as_ref()
        .map(|s| s.split(',').map(|x| x.trim().to_string()).collect());

    let mut venues = Vec::new();
    for spec in VENUES {
        if let Some(filter) = &only {
            if !filter.iter().any(|f| f == spec.id) {
                continue;
            }
        }
        tracing::info!(venue = spec.id, "probing …");
        match recommend::recommend_venue(&client, &ranges, args.rest_samples, args.ws_samples, spec)
            .await
        {
            Ok(rec) => {
                tracing::info!(
                    venue = spec.id,
                    region = %rec.region,
                    method = ?rec.method,
                    confidence = rec.confidence,
                    rest_ms = rec.rest_median_ms,
                    ws_ms = rec.ws_median_ms,
                    "recommendation ready"
                );
                venues.push(rec);
            }
            Err(e) => tracing::error!(venue = spec.id, error = %e, "venue probe failed"),
        }
    }

    // Primary pick: highest composite score (region confidence + stability).
    let primary_pick = venues
        .iter()
        .max_by(|a, b| a.score.partial_cmp(&b.score).unwrap())
        .map(|v| v.exchange.clone());

    let now = chrono::Utc::now();
    let report = Report {
        schema_version: 1,
        generated_at: now.to_rfc3339(),
        generated_unix: now.timestamp(),
        probe_host_note: "Latency measured from the probe host (developer machine), \
            which mostly reflects proximity to CDN edges; region selection is driven \
            primarily by AWS ip-range matches and curated engine locations."
            .to_string(),
        venues,
        primary_pick,
        onchain: None,
    };

    if let Some(parent) = args.out.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&args.out, report.to_pretty_json())
        .with_context(|| format!("write {}", args.out.display()))?;

    print_summary(&report, &args.out);
    Ok(())
}

fn print_summary(report: &Report, out: &std::path::Path) {
    println!("\n=== Colocation recommendation ({}) ===", report.generated_at);
    for v in &report.venues {
        println!(
            "  {:<9} → AWS {:<15} {:>9.4},{:<9.4}  r={}km  conf={:.0}%  stab={:.0}%  score={:.0}%  [{:?}]",
            v.exchange,
            v.region,
            v.lat,
            v.lon,
            v.radius_m / 1000,
            v.confidence * 100.0,
            v.stability * 100.0,
            v.score * 100.0,
            v.method,
        );
        println!(
            "            rest={:.0}ms (±{:.0} p95={:.0}) ws={:.0}ms (±{:.0}) success={:.0}%",
            v.rest_median_ms, v.rest_jitter_ms, v.rest_p95_ms,
            v.ws_median_ms, v.ws_jitter_ms, v.success_rate * 100.0,
        );
        println!("            {} — {}", v.region_city, v.rationale);
    }
    if let Some(p) = &report.primary_pick {
        println!("  primary pick: {p}");
    }
    println!("  report written to {}", out.display());
}
