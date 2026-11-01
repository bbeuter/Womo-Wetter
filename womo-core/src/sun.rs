//! Solar position and sunrise/sunset (NOAA spreadsheet algorithm).
//!
//! Accuracy is about one minute for mid latitudes, which is plenty here.

use chrono::{NaiveDate, NaiveTime};

/// Zenith angle used for sunrise/sunset (refraction + solar disc radius).
const SUNRISE_ZENITH_DEG: f64 = 90.833;

/// Elevation (deg) below which the sun counts as set (matches sunrise/sunset).
pub const HORIZON_ELEVATION_DEG: f64 = 90.0 - SUNRISE_ZENITH_DEG;

struct SolarParams {
    /// Declination in radians.
    decl: f64,
    /// Equation of time in minutes.
    eq_time: f64,
}

fn julian_century(unix: f64) -> f64 {
    let jd = unix / 86_400.0 + 2_440_587.5;
    (jd - 2_451_545.0) / 36_525.0
}

fn solar_params(unix: f64) -> SolarParams {
    let jc = julian_century(unix);
    let l0 = (280.46646 + jc * (36000.76983 + jc * 0.0003032)).rem_euclid(360.0);
    let m = 357.52911 + jc * (35999.05029 - 0.0001537 * jc);
    let e = 0.016708634 - jc * (0.000042037 + 0.0000001267 * jc);
    let mr = m.to_radians();
    let c = mr.sin() * (1.914602 - jc * (0.004817 + 0.000014 * jc))
        + (2.0 * mr).sin() * (0.019993 - 0.000101 * jc)
        + (3.0 * mr).sin() * 0.000289;
    let true_long = l0 + c;
    let omega = (125.04 - 1934.136 * jc).to_radians();
    let app_long = true_long - 0.00569 - 0.00478 * omega.sin();
    let mean_obliq =
        23.0 + (26.0 + (21.448 - jc * (46.815 + jc * (0.00059 - jc * 0.001813))) / 60.0) / 60.0;
    let obliq = (mean_obliq + 0.00256 * omega.cos()).to_radians();
    let decl = (obliq.sin() * app_long.to_radians().sin()).asin();
    let y = (obliq / 2.0).tan().powi(2);
    let l0r = l0.to_radians();
    let eq = y * (2.0 * l0r).sin() - 2.0 * e * mr.sin()
        + 4.0 * e * y * mr.sin() * (2.0 * l0r).cos()
        - 0.5 * y * y * (4.0 * l0r).sin()
        - 1.25 * e * e * (2.0 * mr).sin();
    SolarParams {
        decl,
        eq_time: 4.0 * eq.to_degrees(),
    }
}

/// Solar elevation angle in degrees (no refraction) at a unix timestamp.
pub fn elevation(unix: i64, lat: f64, lon: f64) -> f64 {
    let p = solar_params(unix as f64);
    let minutes_utc = (unix.rem_euclid(86_400)) as f64 / 60.0;
    let true_solar = (minutes_utc + p.eq_time + 4.0 * lon).rem_euclid(1440.0);
    let ha = (true_solar / 4.0 - 180.0).to_radians();
    let latr = lat.to_radians();
    let cos_zen = latr.sin() * p.decl.sin() + latr.cos() * p.decl.cos() * ha.cos();
    90.0 - cos_zen.clamp(-1.0, 1.0).acos().to_degrees()
}

