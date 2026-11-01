//! Open-Meteo forecast client and response parsing.
//!
//! Multiple locations per request (comma-separated lat/lon) return a JSON array,
//! a single location returns one object. Both are handled.
//!
//! Only 7 hourly variables are requested (≤ 10 → one API call per location);
//! all daily values are derived from the hourly data.

use crate::time::resolve_local;
use anyhow::{anyhow, bail, Context, Result};
use chrono::NaiveDateTime;
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;

pub const DEFAULT_URL: &str = "https://api.open-meteo.com/v1/forecast";
pub const MODEL: &str = "icon_seamless";
pub const HOURLY_VARS: &str = "sunshine_duration,precipitation_probability,precipitation,temperature_2m,wind_speed_10m,wind_direction_10m,is_day";
pub const BATCH_SIZE: usize = 100;
pub const USER_AGENT: &str = concat!(
    "womo-wetter/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/; sunshine overview for camper trips)"
);

/// Hourly series of one location in Open-Meteo convention. Times are unix seconds (UTC).
///
/// `sunshine`, `precip` and `precip_prob` at time T describe the preceding
/// hour (T-1h..T); `temp`, `wind`, `wdir`, `is_day` are instantaneous at T.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HourlySeries {
    pub time: Vec<i64>,
    /// Seconds of sunshine in the preceding hour.
    pub sunshine: Vec<Option<f64>>,
    pub precip_prob: Vec<Option<f64>>,
    /// mm in the preceding hour
    pub precip: Vec<Option<f64>>,
    pub temp: Vec<Option<f64>>,
    /// km/h
    pub wind: Vec<Option<f64>>,
    pub wdir: Vec<Option<f64>>,
    /// 1 = day, 0 = night
    pub is_day: Vec<Option<f64>>,
}

/// Values of one location for the output hour h..h+1.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HourSample {
    pub sunshine: Option<f64>,
    pub precip_prob: Option<f64>,
    pub precip: Option<f64>,
    pub temp: Option<f64>,
    pub wind: Option<f64>,
    pub wdir: Option<f64>,
    /// Effective day flag for the hour (see [`HourlySeries::window`]).
    pub is_day: Option<f64>,
}

impl HourlySeries {
    pub fn with_capacity(n: usize) -> Self {
        HourlySeries {
            time: Vec::with_capacity(n),
            sunshine: Vec::with_capacity(n),
            precip_prob: Vec::with_capacity(n),
            precip: Vec::with_capacity(n),
            temp: Vec::with_capacity(n),
            wind: Vec::with_capacity(n),
            wdir: Vec::with_capacity(n),
            is_day: Vec::with_capacity(n),
        }
    }

    fn index(&self, t: i64) -> Option<usize> {
        self.time.binary_search(&t).ok()
    }

