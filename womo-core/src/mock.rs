//! Deterministic synthetic weather for UI development and CI previews.
//!
//! Scenario: a cold front moves NW → SE across the area (rain band, clearing
//! behind it), a second front follows a few days later, and the Danube valley
//! has morning fog under high pressure. Day/night uses the real sun position.

use crate::aggregate::wind_dir_mean;
use crate::openmeteo::{DailySeries, HourlySeries, PointWeather};
use crate::regions::SamplePoint;
use crate::sun;
use crate::time::{to_local, TimeAxis, DAYS};
use chrono::Timelike;

/// Front speed in "front units" (≈ degrees, see `front_coord`) per hour.
const FRONT_SPEED: f64 = 0.38;
/// Distance between the two fronts in front units.
const FRONT_GAP: f64 = 30.0;
/// Half width of the rain band.
const BAND: f64 = 1.2;

/// Danube course (lon, lat) from Donaueschingen to Passau.
const DANUBE: [(f64, f64); 8] = [
    (8.50, 47.95),
    (9.20, 48.10),
    (10.00, 48.40),
    (10.90, 48.70),
    (11.40, 48.76),
    (12.10, 49.00),
    (12.60, 48.90),
    (13.45, 48.57),
];

/// Coordinate increasing from NW to SE.
fn front_coord(lon: f64, lat: f64) -> f64 {
    (lon - 2.0) * 0.63 + (55.3 - lat)
}

/// Smooth deterministic pseudo noise in [-1, 1].
fn noise(lon: f64, lat: f64, th: f64) -> f64 {
    let a = (lon * 2.1 + th * 0.23).sin() * (lat * 2.7 - th * 0.17).cos();
    let b = (lon * 5.3 + lat * 4.1 + th * 0.5).sin();
    0.6 * a + 0.4 * b
}

