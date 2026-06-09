//! `dashboard` — a tiny local web server that reads the colocation report back
//! from Solana devnet and renders it on a Leaflet map (point + 10 km radius).

mod onchain;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const INDEX_HTML: &str = include_str!("../static/index.html");

#[derive(Parser, Debug)]
#[command(name = "dashboard", about = "Local colocation dashboard (reads Solana devnet)")]
struct Args {
    /// Address to bind.
    #[arg(long, default_value = "127.0.0.1:8080")]
    bind: String,
    /// report.json (used to discover the on-chain account + cluster).
    #[arg(long, default_value = "dashboard/report.json")]
    report: PathBuf,
    /// Override the on-chain account (PDA) to read.
    #[arg(long)]
    account: Option<String>,
    /// Override the RPC URL.
    #[arg(long)]
    rpc: Option<String>,
    /// Override the cluster label.
    #[arg(long)]
    cluster: Option<String>,
}

#[derive(Clone)]
struct AppState {
    http: reqwest::Client,
    rpc_url: String,
    cluster: String,
    account: String,
    report_path: PathBuf,
}

fn rpc_for_cluster(cluster: &str) -> String {
    match cluster {
        "mainnet-beta" => "https://api.mainnet-beta.solana.com",
        "localnet" => "http://127.0.0.1:8899",
        _ => "https://api.devnet.solana.com",
    }
    .to_string()
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

    // Discover account + cluster from report.json unless overridden.
    let (mut account, mut cluster) = (args.account.clone(), args.cluster.clone());
    if account.is_none() || cluster.is_none() {
        if let Ok(text) = std::fs::read_to_string(&args.report) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(oc) = val.get("onchain").filter(|v| !v.is_null()) {
                    account = account.or_else(|| {
                        oc.get("account").and_then(|v| v.as_str()).map(String::from)
                    });
                    cluster = cluster.or_else(|| {
                        oc.get("cluster").and_then(|v| v.as_str()).map(String::from)
                    });
                }
            }
        }
    }
    let cluster = cluster.unwrap_or_else(|| "devnet".to_string());
    let account = account.ok_or_else(|| {
        anyhow!(
            "no on-chain account known. Run the publisher first, or pass --account.\n\
             (looked in {})",
            args.report.display()
        )
    })?;
    let rpc_url = args.rpc.unwrap_or_else(|| rpc_for_cluster(&cluster));

    let state = Arc::new(AppState {
        http: reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?,
        rpc_url,
        cluster,
        account,
        report_path: args.report.clone(),
    });

    let listener = TcpListener::bind(&args.bind)
        .await
        .with_context(|| format!("bind {}", args.bind))?;
    println!("dashboard listening on http://{}", args.bind);
    println!("  reading account {} on {}", state.account, state.cluster);

    loop {
        let (stream, _) = listener.accept().await?;
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = handle(stream, state).await {
                tracing::debug!(error = %e, "connection error");
            }
        });
    }
}

async fn handle(mut stream: tokio::net::TcpStream, state: Arc<AppState>) -> Result<()> {
    // Read the request head (enough to get the request line).
    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf).await?;
    if n == 0 {
        return Ok(());
    }
    let head = String::from_utf8_lossy(&buf[..n]);
    let path = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .unwrap_or("/");

    let (status, ctype, body) = match path {
        "/" | "/index.html" => ("200 OK", "text/html; charset=utf-8", INDEX_HTML.as_bytes().to_vec()),
        "/healthz" => ("200 OK", "text/plain", b"ok".to_vec()),
        p if p.starts_with("/api/report") => match api_report(&state).await {
            Ok(json) => ("200 OK", "application/json", json.into_bytes()),
            Err(e) => {
                let body = serde_json::json!({"error": e.to_string()}).to_string();
                ("502 Bad Gateway", "application/json", body.into_bytes())
            }
        },
        _ => ("404 Not Found", "text/plain", b"not found".to_vec()),
    };

    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes()).await?;
    stream.write_all(&body).await?;
    stream.flush().await?;
    Ok(())
}

async fn api_report(state: &AppState) -> Result<String> {
    let mut report =
        onchain::fetch(&state.http, &state.rpc_url, &state.cluster, &state.account).await?;
    // Enrich on-chain core values with locally-measured stability telemetry.
    if let Ok(text) = std::fs::read_to_string(&state.report_path) {
        if let Ok(local) = serde_json::from_str::<coloc_shared::report::Report>(&text) {
            onchain::enrich(&mut report, &local);
        }
    }
    Ok(serde_json::to_string(&report)?)
}