/// Sunrise and sunset as unix timestamps (UTC) for a calendar date.
/// Returns `None` for polar day/night.
pub fn sunrise_sunset(date: NaiveDate, lat: f64, lon: f64) -> Option<(i64, i64)> {
    let midnight = date.and_time(NaiveTime::MIN).and_utc().timestamp() as f64;
    // First guess: solar noon at 12:00 UTC minus longitude offset, then refine once.
    let mut noon = midnight + (720.0 - 4.0 * lon) * 60.0;
    for _ in 0..2 {
        let p = solar_params(noon);
        noon = midnight + (720.0 - 4.0 * lon - p.eq_time) * 60.0;
    }
    let event = |sign: f64| -> Option<i64> {
        let mut t = noon;
        // Iterate with parameters evaluated at the event time itself.
        for _ in 0..3 {
            let p = solar_params(t);
            let latr = lat.to_radians();
            let cos_ha = SUNRISE_ZENITH_DEG.to_radians().cos() / (latr.cos() * p.decl.cos())
                - latr.tan() * p.decl.tan();
            if !(-1.0..=1.0).contains(&cos_ha) {
                return None;
            }
            let ha_deg = cos_ha.acos().to_degrees();
            let noon_min = 720.0 - 4.0 * lon - p.eq_time;
            t = midnight + (noon_min + sign * 4.0 * ha_deg) * 60.0;
        }
        Some(t.round() as i64)
    };
    Some((event(-1.0)?, event(1.0)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, TimeZone};
    use chrono_tz::Europe::Berlin;

    fn local_hm(unix: i64) -> (u32, u32) {
        use chrono::Timelike;
        let dt = Berlin.from_utc_datetime(&DateTime::from_timestamp(unix, 0).unwrap().naive_utc());
        (dt.hour(), dt.minute())
    }

    fn minutes(hm: (u32, u32)) -> u32 {
        hm.0 * 60 + hm.1
    }

    #[test]
    fn koeln_november() {
        let d = NaiveDate::from_ymd_opt(2026, 11, 7).unwrap();
        let (rise, set) = sunrise_sunset(d, 50.94, 6.96).unwrap();
        let r = minutes(local_hm(rise));
        let s = minutes(local_hm(set));
        // Reference (timeanddate): sunrise ~07:32, sunset ~16:59 CET.
        assert!((7 * 60 + 28..=7 * 60 + 36).contains(&r), "sunrise {r}");
        assert!((16 * 60 + 56..=17 * 60 + 3).contains(&s), "sunset {s}");
    }

    #[test]
    fn berlin_summer_solstice_dst() {
        let d = NaiveDate::from_ymd_opt(2026, 6, 21).unwrap();
        let (rise, set) = sunrise_sunset(d, 52.52, 13.40).unwrap();
        // Reference: 04:43 / 21:33 CEST.
        let r = minutes(local_hm(rise));
        let s = minutes(local_hm(set));
        assert!((4 * 60 + 40..=4 * 60 + 47).contains(&r), "sunrise {r}");
        assert!((21 * 60 + 30..=21 * 60 + 37).contains(&s), "sunset {s}");
    }

    #[test]
    fn polar_night_is_none() {
        let d = NaiveDate::from_ymd_opt(2026, 12, 21).unwrap();
        assert!(sunrise_sunset(d, 80.0, 15.0).is_none());
    }

    #[test]
    fn elevation_noon_and_midnight() {
        // Köln, 2026-06-21 ~13:30 CEST = 11:30 UTC: sun high (~62°).
        let noon = chrono::Utc
            .with_ymd_and_hms(2026, 6, 21, 11, 30, 0)
            .unwrap()
            .timestamp();
        let e = elevation(noon, 50.94, 6.96);
        assert!((60.0..64.0).contains(&e), "elev {e}");
        let night = noon + 12 * 3600;
        assert!(elevation(night, 50.94, 6.96) < -10.0);
    }

    #[test]
    fn elevation_crosses_horizon_at_sunset() {
        let d = NaiveDate::from_ymd_opt(2026, 11, 7).unwrap();
        let (_, set) = sunrise_sunset(d, 50.94, 6.96).unwrap();
        assert!(elevation(set - 300, 50.94, 6.96) > HORIZON_ELEVATION_DEG);
        assert!(elevation(set + 300, 50.94, 6.96) < HORIZON_ELEVATION_DEG);
    }
}
