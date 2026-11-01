//! Time axis (48 hours / 7 days in Europe/Berlin) and formatting helpers.

use chrono::{DateTime, Duration, LocalResult, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Europe::Berlin;
use chrono_tz::Tz;

pub const HOURS: usize = 48;
pub const DAYS: usize = 7;

/// Output time axis shared by latest.json and grid.json.
#[derive(Debug, Clone)]
pub struct TimeAxis {
    pub generated: DateTime<Tz>,
    /// Hour instants (unix seconds), hourly from local 00:00 of the current day.
    pub hours: Vec<i64>,
    pub days: Vec<NaiveDate>,
}

impl TimeAxis {
    pub fn new(now: DateTime<Utc>) -> Self {
        let local = now.with_timezone(&Berlin);
        let today = local.date_naive();
        let start = local_midnight(today);
        let hours = (0..HOURS as i64).map(|h| start + h * 3600).collect();
        let days = (0..DAYS as i64).map(|d| today + Duration::days(d)).collect();
        TimeAxis {
            generated: local,
            hours,
            days,
        }
    }

    pub fn hour_strings(&self) -> Vec<String> {
        self.hours.iter().map(|&t| fmt_local(t)).collect()
    }

    pub fn day_strings(&self) -> Vec<String> {
        self.days.iter().map(|d| d.format("%Y-%m-%d").to_string()).collect()
    }
}

/// Unix timestamp of 00:00 local time (Europe/Berlin) on a date.
pub fn local_midnight(date: NaiveDate) -> i64 {
    resolve_local(date.and_hms_opt(0, 0, 0).unwrap(), None)
}

/// Resolves a Berlin wall-clock time to a unix timestamp.
/// Ambiguous times pick the earliest unless that is not after `prev`
/// (keeps sequences monotonic across the autumn DST change).
pub fn resolve_local(naive: NaiveDateTime, prev: Option<i64>) -> i64 {
    match Berlin.from_local_datetime(&naive) {
        LocalResult::Single(dt) => dt.timestamp(),
        LocalResult::Ambiguous(a, b) => {
            let (a, b) = (a.timestamp(), b.timestamp());
            match prev {
                Some(p) if a <= p => b,
                _ => a,
            }
        }
        // Spring-forward gap: shift by one hour.
        LocalResult::None => Berlin
            .from_local_datetime(&(naive + Duration::hours(1)))
            .earliest()
            .map(|d| d.timestamp())
            .unwrap_or_else(|| naive.and_utc().timestamp()),
    }
}

pub fn to_local(unix: i64) -> DateTime<Tz> {
    DateTime::from_timestamp(unix, 0)
        .unwrap_or_default()
        .with_timezone(&Berlin)
}

/// `2026-11-07T10:00+01:00`
pub fn fmt_local(unix: i64) -> String {
    to_local(unix).format("%Y-%m-%dT%H:%M%:z").to_string()
}

pub fn fmt_dt(dt: &DateTime<Tz>) -> String {
    dt.format("%Y-%m-%dT%H:%M%:z").to_string()
}

/// `HH:MM` in local time.
pub fn fmt_hm(unix: i64) -> String {
    to_local(unix).format("%H:%M").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    #[test]
    fn axis_winter() {
        let ax = TimeAxis::new(utc("2026-11-07T09:12:00+01:00"));
        let h = ax.hour_strings();
        assert_eq!(h.len(), 48);
        assert_eq!(h[0], "2026-11-07T00:00+01:00");
        assert_eq!(h[10], "2026-11-07T10:00+01:00");
        assert_eq!(h[47], "2026-11-08T23:00+01:00");
        assert_eq!(ax.day_strings()[6], "2026-11-13");
        assert_eq!(fmt_dt(&ax.generated), "2026-11-07T09:12+01:00");
    }

    #[test]
    fn axis_dst_end() {
        // 2026-10-25: clocks go back 03:00 CEST -> 02:00 CET.
        let ax = TimeAxis::new(utc("2026-10-25T08:00:00Z"));
        let h = ax.hour_strings();
        assert_eq!(h[0], "2026-10-25T00:00+02:00");
        assert_eq!(h[2], "2026-10-25T02:00+02:00");
        assert_eq!(h[3], "2026-10-25T02:00+01:00");
        assert_eq!(h[25], "2026-10-26T00:00+01:00");
    }

    #[test]
    fn axis_near_midnight_uses_local_day() {
        // 23:30 UTC on Nov 6 is already Nov 7 in Berlin.
        let ax = TimeAxis::new(utc("2026-11-06T23:30:00Z"));
        assert_eq!(ax.day_strings()[0], "2026-11-07");
    }

    #[test]
    fn resolve_ambiguous_monotonic() {
        let n = NaiveDate::from_ymd_opt(2026, 10, 25)
            .unwrap()
            .and_hms_opt(2, 0, 0)
            .unwrap();
        let first = resolve_local(n, None);
        let second = resolve_local(n, Some(first));
        assert_eq!(second - first, 3600);
    }
}
