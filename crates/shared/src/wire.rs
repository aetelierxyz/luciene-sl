//! The on-chain byte layout, defined once so the Anchor program, the publisher
//! and the dashboard decoder all agree.
//!
//! Latitude/longitude are stored as **micro-degrees** (`degrees * 1_000_000`) in
//! `i64` to avoid floats on-chain. Strings are fixed, zero-padded byte arrays.

use sha2::{Digest, Sha256};

/// Max venues stored on-chain in a single report account.
pub const MAX_VENUES: usize = 21;
/// Fixed width of the exchange id field.
pub const EXCHANGE_LEN: usize = 12;
/// Fixed width of the AWS region code field.
pub const REGION_LEN: usize = 16;

/// Degrees <-> micro-degrees helpers.
pub fn to_micro(deg: f64) -> i64 {
    (deg * 1_000_000.0).round() as i64
}
pub fn from_micro(micro: i64) -> f64 {
    micro as f64 / 1_000_000.0
}

/// Pad a string into a fixed-size, zero-filled byte array (truncating if long).
pub fn pad<const N: usize>(s: &str) -> [u8; N] {
    let mut out = [0u8; N];
    let b = s.as_bytes();
    let n = b.len().min(N);
    out[..n].copy_from_slice(&b[..n]);
    out
}

/// Read a zero-padded fixed byte array back into a String.
pub fn unpad(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// Anchor instruction discriminator: first 8 bytes of `sha256("global:<name>")`.
pub fn ix_discriminator(name: &str) -> [u8; 8] {
    let mut h = Sha256::new();
    h.update(format!("global:{name}").as_bytes());
    let d = h.finalize();
    let mut out = [0u8; 8];
    out.copy_from_slice(&d[..8]);
    out
}

/// Anchor account discriminator: first 8 bytes of `sha256("account:<Name>")`.
pub fn account_discriminator(name: &str) -> [u8; 8] {
    let mut h = Sha256::new();
    h.update(format!("account:{name}").as_bytes());
    let d = h.finalize();
    let mut out = [0u8; 8];
    out.copy_from_slice(&d[..8]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn micro_roundtrip() {
        let v = 35.6895_f64;
        assert!((from_micro(to_micro(v)) - v).abs() < 1e-6);
    }

    #[test]
    fn pad_unpad() {
        let p: [u8; 12] = pad("binance");
        assert_eq!(unpad(&p), "binance");
    }
}
