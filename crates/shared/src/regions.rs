//! AWS region catalogue: approximate data-center coordinates and helpers to map
//! an arbitrary geo point to the nearest region.

/// Approximate latitude/longitude of the primary data-center cluster for each
/// AWS region we care about. Coordinates are the commonly-cited locations of the
/// physical AZ clusters (e.g. us-east-1 ≈ Ashburn, VA).
pub const AWS_REGIONS: &[AwsRegion] = &[
    AwsRegion { code: "us-east-1", city: "Ashburn, Virginia, USA", lat: 39.0438, lon: -77.4874 },
    AwsRegion { code: "us-east-2", city: "Columbus, Ohio, USA", lat: 39.9612, lon: -82.9988 },
    AwsRegion { code: "us-west-1", city: "San Jose, California, USA", lat: 37.3382, lon: -121.8863 },
    AwsRegion { code: "us-west-2", city: "Boardman, Oregon, USA", lat: 45.8696, lon: -119.6880 },
    AwsRegion { code: "ca-central-1", city: "Montreal, Canada", lat: 45.5017, lon: -73.5673 },
    AwsRegion { code: "eu-west-1", city: "Dublin, Ireland", lat: 53.3498, lon: -6.2603 },
    AwsRegion { code: "eu-west-2", city: "London, United Kingdom", lat: 51.5074, lon: -0.1278 },
    AwsRegion { code: "eu-west-3", city: "Paris, France", lat: 48.8566, lon: 2.3522 },
    AwsRegion { code: "eu-central-1", city: "Frankfurt, Germany", lat: 50.1109, lon: 8.6821 },
    AwsRegion { code: "eu-north-1", city: "Stockholm, Sweden", lat: 59.3293, lon: 18.0686 },
    AwsRegion { code: "ap-east-1", city: "Hong Kong", lat: 22.3193, lon: 114.1694 },
    AwsRegion { code: "ap-northeast-1", city: "Tokyo, Japan", lat: 35.6895, lon: 139.6917 },
    AwsRegion { code: "ap-northeast-2", city: "Seoul, South Korea", lat: 37.5665, lon: 126.9780 },
    AwsRegion { code: "ap-northeast-3", city: "Osaka, Japan", lat: 34.6937, lon: 135.5023 },
    AwsRegion { code: "ap-southeast-1", city: "Singapore", lat: 1.3521, lon: 103.8198 },
    AwsRegion { code: "ap-southeast-2", city: "Sydney, Australia", lat: -33.8688, lon: 151.2093 },
    AwsRegion { code: "ap-south-1", city: "Mumbai, India", lat: 19.0760, lon: 72.8777 },
    AwsRegion { code: "sa-east-1", city: "Sao Paulo, Brazil", lat: -23.5505, lon: -46.6333 },
];

#[derive(Debug, Clone, Copy)]
pub struct AwsRegion {
    pub code: &'static str,
    pub city: &'static str,
    pub lat: f64,
    pub lon: f64,
}

/// Look up a region by its AWS code (e.g. `"ap-northeast-1"`).
pub fn region_by_code(code: &str) -> Option<&'static AwsRegion> {
    AWS_REGIONS.iter().find(|r| r.code == code)
}

/// Great-circle distance between two points in kilometres (haversine).
pub fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const R: f64 = 6371.0;
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dlon / 2.0).sin().powi(2);
    2.0 * R * a.sqrt().asin()
}

/// Nearest AWS region to an arbitrary geo point, with the distance in km.
pub fn nearest_region(lat: f64, lon: f64) -> (&'static AwsRegion, f64) {
    AWS_REGIONS
        .iter()
        .map(|r| (r, haversine_km(lat, lon, r.lat, r.lon)))
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .expect("AWS_REGIONS is non-empty")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokyo_maps_to_ap_northeast_1() {
        let (r, d) = nearest_region(35.6, 139.7);
        assert_eq!(r.code, "ap-northeast-1");
        assert!(d < 50.0);
    }

    #[test]
    fn ashburn_maps_to_us_east_1() {
        let (r, _) = nearest_region(39.04, -77.48);
        assert_eq!(r.code, "us-east-1");
    }

    #[test]
    fn haversine_known_distance() {
        // London <-> Paris ≈ 343 km
        let d = haversine_km(51.5074, -0.1278, 48.8566, 2.3522);
        assert!((d - 343.0).abs() < 15.0, "got {d}");
    }
}
