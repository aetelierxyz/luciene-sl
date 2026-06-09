//! IP geolocation via ip-api.com (free, no token, ~45 req/min).

use std::net::IpAddr;

use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct GeoInfo {
    pub city: String,
    pub country: String,
    pub lat: f64,
    pub lon: f64,
}

#[derive(Debug, Deserialize)]
struct IpApiResp {
    status: String,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    city: Option<String>,
    #[serde(default)]
    country: Option<String>,
    #[serde(default)]
    lat: Option<f64>,
    #[serde(default)]
    lon: Option<f64>,
}

/// Geolocate a single IP. Returns `None` when the service has no fix.
pub async fn locate(client: &reqwest::Client, ip: IpAddr) -> Result<Option<GeoInfo>> {
    let url = format!("http://ip-api.com/json/{ip}?fields=status,message,city,country,lat,lon");
    let resp: IpApiResp = client.get(&url).send().await?.json().await?;
    if resp.status != "success" {
        tracing::warn!(?ip, msg = ?resp.message, "ip-api lookup failed");
        return Ok(None);
    }
    match (resp.lat, resp.lon) {
        (Some(lat), Some(lon)) => Ok(Some(GeoInfo {
            city: resp.city.unwrap_or_default(),
            country: resp.country.unwrap_or_default(),
            lat,
            lon,
        })),
        _ => Ok(None),
    }
}
