//! Map an IP address to an AWS region using the public, tokenless
//! `ip-ranges.json` feed AWS publishes.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use anyhow::{Context, Result};
use serde::Deserialize;

const IP_RANGES_URL: &str = "https://ip-ranges.amazonaws.com/ip-ranges.json";

#[derive(Debug, Deserialize)]
struct IpRanges {
    prefixes: Vec<V4Prefix>,
    #[serde(default)]
    ipv6_prefixes: Vec<V6Prefix>,
}

#[derive(Debug, Deserialize)]
struct V4Prefix {
    ip_prefix: String,
    region: String,
}

#[derive(Debug, Deserialize)]
struct V6Prefix {
    ipv6_prefix: String,
    region: String,
}

/// Parsed AWS CIDR catalogue, ready for membership tests.
pub struct AwsRanges {
    v4: Vec<(u32, u32, String)>,   // (network, mask, region)
    v6: Vec<(u128, u128, String)>, // (network, mask, region)
}

impl AwsRanges {
    /// Download and parse the AWS IP-ranges feed.
    pub async fn fetch(client: &reqwest::Client) -> Result<Self> {
        let raw: IpRanges = client
            .get(IP_RANGES_URL)
            .send()
            .await
            .context("GET ip-ranges.json")?
            .json()
            .await
            .context("parse ip-ranges.json")?;

        let mut v4 = Vec::with_capacity(raw.prefixes.len());
        for p in raw.prefixes {
            if let Some((net, mask)) = parse_v4_cidr(&p.ip_prefix) {
                v4.push((net, mask, p.region));
            }
        }
        let mut v6 = Vec::with_capacity(raw.ipv6_prefixes.len());
        for p in raw.ipv6_prefixes {
            if let Some((net, mask)) = parse_v6_cidr(&p.ipv6_prefix) {
                v6.push((net, mask, p.region));
            }
        }
        tracing::info!(v4 = v4.len(), v6 = v6.len(), "loaded AWS ip-ranges");
        Ok(AwsRanges { v4, v6 })
    }

    /// Return the AWS region containing `ip`, longest-prefix match wins.
    pub fn region_for(&self, ip: IpAddr) -> Option<String> {
        match ip {
            IpAddr::V4(a) => {
                let v = u32::from(a);
                let mut best: Option<(&String, u32)> = None;
                for (net, mask, region) in &self.v4 {
                    if v & mask == *net {
                        let bits = mask.count_ones();
                        if best.map_or(true, |(_, b)| bits > b) {
                            best = Some((region, bits));
                        }
                    }
                }
                best.map(|(r, _)| r.clone())
            }
            IpAddr::V6(a) => {
                let v = u128::from(a);
                let mut best: Option<(&String, u32)> = None;
                for (net, mask, region) in &self.v6 {
                    if v & mask == *net {
                        let bits = mask.count_ones();
                        if best.map_or(true, |(_, b)| bits > b) {
                            best = Some((region, bits));
                        }
                    }
                }
                best.map(|(r, _)| r.clone())
            }
        }
    }
}

/// Well-known Cloudflare IPv4 CIDRs (published at cloudflare.com/ips-v4).
/// Used to recognise CDN-fronted endpoints whose IP hides the real origin.
const CLOUDFLARE_V4: &[&str] = &[
    "173.245.48.0/20", "103.21.244.0/22", "103.22.200.0/22", "103.31.4.0/22",
    "141.101.64.0/18", "108.162.192.0/18", "190.93.240.0/20", "188.114.96.0/20",
    "197.234.240.0/22", "198.41.128.0/17", "162.158.0.0/15", "104.16.0.0/13",
    "104.24.0.0/14", "172.64.0.0/13", "131.0.72.0/22",
];

/// Well-known Akamai IPv4 CIDRs (stable, non-overlapping with AWS/Cloudflare).
/// Akamai is a CDN, so its edge IPs hide the origin just like Cloudflare.
const AKAMAI_V4: &[&str] = &[
    "23.0.0.0/12", "23.32.0.0/11", "23.64.0.0/11", "23.192.0.0/11",
    "2.16.0.0/13", "104.64.0.0/10", "184.24.0.0/13", "184.50.0.0/15",
    "95.100.0.0/15", "96.16.0.0/15", "88.221.0.0/16", "72.246.0.0/15",
];

fn in_any(ip: IpAddr, cidrs: &[&str]) -> bool {
    let IpAddr::V4(a) = ip else { return false };
    let v = u32::from(a);
    cidrs
        .iter()
        .any(|c| matches!(parse_v4_cidr(c), Some((net, mask)) if v & mask == net))
}

/// True if `ip` belongs to a known Cloudflare range (a CDN edge, not an origin).
pub fn is_cloudflare(ip: IpAddr) -> bool {
    in_any(ip, CLOUDFLARE_V4)
}

/// True if `ip` belongs to a known Akamai range.
pub fn is_akamai(ip: IpAddr) -> bool {
    in_any(ip, AKAMAI_V4)
}

/// True if `ip` is a known CDN edge (Cloudflare or Akamai) — its origin region
/// is hidden, so geolocating it is meaningless.
pub fn is_cdn_edge(ip: IpAddr) -> bool {
    is_cloudflare(ip) || is_akamai(ip)
}

fn parse_v4_cidr(cidr: &str) -> Option<(u32, u32)> {
    let (addr, len) = cidr.split_once('/')?;
    let ip: Ipv4Addr = addr.parse().ok()?;
    let len: u32 = len.parse().ok()?;
    if len > 32 {
        return None;
    }
    let mask = if len == 0 { 0 } else { u32::MAX << (32 - len) };
    Some((u32::from(ip) & mask, mask))
}

fn parse_v6_cidr(cidr: &str) -> Option<(u128, u128)> {
    let (addr, len) = cidr.split_once('/')?;
    let ip: Ipv6Addr = addr.parse().ok()?;
    let len: u32 = len.parse().ok()?;
    if len > 128 {
        return None;
    }
    let mask = if len == 0 { 0 } else { u128::MAX << (128 - len) };
    Some((u128::from(ip) & mask, mask))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v4_contains() {
        let (net, mask) = parse_v4_cidr("3.5.140.0/22").unwrap();
        let ranges = AwsRanges {
            v4: vec![(net, mask, "ap-northeast-2".into())],
            v6: vec![],
        };
        let inside: IpAddr = "3.5.141.10".parse().unwrap();
        let outside: IpAddr = "8.8.8.8".parse().unwrap();
        assert_eq!(ranges.region_for(inside).as_deref(), Some("ap-northeast-2"));
        assert_eq!(ranges.region_for(outside), None);
    }
}
