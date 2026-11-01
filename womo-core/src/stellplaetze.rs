//! Camper sites from OpenStreetMap via Overpass (DATENFORMAT §4).

use crate::export::Stellplatz;
use crate::openmeteo::USER_AGENT;
use crate::regions::Regions;
use anyhow::{anyhow, Context, Result};
use geo::Rect;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;

pub const OVERPASS_URL: &str = "https://overpass-api.de/api/interpreter";

/// Overpass QL query for `tourism=caravan_site|camp_site` in a bbox.
pub fn build_query(bbox: Rect<f64>) -> String {
    let b = format!(
        "({:.4},{:.4},{:.4},{:.4})",
        bbox.min().y,
        bbox.min().x,
        bbox.max().y,
        bbox.max().x
    );
    let t = "[\"tourism\"~\"^(caravan_site|camp_site)$\"]";
    format!("[out:json][timeout:240];(node{t}{b};way{t}{b};relation{t}{b};);out center tags;")
}

#[derive(Deserialize)]
struct Response {
    elements: Vec<Element>,
}

#[derive(Deserialize)]
struct Element {
    #[serde(rename = "type")]
    kind: String,
    id: u64,
    lat: Option<f64>,
    lon: Option<f64>,
    center: Option<Center>,
    #[serde(default)]
    tags: HashMap<String, String>,
}

#[derive(Deserialize)]
struct Center {
    lat: f64,
    lon: f64,
}

fn round5(x: f64) -> f64 {
    (x * 1e5).round() / 1e5
}

fn non_empty(tags: &HashMap<String, String>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .filter_map(|k| tags.get(*k))
        .map(|v| v.trim())
        .find(|v| !v.is_empty())
        .map(str::to_string)
}

/// Parses an Overpass JSON response and maps sites to regions.
/// Elements outside all regions or without coordinates are dropped.
pub fn parse_overpass(body: &str, regions: &Regions) -> Result<Vec<Stellplatz>> {
    let resp: Response = serde_json::from_str(body).context("invalid Overpass response")?;
    let mut items: Vec<Stellplatz> = resp
        .elements
        .into_iter()
        .filter_map(|e| {
            let prefix = match e.kind.as_str() {
                "node" => 'n',
                "way" => 'w',
                "relation" => 'r',
                _ => return None,
            };
            let (lat, lon) = match (e.lat, e.lon, &e.center) {
                (Some(la), Some(lo), _) => (la, lo),
                (_, _, Some(c)) => (c.lat, c.lon),
                _ => return None,
            };
            let kind = match e.tags.get("tourism").map(String::as_str) {
                Some("camp_site") => "camp_site",
                _ => "caravan_site",
            };
            // Skip places that are private or explicitly closed to motorhomes.
            let no = |k: &str| e.tags.get(k).is_some_and(|v| v == "no" || v == "private");
            if no("access") || no("motorhome") || (kind == "camp_site" && no("caravans")) {
                return None;
            }
            let region = regions.locate(lon, lat)?;
            Some(Stellplatz {
                id: format!("{prefix}{}", e.id),
                name: non_empty(&e.tags, &["name", "name:de", "operator"]).unwrap_or_else(|| {
                    if kind == "camp_site" {
                        "Campingplatz"
                    } else {
                        "Stellplatz"
                    }
                    .to_string()
                }),
                lon: round5(lon),
                lat: round5(lat),
                region: regions.list[region].id.clone(),
                kind: kind.to_string(),
                fee: non_empty(&e.tags, &["fee"]),
                website: non_empty(&e.tags, &["website", "contact:website", "url"]),
            })
        })
        .collect();
    items.sort_by(|a, b| a.id.cmp(&b.id));
    items.dedup_by(|a, b| a.id == b.id);
    Ok(items)
}

/// Runs the Overpass query (3 attempts with backoff).
pub fn fetch_overpass(url: &str, query: &str) -> Result<String> {
    let http = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(300))
        .build()?;
    let mut last = anyhow!("no attempt");
    for attempt in 1..=3u32 {
        match http.post(url).form(&[("data", query)]).send() {
            Ok(r) if r.status().is_success() => return Ok(r.text()?),
            Ok(r) => last = anyhow!("HTTP {}", r.status()),
            Err(e) => last = anyhow!(e.without_url()),
        }
        if attempt < 3 {
            let wait = Duration::from_secs(30 * attempt as u64);
            eprintln!("Overpass: Versuch {attempt} fehlgeschlagen ({last:#}), warte {wait:?}");
            std::thread::sleep(wait);
        }
    }
    Err(last)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn regions() -> Regions {
        Regions::parse(include_str!("../tests/fixtures/two_regions.geojson")).unwrap()
    }

    #[test]
    fn parse_fixture() {
        let body = include_str!("../tests/fixtures/overpass.json");
        let items = parse_overpass(body, &regions()).unwrap();
        let ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
        // n3 lies outside all regions, the coordless element is dropped,
        // n13 (no caravans) and n14 (private) are filtered out.
        assert_eq!(ids, vec!["n1", "n2", "r9", "w12", "w5"]);
        let w12 = &items[3];
        assert_eq!(
            (w12.kind.as_str(), w12.name.as_str()),
            ("camp_site", "Campingplatz")
        );
        let n1 = &items[0];
        assert_eq!(n1.name, "Wohnmobilstellplatz am See");
        assert_eq!(n1.region, "west");
        assert_eq!(n1.fee.as_deref(), Some("yes"));
        assert_eq!(n1.website.as_deref(), Some("https://example.org"));
        assert_eq!((n1.lon, n1.lat), (6.81235, 50.21));
        let n2 = &items[1];
        assert_eq!(n2.name, "Stellplatz");
        assert_eq!(n2.fee, None);
        assert_eq!(n2.website.as_deref(), Some("https://camp.example"));
        let w5 = &items[4];
        assert_eq!(w5.region, "east");
        assert_eq!((w5.lon, w5.lat), (7.5, 50.7));
        let json = serde_json::to_string(n2).unwrap();
        assert!(!json.contains("fee") && !json.contains("null"));
    }

    #[test]
    fn query_shape() {
        let q = build_query(regions().bbox());
        assert!(q.starts_with("[out:json][timeout:240];"));
        assert!(q.contains(
            "node[\"tourism\"~\"^(caravan_site|camp_site)$\"](50.0000,6.0000,51.0000,7.6000);"
        ));
        assert!(q.contains("way[\"tourism\"~"));
        assert!(q.contains("relation[\"tourism\"~"));
        assert!(q.ends_with("out center tags;"));
    }
}