    /// Sample for the output hour starting at `t` (t..t+1h).
    ///
    /// - sunshine, precipitation and its probability come from timestamp t+1h
    ///   (Open-Meteo sums over the preceding hour); if t+1h is missing (end of
    ///   data) the value at t is reused.
    /// - temperature and wind are instantaneous at t.
    /// - The hour counts as day if `is_day(t) == 1`, or if `is_day(t+1h) == 1`
    ///   and there is sunshine in the hour (sunrise hour).
    pub fn window(&self, t: i64) -> Option<HourSample> {
        let i = self.index(t)?;
        let j = self.index(t + 3600).unwrap_or(i);
        let sunshine = self.sunshine[j];
        let day_now = self.is_day[i];
        let day_next = self.is_day[j];
        let is_day = match (day_now, day_next) {
            (Some(a), _) if a >= 0.5 => Some(1.0),
            (_, Some(b)) if b >= 0.5 && sunshine.unwrap_or(0.0) > 0.0 => Some(1.0),
            (None, None) => None,
            _ => Some(0.0),
        };
        Some(HourSample {
            sunshine,
            precip_prob: self.precip_prob[j],
            precip: self.precip[j],
            temp: self.temp[i],
            wind: self.wind[i],
            wdir: self.wdir[i],
            is_day,
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PointWeather {
    pub hourly: HourlySeries,
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
}

#[derive(Deserialize)]
struct RawHourly {
    time: Vec<TimeVal>,
    #[serde(default)]
    sunshine_duration: Vec<Option<f64>>,
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

impl RawLocation {
    fn into_point(self) -> Result<PointWeather> {
        let hourly = match self.hourly {
            Some(h) => {
                let n = h.time.len();
                HourlySeries {
                    time: times_to_unix(&h.time)?,
                    sunshine: fit(h.sunshine_duration, n),
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
        Ok(PointWeather { hourly })
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
        "{base}?latitude={}&longitude={}&hourly={HOURLY_VARS}\
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
    /// Pause between batches (rate limit: 600 calls/min, each location = 1 call).
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
    /// 2026-11-07T00:00+01:00
    const T0: i64 = 1_794_006_000;
    const H: i64 = 3600;

    #[test]
    fn parse_multi_location_unixtime() {
        let pts = parse_response(MULTI).unwrap();
        assert_eq!(pts.len(), 2);
        let h = &pts[0].hourly;
        assert_eq!(h.time, vec![T0, T0 + H, T0 + 10 * H, T0 + 11 * H, T0 + 12 * H]);
        assert_eq!(h.sunshine[2], Some(3600.0));
        // Null values become None.
        assert_eq!(h.temp[1], None);
        assert_eq!(pts[1].hourly.precip_prob[0], Some(80.0));
    }

    #[test]
    fn window_shifts_period_values() {
        let pts = parse_response(MULTI).unwrap();
        let h = &pts[0].hourly;
        // Hour 10..11: sunshine/precip from 11:00, temp/wind from 10:00.
        let s = h.window(T0 + 10 * H).unwrap();
        assert_eq!(s.sunshine, Some(2700.0));
        assert_eq!(s.precip_prob, Some(5.0));
        assert_eq!(s.temp, Some(9.8));
        assert_eq!(s.wdir, Some(230.0));
        assert_eq!(s.is_day, Some(1.0));
        // Last hour (no 13:00): own values reused.
        let s = h.window(T0 + 12 * H).unwrap();
        assert_eq!(s.sunshine, Some(1800.0));
        // Night.
        assert_eq!(h.window(T0).unwrap().is_day, Some(0.0));
        // Hour missing entirely.
        assert!(h.window(T0 + 5 * H).is_none());
    }

    fn two_hours(is_day: [f64; 2], sun_next: f64) -> HourlySeries {
        HourlySeries {
            time: vec![T0, T0 + H],
            sunshine: vec![Some(0.0), Some(sun_next)],
            precip_prob: vec![Some(0.0); 2],
            precip: vec![Some(0.0); 2],
            temp: vec![Some(5.0); 2],
            wind: vec![Some(5.0); 2],
            wdir: vec![Some(180.0); 2],
            is_day: vec![Some(is_day[0]), Some(is_day[1])],
        }
    }

    #[test]
    fn sunrise_hour_counts_as_day_only_with_sun() {
        assert_eq!(two_hours([0.0, 1.0], 900.0).window(T0).unwrap().is_day, Some(1.0));
        assert_eq!(two_hours([0.0, 1.0], 0.0).window(T0).unwrap().is_day, Some(0.0));
        // Sunset hour: day at h, night at h+1 -> day.
        assert_eq!(two_hours([1.0, 0.0], 0.0).window(T0).unwrap().is_day, Some(1.0));
    }

    #[test]
    fn parse_single_location_iso_times() {
        let pts = parse_response(SINGLE_ISO).unwrap();
        assert_eq!(pts.len(), 1);
        let h = &pts[0].hourly;
        assert_eq!(crate::time::fmt_local(h.time[0]), "2026-11-07T00:00+01:00");
        assert_eq!(crate::time::fmt_local(h.time[2]), "2026-11-07T02:00+01:00");
        // Missing variable arrays are padded with None.
        assert_eq!(h.wdir, vec![None, None, None]);
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
        assert!(!u.contains("daily="));
        assert!(!u.contains("cloud_cover"));
        // ≤ 10 variables -> one API call per location.
        assert_eq!(HOURLY_VARS.split(',').count(), 7);
    }
}
