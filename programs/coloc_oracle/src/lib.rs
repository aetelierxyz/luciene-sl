#![allow(unexpected_cfgs)]
//! On-chain colocation oracle.
//!
//! Solana programs cannot make network calls, so an off-chain probe measures the
//! public CEX REST/WSS endpoints, decides the best AWS region per exchange, and
//! submits the compact recommendation here via `set_colocation`. Anyone can then
//! read the `ColocationReport` PDA to learn the recommended latitude/longitude
//! and radius for colocating next to each exchange.

use anchor_lang::prelude::*;

declare_id!("GH9e1ZjDTRo4WKgJYxE1TZHSFFqgXxsDVTB4manBvvh1");

/// Max venues stored in one report (Binance, Coinbase, Kraken).
pub const MAX_VENUES: usize = 3;
/// Fixed width of the exchange id field.
pub const EXCHANGE_LEN: usize = 12;
/// Fixed width of the AWS region code field.
pub const REGION_LEN: usize = 16;

#[program]
pub mod coloc_oracle {
    use super::*;

    /// Store (or overwrite) the colocation recommendation produced off-chain.
    pub fn set_colocation(
        ctx: Context<SetColocation>,
        schema_version: u32,
        generated_unix: i64,
        primary_index: u8,
        venues: Vec<VenueRecord>,
    ) -> Result<()> {
        require!(
            !venues.is_empty() && venues.len() <= MAX_VENUES,
            OracleError::InvalidInput
        );
        require!(
            (primary_index as usize) < venues.len(),
            OracleError::InvalidInput
        );

        let report = &mut ctx.accounts.colocation_report;
        report.authority = ctx.accounts.authority.key();
        report.last_update = Clock::get()?.unix_timestamp;
        report.generated_unix = generated_unix;
        report.schema_version = schema_version;
        report.num_venues = venues.len() as u8;
        report.primary_index = primary_index;
        report.bump = ctx.bumps.colocation_report;

        report.venues = Default::default();
        for (i, v) in venues.into_iter().enumerate() {
            report.venues[i] = v;
        }

        msg!(
            "Colocation report stored: {} venues, primary #{}",
            report.num_venues,
            report.primary_index
        );
        Ok(())
    }
}

#[derive(Accounts)]
pub struct SetColocation<'info> {
    #[account(
        init_if_needed,
        payer = authority,
        space = ColocationReport::LEN,
        seeds = [b"colocation", authority.key().as_ref()],
        bump
    )]
    pub colocation_report: Account<'info, ColocationReport>,

    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// One exchange's colocation recommendation, byte-packed for on-chain storage.
///
/// Latitude/longitude are micro-degrees (`degrees * 1_000_000`) to avoid floats.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default, Debug)]
pub struct VenueRecord {
    /// Zero-padded exchange id, e.g. "binance".
    pub exchange: [u8; EXCHANGE_LEN],
    /// Zero-padded AWS region code, e.g. "ap-northeast-1".
    pub region: [u8; REGION_LEN],
    pub lat_micro: i64,
    pub lon_micro: i64,
    /// Recommended radius in metres (10 km = 10000).
    pub radius_m: u32,
    pub rest_latency_ms: u32,
    pub ws_latency_ms: u32,
    /// Confidence in basis points (0..=10000).
    pub confidence_bps: u16,
    /// 0 = aws-ip-range, 1 = ip-geo-nearest, 2 = curated.
    pub method_code: u8,
    pub sample_count: u16,
}

impl VenueRecord {
    pub const LEN: usize =
        EXCHANGE_LEN + REGION_LEN + 8 + 8 + 4 + 4 + 4 + 2 + 1 + 2;
}

/// PDA holding the latest colocation recommendation for all venues.
#[account]
pub struct ColocationReport {
    pub authority: Pubkey,
    /// On-chain clock timestamp of the last write.
    pub last_update: i64,
    /// Off-chain probe timestamp (unix seconds).
    pub generated_unix: i64,
    pub schema_version: u32,
    pub num_venues: u8,
    /// Index into `venues` of the single best overall pick.
    pub primary_index: u8,
    pub venues: [VenueRecord; MAX_VENUES],
    pub bump: u8,
}

impl ColocationReport {
    pub const LEN: usize = 8       // discriminator
        + 32                       // authority
        + 8                        // last_update
        + 8                        // generated_unix
        + 4                        // schema_version
        + 1                        // num_venues
        + 1                        // primary_index
        + VenueRecord::LEN * MAX_VENUES
        + 1; // bump
}

#[error_code]
pub enum OracleError {
    #[msg("Invalid colocation report input")]
    InvalidInput,
}
