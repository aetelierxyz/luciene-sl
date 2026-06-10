//! `publisher` — reads `report.json` and writes the colocation recommendation to
//! the on-chain oracle program on Solana devnet via `set_colocation`.

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use borsh::BorshSerialize;
use clap::Parser;
use coloc_shared::report::Report;
use coloc_shared::wire::{ix_discriminator, pad, to_micro};
use coloc_shared::{COLOCATION_SEED, PROGRAM_ID};
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{read_keypair_file, Keypair, Signer};
use solana_sdk::transaction::Transaction;

#[derive(Parser, Debug)]
#[command(name = "publisher", about = "Publish colocation report to Solana devnet")]
struct Args {
    /// Path to the JSON report produced by `probe`.
    #[arg(long, default_value = "dashboard/report.json")]
    report: PathBuf,
    /// RPC URL (defaults to the Solana CLI config, else devnet).
    #[arg(long)]
    rpc: Option<String>,
    /// Signer keypair (defaults to the Solana CLI config keypair).
    #[arg(long)]
    keypair: Option<PathBuf>,
}

/// `init_report` args (header + allocation). Field order must match the program.
#[derive(BorshSerialize)]
struct InitReportArgs {
    schema_version: u32,
    generated_unix: i64,
    num_venues: u8,
    primary_index: u8,
}

/// `set_venue` args (one venue into a slot).
#[derive(BorshSerialize)]
struct SetVenueArgs {
    index: u8,
    venue: VenueRecordWire,
}

/// Mirrors `coloc_oracle::VenueRecord` byte-for-byte.
#[derive(BorshSerialize)]
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

