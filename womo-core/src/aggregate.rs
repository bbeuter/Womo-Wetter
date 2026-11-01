//! Region aggregation and classification (DATENFORMAT §2 / §3).

use crate::export::{RegionDaily, RegionHourly, RegionOut};
use crate::openmeteo::{HourSample, PointWeather};
use crate::sun;
use crate::time::{fmt_hm, TimeAxis};

/// Sunshine share of one hour in percent (0–100).
pub fn sun_pct(sunshine_s: f64) -> f64 {
    (sunshine_s / 3600.0 * 100.0).clamp(0.0, 100.0)
}

/// True if rain rule applies (probability ≥ 50 % and precipitation ≥ 0.2 mm).
pub fn is_rain(prob: f64, precip_mm: f64) -> bool {
    prob >= 50.0 && precip_mm >= 0.2
}

/// Hour class from sun percentage (no night/rain).
pub fn sun_class(sun: f64) -> char {
    if sun >= 75.0 {
        'S'
    } else if sun >= 50.0 {
        'H'
    } else if sun >= 25.0 {
        'W'
    } else {
        'B'
    }
}

/// Full hour class: N > R > S/H/W/B.
pub fn hour_class(night: bool, rain_prob: f64, precip_mm: f64, sun: f64) -> char {
    if night {
        'N'
    } else if is_rain(rain_prob, precip_mm) {
        'R'
    } else {
        sun_class(sun)
    }
}

/// Day class from sunshine hours relative to day length and max rain probability.
pub fn day_class(sun_h: f64, daylen_h: f64, rain: f64) -> char {
    if rain >= 60.0 && sun_h < 2.0 {
        return 'R';
    }
    let ratio = if daylen_h > 0.0 { sun_h / daylen_h } else { 0.0 };
    if ratio >= 0.6 {
        'S'
    } else if ratio >= 0.4 {
        'H'
    } else if ratio >= 0.2 {
        'W'
    } else {
        'B'
    }
}

/// Vector mean of wind directions (deg), weighted by speed.
/// Falls back to unit vectors if all speeds are zero/missing; `None` if undefined.
pub fn wind_dir_mean<I>(samples: I) -> Option<f64>
where
    I: IntoIterator<Item = (Option<f64>, f64)>,
{
    let (mut ws, mut wc, mut us, mut uc, mut n) = (0.0, 0.0, 0.0, 0.0, 0);
    for (speed, dir) in samples {
        let r = dir.to_radians();
        let w = speed.unwrap_or(0.0).max(0.0);
        ws += w * r.sin();
        wc += w * r.cos();
        us += r.sin();
        uc += r.cos();
        n += 1;
    }
    if n == 0 {
        return None;
    }
    let (s, c) = if ws.hypot(wc) > 1e-9 { (ws, wc) } else { (us, uc) };
    if s.hypot(c) < 1e-9 {
        return None;
    }
    Some(s.atan2(c).to_degrees().rem_euclid(360.0))
}

/// Grid character for one point and hour: 'N', 'R', '0'..'9' or '.'.
pub fn grid_char(s: Option<HourSample>) -> char {
    let Some(s) = s else { return '.' };
    if s.is_day == Some(0.0) {
        return 'N';
    }
    if is_rain(s.precip_prob.unwrap_or(0.0), s.precip.unwrap_or(0.0)) {
        return 'R';
    }
    match s.sunshine {
        Some(v) => {
            let d = (sun_pct(v) / 10.0).floor().min(9.0) as u8;
            (b'0' + d) as char
        }
        None => '.',
    }
}

fn mean<I: IntoIterator<Item = f64>>(it: I) -> Option<f64> {
    let (mut s, mut n) = (0.0, 0usize);
    for v in it {
        s += v;
        n += 1;
    }
    (n > 0).then(|| s / n as f64)
}

fn round_i(v: f64) -> i32 {
    v.round() as i32
}

