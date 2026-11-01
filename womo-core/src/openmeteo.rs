//! Open-Meteo forecast client and response parsing.
//!
//! Multiple locations per request (comma-separated lat/lon) return a JSON array,
//! a single location returns one object. Both are handled.

use crate::time::{resolve_local, to_local};
use anyhow::{anyhow, bail, Context, Result};
use chrono::{NaiveDate, NaiveDateTime};
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;

pub const DEFAULT_URL: &str = "https://api.open-meteo.com/v1/forecast";
pub const MODEL: &str = "icon_seamless";
pub const HOURLY_VARS: &str = "sunshine_duration,cloud_cover,precipitation_probability,precipitation,temperature_2m,wind_speed_10m,wind_direction_10m,is_day";
pub const DAILY_VARS: &str = "sunrise,sunset,sunshine_duration,temperature_2m_max,temperature_2m_min,precipitation_probability_max,wind_speed_10m_max,wind_direction_10m_dominant";
pub const BATCH_SIZE: usize = 100;
pub const USER_AGENT: &str = concat!(
    "womo-wetter/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/; sunshine overview for camper trips)"
);

/// Hourly series of one location. Times are unix seconds (UTC).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HourlySeries {
    pub time: Vec<i64>,
    /// Seconds of sunshine in the preceding hour.
    pub sunshine: Vec<Option<f64>>,
    pub cloud_cover: Vec<Option<f64>>,
    pub precip_prob: Vec<Option<f64>>,
    /// mm
    pub precip: Vec<Option<f64>>,
    pub temp: Vec<Option<f64>>,
    /// km/h
    pub wind: Vec<Option<f64>>,
    pub wdir: Vec<Option<f64>>,
    /// 1 = day, 0 = night
    pub is_day: Vec<Option<f64>>,
}

/// Values of one location for one hour.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HourSample {
    pub sunshine: Option<f64>,
    pub precip_prob: Option<f64>,
    pub precip: Option<f64>,
    pub temp: Option<f64>,
    pub wind: Option<f64>,
    pub wdir: Option<f64>,
    pub is_day: Option<f64>,
}

impl HourlySeries {
    pub fn with_capacity(n: usize) -> Self {
        HourlySeries {
            time: Vec::with_capacity(n),
            sunshine: Vec::with_capacity(n),
            cloud_cover: Vec::with_capacity(n),
            precip_prob: Vec::with_capacity(n),
            precip: Vec::with_capacity(n),
            temp: Vec::with_capacity(n),
            wind: Vec::with_capacity(n),
            wdir: Vec::with_capacity(n),
            is_day: Vec::with_capacity(n),
        }
    }

    /// Sample at unix time `t`, if present.
    pub fn at(&self, t: i64) -> Option<HourSample> {
        let i = self.time.binary_search(&t).ok()?;
        Some(HourSample {
            sunshine: self.sunshine[i],
            precip_prob: self.precip_prob[i],
            precip: self.precip[i],
            temp: self.temp[i],
            wind: self.wind[i],
            wdir: self.wdir[i],
            is_day: self.is_day[i],
        })
    }
}

/// Daily series of one location.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DailySeries {
    pub date: Vec<NaiveDate>,
    /// Unix seconds.
    pub sunrise: Vec<Option<i64>>,
    pub sunset: Vec<Option<i64>>,
    /// Seconds of sunshine per day.
    pub sunshine: Vec<Option<f64>>,
    pub tmax: Vec<Option<f64>>,
    pub tmin: Vec<Option<f64>>,
    pub precip_prob_max: Vec<Option<f64>>,
    pub wind_max: Vec<Option<f64>>,
    pub wdir_dominant: Vec<Option<f64>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DaySample {
    pub sunrise: Option<i64>,
    pub sunset: Option<i64>,
    pub sunshine: Option<f64>,
    pub tmax: Option<f64>,
    pub tmin: Option<f64>,
    pub precip_prob_max: Option<f64>,
    pub wind_max: Option<f64>,
    pub wdir_dominant: Option<f64>,
}

