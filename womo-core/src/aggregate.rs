//! Region aggregation and classification (DATENFORMAT §2 / §3).

use crate::export::{RegionDaily, RegionHourly, RegionOut};
use crate::openmeteo::{HourSample, PointWeather};
use crate::sun;
use crate::time::{fmt_hm, local_midnight, to_local, TimeAxis};
use chrono::NaiveDate;

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
    let ratio = if daylen_h > 0.0 {
        sun_h / daylen_h
    } else {
        0.0
    };
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
    let (s, c) = if ws.hypot(wc) > 1e-9 {
        (ws, wc)
    } else {
        (us, uc)
    };
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

/// Region mean of all points for one output hour (unrounded).
#[derive(Debug, Clone, Copy, Default)]
struct HourAgg {
    /// Mean sunshine share in % (0–100).
    sun: Option<f64>,
    /// Share of points with sun ≥ 50 %, in %.
    sunny: Option<f64>,
    rain: Option<f64>,
    precip: f64,
    temp: Option<f64>,
    wind: Option<f64>,
    wdir: Option<f64>,
    night: bool,
    /// No point has data for this hour.
    empty: bool,
}

fn region_hour(points: &[&PointWeather], t: i64, center: (f64, f64)) -> HourAgg {
    let samples: Vec<HourSample> = points.iter().filter_map(|p| p.hourly.window(t)).collect();
    if samples.is_empty() {
        // No data: decide night from the computed sun position.
        let night = sun::elevation(t + 1800, center.1, center.0) < sun::HORIZON_ELEVATION_DEG;
        return HourAgg {
            night,
            empty: true,
            ..Default::default()
        };
    }
    let sun_vals: Vec<f64> = samples
        .iter()
        .filter_map(|s| s.sunshine)
        .map(sun_pct)
        .collect();
    let flags: Vec<f64> = samples.iter().filter_map(|s| s.is_day).collect();
    let nights = flags.iter().filter(|&&d| d < 0.5).count();
    HourAgg {
        sun: mean(sun_vals.iter().copied()),
        sunny: (!sun_vals.is_empty()).then(|| {
            let k = sun_vals.iter().filter(|&&v| v >= 50.0).count();
            k as f64 * 100.0 / sun_vals.len() as f64
        }),
        rain: mean(samples.iter().filter_map(|s| s.precip_prob)),
        precip: mean(samples.iter().filter_map(|s| s.precip)).unwrap_or(0.0),
        temp: mean(samples.iter().filter_map(|s| s.temp)),
        wind: mean(samples.iter().filter_map(|s| s.wind)),
        wdir: wind_dir_mean(samples.iter().filter_map(|s| s.wdir.map(|d| (s.wind, d)))),
        night: nights * 2 > flags.len(),
        empty: false,
    }
}

impl HourAgg {
    /// Hour class from the rounded output values.
    fn class(&self) -> char {
        if self.empty {
            return if self.night { 'N' } else { 'B' };
        }
        hour_class(
            self.night,
            self.rain.map(f64::round).unwrap_or(0.0),
            self.precip,
            self.sun.map(f64::round).unwrap_or(0.0),
        )
    }
}

fn max_of<I: IntoIterator<Item = f64>>(it: I) -> Option<f64> {
    it.into_iter().reduce(f64::max)
}

fn min_of<I: IntoIterator<Item = f64>>(it: I) -> Option<f64> {
    it.into_iter().reduce(f64::min)
}