/// Fills gaps forward, then backward; all-missing becomes 0.
fn fill(v: Vec<Option<i32>>) -> Vec<i32> {
    let first = v.iter().flatten().next().copied().unwrap_or(0);
    let mut last = first;
    v.into_iter()
        .map(|x| {
            if let Some(x) = x {
                last = x;
            }
            last
        })
        .collect()
}

/// Aggregates the weather of all sample points of one region.
///
/// `nearest` is the index (into `points`) of the point closest to the region
/// center; its daily sunrise/sunset is used. `center` is (lon, lat) for the
/// computed fallback.
pub fn aggregate_region(
    points: &[&PointWeather],
    nearest: usize,
    center: (f64, f64),
    axis: &TimeAxis,
) -> RegionOut {
    let nh = axis.hours.len();
    let mut cls = String::with_capacity(nh);
    let mut sun = Vec::with_capacity(nh);
    let mut sunny = Vec::with_capacity(nh);
    let mut rain = Vec::with_capacity(nh);
    let mut temp = Vec::with_capacity(nh);
    let mut wind = Vec::with_capacity(nh);
    let mut wdir = Vec::with_capacity(nh);

    for &t in &axis.hours {
        let samples: Vec<HourSample> = points.iter().filter_map(|p| p.hourly.at(t)).collect();
        let sun_vals: Vec<f64> = samples.iter().filter_map(|s| s.sunshine).map(sun_pct).collect();
        let sun_m = mean(sun_vals.iter().copied()).map(round_i);
        let sunny_m = (!sun_vals.is_empty()).then(|| {
            let k = sun_vals.iter().filter(|&&v| v >= 50.0).count();
            round_i(k as f64 * 100.0 / sun_vals.len() as f64)
        });
        let rain_m = mean(samples.iter().filter_map(|s| s.precip_prob)).map(round_i);
        let precip_m = mean(samples.iter().filter_map(|s| s.precip)).unwrap_or(0.0);
        let day_flags: Vec<f64> = samples.iter().filter_map(|s| s.is_day).collect();
        let nights = day_flags.iter().filter(|&&d| d < 0.5).count();
        let night = nights * 2 > day_flags.len();

        let c = if samples.is_empty() {
            // No data at all: decide night from the computed sun position.
            if sun::elevation(t, center.1, center.0) < sun::HORIZON_ELEVATION_DEG {
                'N'
            } else {
                'B'
            }
        } else {
            hour_class(
                night,
                rain_m.unwrap_or(0) as f64,
                precip_m,
                sun_m.unwrap_or(0) as f64,
            )
        };
        cls.push(c);
        sun.push(sun_m);
        sunny.push(sunny_m);
        rain.push(rain_m);
        temp.push(mean(samples.iter().filter_map(|s| s.temp)).map(round_i));
        wind.push(mean(samples.iter().filter_map(|s| s.wind)).map(round_i));
        wdir.push(
            wind_dir_mean(samples.iter().filter_map(|s| s.wdir.map(|d| (s.wind, d))))
                .map(|d| round_i(d) % 360),
        );
    }

    let hourly = RegionHourly {
        cls,
        sun: fill(sun),
        sunny: fill(sunny),
        rain: fill(rain),
        temp: fill(temp),
        wind: fill(wind),
        wdir: fill(wdir),
    };

    let daily = axis
        .days
        .iter()
        .map(|&date| {
            let ds: Vec<_> = points.iter().filter_map(|p| p.daily.at(date)).collect();
            let near = points.get(nearest).and_then(|p| p.daily.at(date));
            let computed = sun::sunrise_sunset(date, center.1, center.0);
            let rise = near
                .and_then(|d| d.sunrise)
                .or(computed.map(|c| c.0));
            let set = near.and_then(|d| d.sunset).or(computed.map(|c| c.1));
            let daylen_h = match (rise, set) {
                (Some(r), Some(s)) if s > r => (s - r) as f64 / 3600.0,
                _ => 0.0,
            };
            let sun_h_raw = mean(ds.iter().filter_map(|d| d.sunshine)).unwrap_or(0.0) / 3600.0;
            let sun_h = (sun_h_raw * 10.0).round() / 10.0;
            let rain = round_i(mean(ds.iter().filter_map(|d| d.precip_prob_max)).unwrap_or(0.0));
            let wd = wind_dir_mean(
                ds.iter()
                    .filter_map(|d| d.wdir_dominant.map(|w| (d.wind_max, w))),
            );
            RegionDaily {
                date: date.format("%Y-%m-%d").to_string(),
                sunrise: rise.map(fmt_hm),
                sunset: set.map(fmt_hm),
                sun_h,
                tmax: round_i(mean(ds.iter().filter_map(|d| d.tmax)).unwrap_or(0.0)),
                tmin: round_i(mean(ds.iter().filter_map(|d| d.tmin)).unwrap_or(0.0)),
                rain,
                wind: round_i(mean(ds.iter().filter_map(|d| d.wind_max)).unwrap_or(0.0)),
                wdir: wd.map(|d| round_i(d) % 360).unwrap_or(0),
                cls: day_class(sun_h_raw, daylen_h, rain as f64).to_string(),
            }
        })
        .collect();

    RegionOut { hourly, daily }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openmeteo::parse_response;
    use chrono::DateTime;

    #[test]
    fn hour_class_thresholds() {
        assert_eq!(hour_class(false, 0.0, 0.0, 100.0), 'S');
        assert_eq!(hour_class(false, 0.0, 0.0, 75.0), 'S');
        assert_eq!(hour_class(false, 0.0, 0.0, 74.9), 'H');
        assert_eq!(hour_class(false, 0.0, 0.0, 50.0), 'H');
        assert_eq!(hour_class(false, 0.0, 0.0, 49.0), 'W');
        assert_eq!(hour_class(false, 0.0, 0.0, 25.0), 'W');
        assert_eq!(hour_class(false, 0.0, 0.0, 24.0), 'B');
        assert_eq!(hour_class(false, 0.0, 0.0, 0.0), 'B');
        // Rain needs both probability and amount.
        assert_eq!(hour_class(false, 50.0, 0.2, 80.0), 'R');
        assert_eq!(hour_class(false, 49.0, 5.0, 80.0), 'S');
        assert_eq!(hour_class(false, 90.0, 0.1, 10.0), 'B');
        // Night wins.
        assert_eq!(hour_class(true, 90.0, 3.0, 0.0), 'N');
    }

    #[test]
    fn day_class_thresholds() {
        assert_eq!(day_class(6.0, 10.0, 0.0), 'S');
        assert_eq!(day_class(5.9, 10.0, 0.0), 'H');
        assert_eq!(day_class(4.0, 10.0, 0.0), 'H');
        assert_eq!(day_class(3.9, 10.0, 0.0), 'W');
        assert_eq!(day_class(2.0, 10.0, 0.0), 'W');
        assert_eq!(day_class(1.9, 10.0, 0.0), 'B');
        assert_eq!(day_class(1.9, 10.0, 60.0), 'R');
        assert_eq!(day_class(2.0, 10.0, 90.0), 'W');
        assert_eq!(day_class(1.0, 10.0, 59.0), 'B');
    }

    #[test]
    fn wind_vector_mean() {
        let m = wind_dir_mean([(Some(10.0), 350.0), (Some(10.0), 10.0)]).unwrap();
        assert!(m < 0.01 || m > 359.99, "{m}");
        let m = wind_dir_mean([(Some(5.0), 90.0), (Some(5.0), 180.0)]).unwrap();
        assert!((m - 135.0).abs() < 1e-9);
        // Speed weighting: strong west wind dominates weak east wind.
        let m = wind_dir_mean([(Some(30.0), 270.0), (Some(5.0), 90.0)]).unwrap();
        assert!((m - 270.0).abs() < 1e-9);
        // Calm: unit-vector fallback.
        let m = wind_dir_mean([(Some(0.0), 200.0), (None, 220.0)]).unwrap();
        assert!((m - 210.0).abs() < 1e-9);
        // Opposite directions cancel.
        assert!(wind_dir_mean([(Some(5.0), 0.0), (Some(5.0), 180.0)]).is_none());
        assert!(wind_dir_mean(std::iter::empty()).is_none());
    }

    #[test]
    fn grid_chars() {
        let base = HourSample {
            sunshine: Some(3600.0),
            precip_prob: Some(0.0),
            precip: Some(0.0),
            is_day: Some(1.0),
            ..Default::default()
        };
        assert_eq!(grid_char(Some(base)), '9');
        assert_eq!(grid_char(Some(HourSample { sunshine: Some(1800.0), ..base })), '5');
        assert_eq!(grid_char(Some(HourSample { sunshine: Some(359.0), ..base })), '0');
        assert_eq!(grid_char(Some(HourSample { sunshine: Some(3239.0), ..base })), '8');
        assert_eq!(
            grid_char(Some(HourSample { precip_prob: Some(70.0), precip: Some(1.0), ..base })),
            'R'
        );
        assert_eq!(grid_char(Some(HourSample { is_day: Some(0.0), ..base })), 'N');
        assert_eq!(grid_char(Some(HourSample { sunshine: None, ..base })), '.');
        assert_eq!(grid_char(None), '.');
    }

    #[test]
    fn fill_gaps() {
        assert_eq!(fill(vec![None, Some(3), None, Some(5)]), vec![3, 3, 3, 5]);
        assert_eq!(fill(vec![None, None]), vec![0, 0]);
    }

    #[test]
    fn aggregate_fixture_region() {
        let pts = parse_response(include_str!("../tests/fixtures/openmeteo_multi.json")).unwrap();
        let refs: Vec<&PointWeather> = pts.iter().collect();
        let now = DateTime::parse_from_rfc3339("2026-11-07T09:12:00+01:00")
            .unwrap()
            .to_utc();
        let axis = TimeAxis::new(now);
        let out = aggregate_region(&refs, 0, (6.6, 50.25), &axis);
        let h = &out.hourly;
        assert_eq!(h.cls.len(), 48);
        // 00:00 both night.
        assert_eq!(&h.cls[0..2], "NN");
        // 10:00: sun (100 + 16.7)/2 = 58 -> H; rain mean 35 -> no R.
        assert_eq!(h.sun[10], 58);
        assert_eq!(h.sunny[10], 50);
        assert_eq!(h.rain[10], 35);
        assert_eq!(h.cls.as_bytes()[10], b'H');
        // 11:00: rain mean 47.5 -> 48 < 50, sun (75+0)/2 -> 38 -> W.
        assert_eq!(h.cls.as_bytes()[11], b'W');
        // 01:00 temp only from second point (first is null).
        assert_eq!(h.temp[1], 5);
        // Speed weighted wdir at 10:00 lies between 230 and 260, nearer 260.
        assert!(h.wdir[10] > 245 && h.wdir[10] < 260, "{}", h.wdir[10]);
        // Gaps after 11:00 carried forward.
        assert_eq!(h.temp[47], h.temp[11]);

        let d0 = &out.daily[0];
        assert_eq!(d0.date, "2026-11-07");
        assert_eq!(d0.sunrise.as_deref(), Some("07:32"));
        assert_eq!(d0.sunset.as_deref(), Some("16:59"));
        // (24120 + 1800) / 2 / 3600 = 3.6 h of 9.45 h daylight = 38 % -> W.
        assert_eq!(d0.sun_h, 3.6);
        assert_eq!(d0.cls, "W");
        assert_eq!(d0.rain, 53);
        let d1 = &out.daily[1];
        assert_eq!(d1.cls, "R");
        assert_eq!(d1.tmax, 9);
        // Days without data: computed sunrise/sunset fallback, zero sun.
        let d6 = &out.daily[6];
        assert!(d6.sunrise.is_some() && d6.sunset.is_some());
        assert_eq!(d6.sun_h, 0.0);
    }
}