/// Distance (approx. degrees of latitude) to the Danube polyline.
fn danube_dist(lon: f64, lat: f64) -> f64 {
    let k = lat.to_radians().cos();
    let (px, py) = (lon * k, lat);
    DANUBE
        .windows(2)
        .map(|w| {
            let (ax, ay) = (w[0].0 * k, w[0].1);
            let (bx, by) = (w[1].0 * k, w[1].1);
            let (dx, dy) = (bx - ax, by - ay);
            let t = (((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
            (px - ax - t * dx).hypot(py - ay - t * dy)
        })
        .fold(f64::INFINITY, f64::min)
}

struct Hour {
    sunshine: f64,
    cloud: f64,
    pp: f64,
    precip: f64,
    temp: f64,
    wind: f64,
    wdir: f64,
    is_day: bool,
}

fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

fn hour_weather(lon: f64, lat: f64, t: i64, th: f64, fog_dist: f64) -> Hour {
    let s = front_coord(lon, lat);
    let f1 = -2.0 + FRONT_SPEED * th;
    let d1 = s - f1;
    // Far behind front 1, the second front takes over.
    let d = if d1 < -6.0 { d1 + FRONT_GAP } else { d1 };
    let n = noise(lon, lat, th);
    let local_h = to_local(t).hour() as f64;

    let (mut cloud, mut pp, mut precip) = if d > BAND {
        // Warm sector: clouds thicken towards the front.
        (20.0 + 55.0 * (-(d - BAND) / 3.0).exp(), 10.0, 0.0)
    } else if d >= -BAND {
        let core = 1.0 - d.abs() / BAND;
        (97.0, 55.0 + 35.0 * core, 0.3 + 2.2 * core)
    } else {
        // Clearing behind the front, a few showers right behind it.
        let x = ((d + BAND) / 1.5).exp();
        (12.0 + 63.0 * x, 10.0 + 30.0 * x, 0.15 * x)
    };
    cloud = (cloud + 14.0 * n).clamp(0.0, 100.0);

    // Morning fog in the Danube valley under calm, mostly clear conditions.
    let fog = fog_dist < 0.2 && cloud < 60.0 && pp < 30.0;
    if fog && (5.0..=10.0).contains(&local_h) {
        cloud = 100.0;
    } else if fog && local_h == 11.0 {
        cloud = cloud.max(70.0);
    }

    // Sunshine in the preceding hour.
    let subs = [-52.5, -37.5, -22.5, -7.5];
    let elevs: Vec<f64> = subs
        .iter()
        .map(|m| sun::elevation(t + (m * 60.0) as i64, lat, lon))
        .collect();
    let frac = elevs.iter().filter(|&&e| e > 0.0).count() as f64 / subs.len() as f64;
    let mean_elev = elevs.iter().sum::<f64>() / elevs.len() as f64;
    let low_sun = (mean_elev / 6.0).clamp(0.3, 1.0);
    let factor = (1.0 - cloud / 100.0 * 1.05).clamp(0.0, 1.0) * low_sun;
    let sunshine = (3600.0 * frac * factor).round();

    if precip < 0.05 {
        precip = 0.0;
    }
    pp = pp.clamp(0.0, 100.0).round();

    let behind = sigmoid(-d * 1.5);
    let diurnal = 3.5 * (std::f64::consts::PI * (local_h - 9.0) / 12.0).sin();
    let temp = 10.0 - (lat - 50.0) * 0.9 + diurnal - 3.0 * behind + n;
    let wdir = (200.0 + 100.0 * behind + 10.0 * n).rem_euclid(360.0);
    let wind = (11.0 + 17.0 * (-d * d / 2.0).exp() + 6.0 * behind + 3.0 * n).max(0.0);

    Hour {
        sunshine,
        cloud: cloud.round(),
        pp,
        precip: (precip * 10.0).round() / 10.0,
        temp: (temp * 10.0).round() / 10.0,
        wind: (wind * 10.0).round() / 10.0,
        wdir: wdir.round(),
        is_day: sun::elevation(t, lat, lon) > sun::HORIZON_ELEVATION_DEG,
    }
}

/// Synthetic weather for one location over the full 7-day axis.
pub fn point_weather(lon: f64, lat: f64, axis: &TimeAxis) -> PointWeather {
    let start = axis.hours[0];
    let n = DAYS * 24 + 1;
    let fog_dist = danube_dist(lon, lat);
    let mut h = HourlySeries::with_capacity(n);
    for k in 0..n {
        let t = start + k as i64 * 3600;
        let w = hour_weather(lon, lat, t, k as f64, fog_dist);
        h.time.push(t);
        h.sunshine.push(Some(w.sunshine));
        h.cloud_cover.push(Some(w.cloud));
        h.precip_prob.push(Some(w.pp));
        h.precip.push(Some(w.precip));
        h.temp.push(Some(w.temp));
        h.wind.push(Some(w.wind));
        h.wdir.push(Some(w.wdir));
        h.is_day.push(Some(if w.is_day { 1.0 } else { 0.0 }));
    }

    let mut d = DailySeries::default();
    for &date in &axis.days {
        // Hours belonging to this local day (sunshine of the preceding hour → shift by 1).
        let idx: Vec<usize> = (0..n)
            .filter(|&i| to_local(h.time[i] - 3600).date_naive() == date)
            .collect();
        let vals =
            |v: &Vec<Option<f64>>| -> Vec<f64> { idx.iter().filter_map(|&i| v[i]).collect() };
        let max = |v: Vec<f64>| v.into_iter().reduce(f64::max);
        let min = |v: Vec<f64>| v.into_iter().reduce(f64::min);
        let ss = sun::sunrise_sunset(date, lat, lon);
        d.date.push(date);
        d.sunrise.push(ss.map(|x| x.0));
        d.sunset.push(ss.map(|x| x.1));
        d.sunshine.push(Some(vals(&h.sunshine).iter().sum()));
        d.tmax.push(max(vals(&h.temp)));
        d.tmin.push(min(vals(&h.temp)));
        d.precip_prob_max.push(max(vals(&h.precip_prob)));
        d.wind_max.push(max(vals(&h.wind)));
        d.wdir_dominant.push(
            wind_dir_mean(
                idx.iter()
                    .filter_map(|&i| h.wdir[i].map(|w| (h.wind[i], w))),
            )
            .map(f64::round),
        );
    }
    PointWeather {
        hourly: h,
        daily: d,
    }
}

/// Synthetic weather for all sample points (same order).
pub fn generate(points: &[SamplePoint], axis: &TimeAxis) -> Vec<PointWeather> {
    points
        .iter()
        .map(|p| point_weather(p.lon, p.lat, axis))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::DateTime;

    fn axis() -> TimeAxis {
        TimeAxis::new(
            DateTime::parse_from_rfc3339("2026-11-07T09:12:00+01:00")
                .unwrap()
                .to_utc(),
        )
    }

    #[test]
    fn deterministic() {
        let a = axis();
        assert_eq!(point_weather(6.7, 50.3, &a), point_weather(6.7, 50.3, &a));
    }

    #[test]
    fn nights_have_no_sun() {
        let a = axis();
        let w = point_weather(6.96, 50.94, &a);
        // 02:00 local and 22:00 local.
        for h in [2, 22] {
            let s = w.hourly.at(a.hours[h]).unwrap();
            assert_eq!(s.is_day, Some(0.0));
            assert_eq!(s.sunshine, Some(0.0));
        }
        let s = w.hourly.at(a.hours[12]).unwrap();
        assert_eq!(s.is_day, Some(1.0));
    }

    #[test]
    fn danube_morning_fog() {
        let a = axis();
        // Regensburg vs. a point 1° north of it, day 3 (high pressure behind front).
        let fog = point_weather(12.1, 49.0, &a);
        let t = a.hours[0] + (48 + 9) * 3600;
        let s = fog.hourly.at(t).unwrap();
        assert_eq!(s.sunshine, Some(0.0), "fog expected in the morning");
        assert!(danube_dist(12.1, 50.0) > 0.5);
    }

    #[test]
    fn front_moves_southeast() {
        let a = axis();
        // NW (Groningen) gets rain earlier than SE (Passau).
        let rainy_hour = |lon: f64, lat: f64| {
            let w = point_weather(lon, lat, &a);
            (0..48).find(|&h| {
                let s = w.hourly.at(a.hours[h]).unwrap();
                s.precip_prob.unwrap() >= 50.0 && s.precip.unwrap() >= 0.2
            })
        };
        let nw = rainy_hour(6.5, 53.2).unwrap();
        let se = rainy_hour(13.4, 48.6).unwrap();
        assert!(nw < se, "nw {nw} se {se}");
    }

    #[test]
    fn daily_has_sunrise_and_sun_hours() {
        let a = axis();
        let w = point_weather(6.96, 50.94, &a);
        assert_eq!(w.daily.date.len(), 7);
        let d = w.daily.at(a.days[0]).unwrap();
        assert_eq!(crate::time::fmt_hm(d.sunset.unwrap()).len(), 5);
        let sh = d.sunshine.unwrap() / 3600.0;
        assert!((0.0..=10.0).contains(&sh), "{sh}");
    }
}