/// Daily values derived from the region-mean hourly data of one local day.
fn region_day(points: &[&PointWeather], date: NaiveDate, center: (f64, f64)) -> RegionDaily {
    let start = local_midnight(date);
    let hours: Vec<HourAgg> = (0..25)
        .map(|k| start + k * 3600)
        .filter(|&t| to_local(t).date_naive() == date)
        .map(|t| region_hour(points, t, center))
        .collect();
    let data: Vec<&HourAgg> = hours.iter().filter(|h| !h.empty).collect();
    let day: Vec<&HourAgg> = data.iter().copied().filter(|h| !h.night).collect();
    // Daylight hours, or all hours if none qualify.
    let day_or_all = if day.is_empty() { &data } else { &day };

    let sun_h_raw = data.iter().filter_map(|h| h.sun).sum::<f64>() / 100.0;
    let sun_h = (sun_h_raw * 10.0).round() / 10.0;
    let rain = round_i(max_of(day_or_all.iter().filter_map(|h| h.rain)).unwrap_or(0.0));
    let wdir = wind_dir_mean(
        day_or_all
            .iter()
            .filter_map(|h| h.wdir.map(|d| (h.wind, d))),
    );

    let ss = sun::sunrise_sunset(date, center.1, center.0);
    let daylen_h = ss.map(|(r, s)| (s - r) as f64 / 3600.0).unwrap_or(0.0);
    RegionDaily {
        date: date.format("%Y-%m-%d").to_string(),
        sunrise: ss.map(|x| fmt_hm(x.0)),
        sunset: ss.map(|x| fmt_hm(x.1)),
        sun_h,
        tmax: round_i(max_of(data.iter().filter_map(|h| h.temp)).unwrap_or(0.0)),
        tmin: round_i(min_of(data.iter().filter_map(|h| h.temp)).unwrap_or(0.0)),
        rain,
        wind: round_i(max_of(data.iter().filter_map(|h| h.wind)).unwrap_or(0.0)),
        wdir: wdir.map(|d| round_i(d) % 360).unwrap_or(0),
        cls: day_class(sun_h_raw, daylen_h, rain as f64).to_string(),
    }
}

