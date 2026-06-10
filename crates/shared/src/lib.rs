//! Shared types and tables for the CEX colocation oracle.

pub mod regions;
pub mod report;
pub mod venues;
pub mod wire;

pub use report::{
    EndpointProbe, Method, OnChainRef, Report, VenueRecommendation,
};

/// The deployed colocation-oracle program id (devnet).
/// Mirrors `declare_id!` in `programs/coloc_oracle`.
pub const PROGRAM_ID: &str = "GH9e1ZjDTRo4WKgJYxE1TZHSFFqgXxsDVTB4manBvvh1";

/// PDA seed for the colocation report account (versioned: bumping MAX_VENUES
/// changes the account size, so v2 yields a fresh, correctly-sized account).
pub const COLOCATION_SEED: &[u8] = b"colocation-v2";