fn solana_cli_config() -> (Option<String>, Option<PathBuf>) {
    let home = std::env::var("HOME").unwrap_or_default();
    let path = format!("{home}/.config/solana/cli/config.yml");
    let Ok(text) = std::fs::read_to_string(path) else {
        return (None, None);
    };
    let mut rpc = None;
    let mut keypair = None;
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("json_rpc_url:") {
            rpc = Some(v.trim().trim_matches('"').to_string());
        } else if let Some(v) = line.strip_prefix("keypair_path:") {
            keypair = Some(PathBuf::from(v.trim().trim_matches('"')));
        }
    }
    (rpc, keypair)
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let args = Args::parse();
    let (cfg_rpc, cfg_keypair) = solana_cli_config();

    let rpc_url = args
        .rpc
        .or(cfg_rpc)
        .unwrap_or_else(|| "https://api.devnet.solana.com".to_string());
    let keypair_path = args
        .keypair
        .or(cfg_keypair)
        .ok_or_else(|| anyhow!("no keypair: pass --keypair or set Solana CLI config"))?;

    let payer: Keypair = read_keypair_file(&keypair_path)
        .map_err(|e| anyhow!("read keypair {}: {e}", keypair_path.display()))?;
    let program_id: Pubkey = PROGRAM_ID.parse().context("parse program id")?;

    let mut report: Report = serde_json::from_str(
        &std::fs::read_to_string(&args.report)
            .with_context(|| format!("read {}", args.report.display()))?,
    )
    .context("parse report.json")?;

    // Build the wire venues and resolve the primary index.
    let mut venues = Vec::new();
    // The deployed program stores a fixed [VenueRecord; MAX_VENUES]; anchor the
    // first MAX_VENUES (configured order) and leave any extras measured-only
    // until the on-chain layout is bumped + redeployed.
    let max_onchain = coloc_shared::wire::MAX_VENUES;
    let anchored: Vec<&coloc_shared::report::VenueRecommendation> =
        report.venues.iter().take(max_onchain).collect();
    if report.venues.len() > max_onchain {
        let dropped: Vec<&str> = report.venues[max_onchain..]
            .iter()
            .map(|v| v.exchange.as_str())
            .collect();
        tracing::warn!(
            ?dropped,
            max_onchain,
            "more venues than on-chain capacity; these are measured-only (bump MAX_VENUES + redeploy to anchor them)"
        );
    }
    for v in &anchored {
        venues.push(VenueRecordWire {
            exchange: pad::<12>(&v.exchange),
            region: pad::<16>(&v.region),
            lat_micro: to_micro(v.lat),
            lon_micro: to_micro(v.lon),
            radius_m: v.radius_m,
            rest_latency_ms: v.rest_median_ms.round().max(0.0) as u32,
            ws_latency_ms: v.ws_median_ms.round().max(0.0) as u32,
            confidence_bps: (v.confidence * 10_000.0).round().clamp(0.0, 10_000.0) as u16,
            method_code: v.method.code(),
            sample_count: v.sample_count.min(u16::MAX as u32) as u16,
        });
    }
    if venues.is_empty() {
        return Err(anyhow!("report has no venues"));
    }
    // Primary index within the anchored subset.
    let primary_index = report
        .primary_pick
        .as_ref()
        .and_then(|p| anchored.iter().position(|v| &v.exchange == p))
        .unwrap_or(0) as u8;

    // PDA: seeds = [COLOCATION_SEED, authority].
    let (report_pda, _bump) =
        Pubkey::find_program_address(&[COLOCATION_SEED, payer.pubkey().as_ref()], &program_id);

    let client = RpcClient::new_with_commitment(rpc_url.clone(), CommitmentConfig::confirmed());
    let num_venues = venues.len() as u8;
    tracing::info!(%rpc_url, payer = %payer.pubkey(), pda = %report_pda, num_venues, "publishing colocation report (chunked)");

    // Helper: build, sign with a fresh blockhash, send + confirm.
    let send = |ix: Instruction| -> Result<solana_sdk::signature::Signature> {
        let blockhash = client.get_latest_blockhash().context("get blockhash")?;
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            blockhash,
        );
        client.send_and_confirm_transaction(&tx).map_err(Into::into)
    };

    // 1) init_report — header + allocation, zeroes all slots.
    let mut data = ix_discriminator("init_report").to_vec();
    InitReportArgs {
        schema_version: report.schema_version,
        generated_unix: report.generated_unix,
        num_venues,
        primary_index,
    }
    .serialize(&mut data)
    .context("borsh init_report args")?;
    let init_sig = send(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(report_pda, false),
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
        data,
    })
    .context("send init_report")?;
    tracing::info!(sig = %init_sig, "init_report confirmed");

    // 2) set_venue — one transaction per venue (61-byte records exceed the
    //    1232-byte tx limit when batched, so we chunk one at a time).
    let setv_disc = ix_discriminator("set_venue");
    for (i, v) in venues.into_iter().enumerate() {
        let mut data = setv_disc.to_vec();
        SetVenueArgs { index: i as u8, venue: v }
            .serialize(&mut data)
            .context("borsh set_venue args")?;
        let sig = send(Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(report_pda, false),
                AccountMeta::new_readonly(payer.pubkey(), true),
            ],
            data,
        })
        .with_context(|| format!("send set_venue[{i}]"))?;
        tracing::info!(index = i, %sig, "set_venue confirmed");
    }

    let sig = init_sig;
    let slot = client.get_slot().unwrap_or_default();

    let cluster = if rpc_url.contains("devnet") {
        "devnet"
    } else if rpc_url.contains("mainnet") {
        "mainnet-beta"
    } else if rpc_url.contains("127.0.0.1") || rpc_url.contains("localhost") {
        "localnet"
    } else {
        "custom"
    };

    report.onchain = Some(coloc_shared::report::OnChainRef {
        cluster: cluster.to_string(),
        program_id: PROGRAM_ID.to_string(),
        account: report_pda.to_string(),
        signature: sig.to_string(),
        slot,
    });
    std::fs::write(&args.report, report.to_pretty_json())
        .with_context(|| format!("write {}", args.report.display()))?;

    println!("\n=== Published on-chain ({cluster}) ===");
    println!("  program : {PROGRAM_ID}");
    println!("  account : {report_pda}");
    println!("  tx      : {sig}");
    println!("  explorer: https://explorer.solana.com/tx/{sig}?cluster={cluster}");
    println!("  report.json updated with on-chain reference");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use coloc_shared::report::Method;

    /// The wire record must serialise to exactly the on-chain `VenueRecord::LEN`
    /// (EXCHANGE_LEN 12 + REGION_LEN 16 + i64*2 + u32*3 + u16 + u8 + u16 = 61).
    #[test]
    fn venue_record_wire_is_61_bytes() {
        let rec = VenueRecordWire {
            exchange: pad::<12>("binance"),
            region: pad::<16>("ap-northeast-1"),
            lat_micro: to_micro(35.6895),
            lon_micro: to_micro(139.6917),
            radius_m: 10_000,
            rest_latency_ms: 184,
            ws_latency_ms: 1062,
            confidence_bps: 9200,
            method_code: Method::AwsIpRange.code(),
            sample_count: 13,
        };
        let bytes = borsh::to_vec(&rec).unwrap();
        assert_eq!(bytes.len(), 61, "wire layout drifted from on-chain VenueRecord");
    }

    /// Anchor discriminators are deterministic; lock the value so an accidental
    /// rename of the instruction is caught.
    #[test]
    fn discriminator_is_stable() {
        let d = ix_discriminator("set_colocation");
        assert_eq!(d.len(), 8);
        // sha256("global:set_colocation")[..8]
        assert_eq!(d, [0x87, 0xec, 0xf8, 0x84, 0x76, 0x1e, 0x2f, 0xa8]);
    }
}