/// Aggregates the weather of all sample points of one region.
/// `center` is (lon, lat); sunrise/sunset are computed there.
pub fn aggregate_region(
    points: &[&PointWeather],
    center: (f64, f64),
    axis: &TimeAxis,
) -> RegionOut {
    let aggs: Vec<HourAgg> = axis
        .hours
        .iter()
        .map(|&t| region_hour(points, t, center))
        .collect();
    let col = |f: &dyn Fn(&HourAgg) -> Option<f64>| -> Vec<i32> {
        fill(aggs.iter().map(|h| f(h).map(round_i)).collect())
    };
    let hourly = RegionHourly {
        cls: aggs.iter().map(HourAgg::class).collect(),
        sun: col(&|h| h.sun),
        sunny: col(&|h| h.sunny),
        rain: col(&|h| h.rain),
        temp: col(&|h| h.temp),
        wind: col(&|h| h.wind),
        wdir: fill(
            aggs.iter()
                .map(|h| h.wdir.map(|d| round_i(d) % 360))
                .collect(),
        ),
    };
    let daily = axis
        .days
        .iter()
        .map(|&d| region_day(points, d, center))
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
        assert!(!(0.01..=359.99).contains(&m), "{m}");
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
        assert_eq!(
            grid_char(Some(HourSample {
                sunshine: Some(1800.0),
                ..base
            })),
            '5'
        );
        assert_eq!(
            grid_char(Some(HourSample {
                sunshine: Some(359.0),
                ..base
            })),
            '0'
        );
        assert_eq!(
            grid_char(Some(HourSample {
                sunshine: Some(3239.0),
                ..base
            })),
            '8'
        );
        assert_eq!(
            grid_char(Some(HourSample {
                precip_prob: Some(70.0),
                precip: Some(1.0),
                ..base
            })),
            'R'
        );
        assert_eq!(
            grid_char(Some(HourSample {
                is_day: Some(0.0),
                ..base
            })),
            'N'
        );
        assert_eq!(
            grid_char(Some(HourSample {
                sunshine: None,
                ..base
            })),
            '.'
        );
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
        let out = aggregate_region(&refs, (6.6, 50.25), &axis);
        let h = &out.hourly;
        assert_eq!(h.cls.len(), 48);
        assert_eq!(&h.cls[0..2], "NN");
        // Hour 10..11 uses sunshine/rain at 11:00: sun (75 + 0)/2 -> 38,
        // rain (5 + 90)/2 = 47.5 -> 48 (< 50, no R) -> W.
        assert_eq!(h.sun[10], 38);
        assert_eq!(h.sunny[10], 50);
        assert_eq!(h.rain[10], 48);
        assert_eq!(h.cls.as_bytes()[10], b'W');
        // Temperature stays instantaneous at 10:00: (9.8 + 7.4)/2.
        assert_eq!(h.temp[10], 9);
        // Hour 11..12: rain (5 + 95)/2 = 50 and 0.75 mm -> R.
        assert_eq!(h.cls.as_bytes()[11], b'R');
        // 01:00 temp only from second point (first is null).
        assert_eq!(h.temp[1], 5);
        // Speed weighted wdir at 10:00 lies between 230 and 260, nearer 260.
        assert!(h.wdir[10] > 245 && h.wdir[10] < 260, "{}", h.wdir[10]);
        // Gaps carried forward; hours without data use the sun position.
        assert_eq!(h.temp[47], h.temp[12]);
        assert_eq!(h.cls.as_bytes()[22], b'N');
        assert_eq!(h.cls.as_bytes()[13], b'B');

        let d0 = &out.daily[0];
        assert_eq!(d0.date, "2026-11-07");
        let rise = d0.sunrise.as_deref().unwrap();
        let set = d0.sunset.as_deref().unwrap();
        assert!(("07:25".."07:40").contains(&rise), "{rise}");
        assert!(("16:50".."17:05").contains(&set), "{set}");
        // Sun: 37.5 % + 25 % + 25 % of an hour = 0.875 h.
        assert_eq!(d0.sun_h, 0.9);
        assert_eq!(d0.tmax, 9);
        assert_eq!(d0.tmin, 5);
        // Max rain over daylight hours, max wind over all hours.
        assert_eq!(d0.rain, 50);
        assert_eq!(d0.wind, 21);
        assert_eq!(d0.cls, "B");
        // Days without data: computed sunrise/sunset, zero sun.
        let d6 = &out.daily[6];
        assert!(d6.sunrise.is_some() && d6.sunset.is_some());
        assert_eq!(d6.sun_h, 0.0);
    }

    #[test]
    fn daily_rain_class_and_dst_day() {
        // Constant rainy weather over a 25-hour DST day: R, 25 hours counted.
        let start = local_midnight(NaiveDate::from_ymd_opt(2026, 10, 25).unwrap());
        let mut hs = crate::openmeteo::HourlySeries::with_capacity(200);
        for k in 0..(7 * 24 + 2) {
            hs.time.push(start + k * 3600);
            hs.sunshine.push(Some(360.0));
            hs.precip_prob.push(Some(80.0));
            hs.precip.push(Some(1.0));
            hs.temp.push(Some(10.0));
            hs.wind.push(Some(20.0));
            hs.wdir.push(Some(270.0));
            hs.is_day.push(Some(1.0));
        }
        let pw = PointWeather { hourly: hs };
        let axis = TimeAxis::new(
            DateTime::parse_from_rfc3339("2026-10-25T09:00:00+01:00")
                .unwrap()
                .to_utc(),
        );
        let out = aggregate_region(&[&pw], (7.0, 50.0), &axis);
        let d0 = &out.daily[0];
        assert_eq!(d0.sun_h, 2.5);
        assert_eq!(d0.rain, 80);
        assert_eq!(d0.wdir, 270);
        assert_eq!(d0.cls, "W"); // 2.5 h is not < 2 h, ~27 % of daylight
        assert!(out.hourly.cls.chars().all(|c| c == 'R'));
    }
}