impl DailySeries {
    pub fn at(&self, d: NaiveDate) -> Option<DaySample> {
        let i = self.date.iter().position(|x| *x == d)?;
        Some(DaySample {
            sunrise: self.sunrise[i],
            sunset: self.sunset[i],
            sunshine: self.sunshine[i],
            tmax: self.tmax[i],
            tmin: self.tmin[i],
            precip_prob_max: self.precip_prob_max[i],
            wind_max: self.wind_max[i],
            wdir_dominant: self.wdir_dominant[i],
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PointWeather {
    pub hourly: HourlySeries,
    pub daily: DailySeries,
}

// --- raw response ------------------------------------------------------------

/// Time value: unix seconds (`timeformat=unixtime`) or local ISO string.
#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum TimeVal {
    Unix(i64),
    Iso(String),
}

#[derive(Deserialize)]
struct RawLocation {
    hourly: Option<RawHourly>,
    daily: Option<RawDaily>,
}

#[derive(Deserialize)]
struct RawHourly {
    time: Vec<TimeVal>,
    #[serde(default)]
    sunshine_duration: Vec<Option<f64>>,
    #[serde(default)]
    cloud_cover: Vec<Option<f64>>,
    #[serde(default)]
    precipitation_probability: Vec<Option<f64>>,
    #[serde(default)]
    precipitation: Vec<Option<f64>>,
    #[serde(default)]
    temperature_2m: Vec<Option<f64>>,
    #[serde(default)]
    wind_speed_10m: Vec<Option<f64>>,
    #[serde(default)]
    wind_direction_10m: Vec<Option<f64>>,
    #[serde(default)]
    is_day: Vec<Option<f64>>,
}

#[derive(Deserialize)]
struct RawDaily {
    time: Vec<TimeVal>,
    #[serde(default)]
    sunrise: Vec<Option<TimeVal>>,
    #[serde(default)]
    sunset: Vec<Option<TimeVal>>,
    #[serde(default)]
    sunshine_duration: Vec<Option<f64>>,
    #[serde(default)]
    temperature_2m_max: Vec<Option<f64>>,
    #[serde(default)]
    temperature_2m_min: Vec<Option<f64>>,
    #[serde(default)]
    precipitation_probability_max: Vec<Option<f64>>,
    #[serde(default)]
    wind_speed_10m_max: Vec<Option<f64>>,
    #[serde(default)]
    wind_direction_10m_dominant: Vec<Option<f64>>,
}

/// Pads/truncates a series to `n` entries.
fn fit<T: Clone>(mut v: Vec<Option<T>>, n: usize) -> Vec<Option<T>> {
    v.resize(n, None);
    v
}

/// Converts times to unix seconds; ISO strings are Europe/Berlin wall clock.
fn times_to_unix(times: &[TimeVal]) -> Result<Vec<i64>> {
    let mut out = Vec::with_capacity(times.len());
    let mut prev = None;
    for t in times {
        let u = match t {
            TimeVal::Unix(u) => *u,
            TimeVal::Iso(s) => {
                let n = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M")
                    .with_context(|| format!("bad time {s:?}"))?;
                resolve_local(n, prev)
            }
        };
        prev = Some(u);
        out.push(u);
    }
    Ok(out)
}

fn day_of(t: &TimeVal) -> Result<NaiveDate> {
    Ok(match t {
        TimeVal::Unix(u) => to_local(*u).date_naive(),
        TimeVal::Iso(s) => NaiveDate::parse_from_str(&s[..s.len().min(10)], "%Y-%m-%d")
            .with_context(|| format!("bad date {s:?}"))?,
    })
}

fn opt_instant(t: &Option<TimeVal>) -> Option<i64> {
    match t.as_ref()? {
        TimeVal::Unix(u) => Some(*u),
        TimeVal::Iso(s) => NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M")
            .ok()
            .map(|n| resolve_local(n, None)),
    }
}

impl RawLocation {
    fn into_point(self) -> Result<PointWeather> {
        let hourly = match self.hourly {
            Some(h) => {
                let n = h.time.len();
                HourlySeries {
                    time: times_to_unix(&h.time)?,
                    sunshine: fit(h.sunshine_duration, n),
                    cloud_cover: fit(h.cloud_cover, n),
                    precip_prob: fit(h.precipitation_probability, n),
                    precip: fit(h.precipitation, n),
                    temp: fit(h.temperature_2m, n),
                    wind: fit(h.wind_speed_10m, n),
                    wdir: fit(h.wind_direction_10m, n),
                    is_day: fit(h.is_day, n),
                }
            }
            None => HourlySeries::default(),
        };
        if hourly.time.windows(2).any(|w| w[0] >= w[1]) {
            bail!("hourly times not strictly increasing");
        }
        let daily = match self.daily {
            Some(d) => {
                let n = d.time.len();
                let mut sunrise = d.sunrise;
                sunrise.resize_with(n, || None);
                let mut sunset = d.sunset;
                sunset.resize_with(n, || None);
                DailySeries {
                    date: d.time.iter().map(day_of).collect::<Result<_>>()?,
                    sunrise: sunrise.iter().map(opt_instant).collect(),
                    sunset: sunset.iter().map(opt_instant).collect(),
                    sunshine: fit(d.sunshine_duration, n),
                    tmax: fit(d.temperature_2m_max, n),
                    tmin: fit(d.temperature_2m_min, n),
                    precip_prob_max: fit(d.precipitation_probability_max, n),
                    wind_max: fit(d.wind_speed_10m_max, n),
                    wdir_dominant: fit(d.wind_direction_10m_dominant, n),
                }
            }
            None => DailySeries::default(),
        };
        Ok(PointWeather { hourly, daily })
    }
}

/// Parses a forecast response (array for multiple locations, object for one).
pub fn parse_response(body: &str) -> Result<Vec<PointWeather>> {
    let v: Value = serde_json::from_str(body).context("response is not JSON")?;
    if let Some(reason) = api_error(&v) {
        bail!("Open-Meteo error: {reason}");
    }
    let items = match v {
        Value::Array(a) => a,
        obj @ Value::Object(_) => vec![obj],
        _ => bail!("unexpected response shape"),
    };
    items
        .into_iter()
        .enumerate()
        .map(|(i, it)| {
            let raw: RawLocation =
                serde_json::from_value(it).with_context(|| format!("location #{i}"))?;
            raw.into_point().with_context(|| format!("location #{i}"))
        })
        .collect()
}

fn api_error(v: &Value) -> Option<String> {
    let obj = v.as_object()?;
    if obj.get("error").and_then(Value::as_bool) == Some(true) {
        return Some(
            obj.get("reason")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string(),
        );
    }
    None
}

/// Request URL for a batch of (lon, lat) coordinates.
pub fn build_url(base: &str, coords: &[(f64, f64)], apikey: Option<&str>) -> String {
    let lats: Vec<String> = coords.iter().map(|c| format!("{:.4}", c.1)).collect();
    let lons: Vec<String> = coords.iter().map(|c| format!("{:.4}", c.0)).collect();
    let mut url = format!(
        "{base}?latitude={}&longitude={}&hourly={HOURLY_VARS}&daily={DAILY_VARS}\
         &models={MODEL}&timezone=Europe%2FBerlin&forecast_days=7&timeformat=unixtime",
        lats.join(","),
        lons.join(",")
    );
    if let Some(k) = apikey {
        url.push_str("&apikey=");
        url.push_str(k);
    }
    url
}

/// Blocking Open-Meteo client with batching and retries.
pub struct Client {
    http: reqwest::blocking::Client,
    base_url: String,
    apikey: Option<String>,
    pub batch_size: usize,
    /// Pause between batches (rate limit: 600 calls/min, each location ≈ 1.6 calls).
    pub pause: Duration,
    pub retries: u32,
}

impl Client {
    pub fn new(base_url: &str, apikey: Option<String>, pause: Duration) -> Result<Self> {
        let http = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(90))
            .build()?;
        Ok(Client {
            http,
            base_url: base_url.to_string(),
            apikey,
            batch_size: BATCH_SIZE,
            pause,
            retries: 3,
        })
    }

    /// Fetches all coordinates `(lon, lat)`; result order matches the input.
    pub fn fetch_all(&self, coords: &[(f64, f64)]) -> Result<Vec<PointWeather>> {
        let mut out = Vec::with_capacity(coords.len());
        let n_batches = coords.len().div_ceil(self.batch_size);
        for (b, chunk) in coords.chunks(self.batch_size).enumerate() {
            if b > 0 {
                std::thread::sleep(self.pause);
            }
            eprintln!(
                "Open-Meteo: Paket {}/{} ({} Orte)",
                b + 1,
                n_batches,
                chunk.len()
            );
            let pts = self
                .fetch_batch(chunk)
                .with_context(|| format!("batch {}/{}", b + 1, n_batches))?;
            if pts.len() != chunk.len() {
                bail!(
                    "batch {}: expected {} locations, got {}",
                    b + 1,
                    chunk.len(),
                    pts.len()
                );
            }
            out.extend(pts);
        }
        Ok(out)
    }

    fn fetch_batch(&self, coords: &[(f64, f64)]) -> Result<Vec<PointWeather>> {
        let url = build_url(&self.base_url, coords, self.apikey.as_deref());
        let mut last_err = anyhow!("no attempt");
        for attempt in 1..=self.retries {
            match self.http.get(&url).send() {
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().unwrap_or_default();
                    if status.is_success() {
                        return parse_response(&body);
                    }
                    let reason = serde_json::from_str::<Value>(&body)
                        .ok()
                        .and_then(|v| api_error(&v))
                        .unwrap_or_else(|| body.chars().take(200).collect());
                    last_err = anyhow!("HTTP {status}: {reason}");
                    // Client errors other than rate limiting are not retried.
                    if status.is_client_error() && status.as_u16() != 429 {
                        return Err(last_err);
                    }
                    if status.as_u16() == 429 && attempt < self.retries {
                        eprintln!("  Rate-Limit erreicht, warte 65 s …");
                        std::thread::sleep(Duration::from_secs(65));
                        continue;
                    }
                }
                Err(e) => last_err = anyhow!(e.without_url()),
            }
            if attempt < self.retries {
                let wait = Duration::from_secs(5 * 3u64.pow(attempt - 1));
                eprintln!("  Versuch {attempt} fehlgeschlagen ({last_err:#}), warte {wait:?}");
                std::thread::sleep(wait);
            }
        }
        Err(last_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MULTI: &str = include_str!("../tests/fixtures/openmeteo_multi.json");
    const SINGLE_ISO: &str = include_str!("../tests/fixtures/openmeteo_single_iso.json");

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn parse_multi_location_unixtime() {
        let pts = parse_response(MULTI).unwrap();
        assert_eq!(pts.len(), 2);
        let p = &pts[0];
        // 2026-11-07T00:00+01:00
        assert_eq!(p.hourly.time[0], 1_794_006_000);
        assert_eq!(p.hourly.time.len(), 4);
        let s = p.hourly.at(1_794_006_000 + 10 * 3600).unwrap();
        assert_eq!(s.sunshine, Some(3600.0));
        assert_eq!(s.is_day, Some(1.0));
        assert_eq!(s.wdir, Some(230.0));
        // Null values become None.
        let s1 = p.hourly.at(1_794_006_000 + 3600).unwrap();
        assert_eq!(s1.temp, None);
        assert!(p.hourly.at(1_794_006_000 + 5 * 3600).is_none());
        let day = p.daily.at(d("2026-11-07")).unwrap();
        assert_eq!(crate::time::fmt_hm(day.sunrise.unwrap()), "07:32");
        assert_eq!(crate::time::fmt_hm(day.sunset.unwrap()), "16:59");
        assert_eq!(day.sunshine, Some(24120.0));
        assert_eq!(p.daily.date[1], d("2026-11-08"));
        // Second location differs.
        assert_eq!(
            pts[1].hourly.at(1_794_006_000).unwrap().precip_prob,
            Some(80.0)
        );
        assert_eq!(pts[1].daily.at(d("2026-11-08")).unwrap().tmax, Some(9.4));
    }

    #[test]
    fn parse_single_location_iso_times() {
        let pts = parse_response(SINGLE_ISO).unwrap();
        assert_eq!(pts.len(), 1);
        let p = &pts[0];
        assert_eq!(
            crate::time::fmt_local(p.hourly.time[0]),
            "2026-11-07T00:00+01:00"
        );
        assert_eq!(
            crate::time::fmt_local(p.hourly.time[2]),
            "2026-11-07T02:00+01:00"
        );
        let day = p.daily.at(d("2026-11-07")).unwrap();
        assert_eq!(crate::time::fmt_hm(day.sunset.unwrap()), "16:59");
        // Missing variable arrays are padded with None.
        assert_eq!(p.hourly.wdir, vec![None, None, None]);
    }

    #[test]
    fn api_error_is_reported() {
        let e = parse_response(r#"{"error":true,"reason":"Latitude must be in range"}"#)
            .unwrap_err()
            .to_string();
        assert!(e.contains("Latitude must be in range"), "{e}");
    }

    #[test]
    fn url_contains_contract_params() {
        let u = build_url(DEFAULT_URL, &[(6.5, 50.25), (7.0, 51.0)], None);
        assert!(u.starts_with("https://api.open-meteo.com/v1/forecast?latitude=50.2500,51.0000&longitude=6.5000,7.0000&"));
        assert!(u.contains("models=icon_seamless"));
        assert!(u.contains("timezone=Europe%2FBerlin"));
        assert!(u.contains("forecast_days=7"));
        assert!(u.contains(&format!("hourly={HOURLY_VARS}")));
        assert!(u.contains(&format!("daily={DAILY_VARS}")));
    }
}
