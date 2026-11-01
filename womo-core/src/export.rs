//! Output structures (DATENFORMAT §2–§4) and JSON writing.

use crate::aggregate::grid_char;
use crate::openmeteo::PointWeather;
use crate::regions::{Grid, SamplePoint};
use crate::time::TimeAxis;
use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct Latest {
    pub generated: String,
    pub source: String,
    pub mock: bool,
    pub hours: Vec<String>,
    pub days: Vec<String>,
    pub regions: BTreeMap<String, RegionOut>,
}

#[derive(Debug, Serialize)]
pub struct RegionOut {
    pub hourly: RegionHourly,
    pub daily: Vec<RegionDaily>,
}

#[derive(Debug, Serialize)]
pub struct RegionHourly {
    pub cls: String,
    pub sun: Vec<i32>,
    pub sunny: Vec<i32>,
    pub rain: Vec<i32>,
    pub temp: Vec<i32>,
    pub wind: Vec<i32>,
    pub wdir: Vec<i32>,
}

#[derive(Debug, Serialize)]
pub struct RegionDaily {
    pub date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sunrise: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sunset: Option<String>,
    pub sun_h: f64,
    pub tmax: i32,
    pub tmin: i32,
    pub rain: i32,
    pub wind: i32,
    pub wdir: i32,
    pub cls: String,
}

#[derive(Debug, Serialize)]
pub struct GridOut {
    pub lon0: f64,
    pub lat0: f64,
    pub step: f64,
    pub nx: usize,
    pub ny: usize,
    pub hours: Vec<String>,
    pub sun: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Stellplaetze {
    pub generated: String,
    pub items: Vec<Stellplatz>,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct Stellplatz {
    pub id: String,
    pub name: String,
    pub lon: f64,
    pub lat: f64,
    pub region: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website: Option<String>,
}

/// Encodes one string per hour, row by row from south (lat0) to north, west → east.
/// `weather[k]` belongs to `points[k]`; points without a cell are ignored.
pub fn encode_grid(
    grid: &Grid,
    points: &[SamplePoint],
    weather: &[PointWeather],
    axis: &TimeAxis,
) -> Vec<String> {
    axis.hours
        .iter()
        .map(|&t| {
            let mut row = vec![b'.'; grid.len()];
            for (p, w) in points.iter().zip(weather) {
                if let Some(c) = p.cell {
                    row[c] = grid_char(w.hourly.at(t)) as u8;
                }
            }
            String::from_utf8(row).expect("ascii")
        })
        .collect()
}

/// Writes compact JSON atomically (temp file + rename).
pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    let data = serde_json::to_vec(value)?;
    std::fs::write(&tmp, &data).with_context(|| format!("cannot write {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("cannot write {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openmeteo::HourlySeries;
    use chrono::DateTime;

    fn weather_at(t: i64, sunshine: f64, is_day: f64, pp: f64, pr: f64) -> PointWeather {
        let mut h = HourlySeries::with_capacity(1);
        h.time.push(t);
        h.sunshine.push(Some(sunshine));
        h.cloud_cover.push(None);
        h.precip_prob.push(Some(pp));
        h.precip.push(Some(pr));
        h.temp.push(Some(10.0));
        h.wind.push(Some(5.0));
        h.wdir.push(Some(180.0));
        h.is_day.push(Some(is_day));
        PointWeather {
            hourly: h,
            daily: Default::default(),
        }
    }

    #[test]
    fn grid_encoding_layout() {
        let axis = TimeAxis::new(
            DateTime::parse_from_rfc3339("2026-11-07T09:00:00+01:00")
                .unwrap()
                .to_utc(),
        );
        let t0 = axis.hours[0];
        let grid = Grid {
            lon0: 6.0,
            lat0: 50.0,
            step: 0.25,
            nx: 3,
            ny: 2,
        };
        let pt = |i: usize, j: usize| SamplePoint {
            lon: 6.0 + i as f64 * 0.25,
            lat: 50.0 + j as f64 * 0.25,
            region: 0,
            cell: Some(j * 3 + i),
        };
        let points = vec![
            pt(0, 0),
            pt(2, 0),
            pt(1, 1),
            pt(2, 1),
            // Extra point (no cell) must not appear.
            SamplePoint {
                lon: 6.1,
                lat: 50.1,
                region: 0,
                cell: None,
            },
        ];
        let weather = vec![
            weather_at(t0, 3600.0, 1.0, 0.0, 0.0),
            weather_at(t0, 1500.0, 1.0, 0.0, 0.0),
            weather_at(t0, 0.0, 1.0, 80.0, 1.5),
            weather_at(t0, 3600.0, 0.0, 0.0, 0.0),
            weather_at(t0, 0.0, 1.0, 0.0, 0.0),
        ];
        let enc = encode_grid(&grid, &points, &weather, &axis);
        assert_eq!(enc.len(), 48);
        // South row: "9.4", north row: ".RN".
        assert_eq!(enc[0], "9.4.RN");
        // Hours without data -> all '.'.
        assert_eq!(enc[1], "......");
    }

    #[test]
    fn stellplatz_omits_missing_fields() {
        let s = Stellplatz {
            id: "n1".into(),
            name: "X".into(),
            lon: 6.1,
            lat: 50.2,
            region: "eifel".into(),
            kind: "caravan_site".into(),
            fee: None,
            website: Some("https://x".into()),
        };
        let j = serde_json::to_string(&s).unwrap();
        assert_eq!(
            j,
            r#"{"id":"n1","name":"X","lon":6.1,"lat":50.2,"region":"eifel","type":"caravan_site","website":"https://x"}"#
        );
    }
}
