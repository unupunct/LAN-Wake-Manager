//! Recurrence arithmetic shared by the controller scheduler and the relay scheduler.
use crate::models::Recurrence;
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};

#[derive(Clone, Copy, Debug)]
pub enum Zone {
    Local,
    Named(chrono_tz::Tz),
}
impl Zone {
    pub fn parse(s: Option<&str>) -> Zone {
        match s.and_then(|s| s.parse::<chrono_tz::Tz>().ok()) {
            Some(t) => Zone::Named(t),
            None => Zone::Local,
        }
    }
    fn to_ts(&self, naive: NaiveDateTime) -> Option<i64> {
        match self {
            Zone::Local => chrono::Local.from_local_datetime(&naive).earliest().or_else(|| chrono::Local.from_local_datetime(&(naive + Duration::hours(1))).earliest()).map(|d| d.timestamp()),
            Zone::Named(z) => z.from_local_datetime(&naive).earliest().or_else(|| z.from_local_datetime(&(naive + Duration::hours(1))).earliest()).map(|d| d.timestamp()),
        }
    }
    pub fn date_of(&self, ts: i64) -> NaiveDate {
        match self {
            Zone::Local => chrono::Local.timestamp_opt(ts, 0).single().map(|d| d.date_naive()).unwrap_or_default(),
            Zone::Named(z) => z.timestamp_opt(ts, 0).single().map(|d| d.date_naive()).unwrap_or_default(),
        }
    }
}

pub fn parse_hhmm(s: &str) -> Option<NaiveTime> {
    let (h, m) = s.split_once(':')?;
    NaiveTime::from_hms_opt(h.trim().parse().ok()?, m.trim().parse().ok()?, 0)
}
fn parse_date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}
fn month_len(y: i32, m: u32) -> u32 {
    let next = if m == 12 { NaiveDate::from_ymd_opt(y + 1, 1, 1) } else { NaiveDate::from_ymd_opt(y, m + 1, 1) };
    next.and_then(|n| n.pred_opt()).map(|d| d.day()).unwrap_or(28)
}

pub struct Spec<'a> {
    pub rec: &'a Recurrence,
    pub start_date: Option<&'a str>,
    pub end_date: Option<&'a str>,
    pub exceptions: &'a [String],
}

pub fn validate(rec: &Recurrence, start: Option<&str>, end: Option<&str>) -> Result<(), String> {
    if let Some(a) = &rec.at {
        parse_hhmm(a).ok_or("time must be HH:MM")?;
    } else if rec.kind != "custom" {
        return Err("time of day (at) is required".into());
    }
    match rec.kind.as_str() {
        "once" => {
            parse_date(rec.date.as_deref().ok_or("date is required for one-time schedules")?).ok_or("date must be YYYY-MM-DD")?;
        }
        "daily" => {}
        "weekly" => {
            if rec.weekdays.is_empty() || rec.weekdays.iter().any(|d| !(1..=7).contains(d)) {
                return Err("weekdays must contain values 1 (Mon) to 7 (Sun)".into());
            }
        }
        "monthly" => {
            let d = rec.day_of_month.ok_or("day_of_month is required")?;
            if !(1..=31).contains(&d) {
                return Err("day_of_month must be 1-31".into());
            }
        }
        "custom" => {
            let i = rec.interval.as_ref().ok_or("interval is required for custom schedules")?;
            if i.every == 0 || i.every > 10_000 || !matches!(i.unit.as_str(), "minutes" | "hours" | "days") {
                return Err("interval must be a positive number of minutes, hours or days".into());
            }
        }
        k => return Err(format!("unknown recurrence kind '{k}'")),
    }
    for d in [start, end].into_iter().flatten() {
        parse_date(d).ok_or("start/end dates must be YYYY-MM-DD")?;
    }
    Ok(())
}

/// First occurrence strictly after `after` (unix seconds), honouring start/end dates and exceptions.
pub fn next_after(spec: &Spec, zone: Zone, after: i64) -> Option<i64> {
    let rec = spec.rec;
    let start = spec.start_date.and_then(parse_date);
    let end = spec.end_date.and_then(parse_date);
    let ok_date = |d: NaiveDate| -> bool {
        start.map(|s| d >= s).unwrap_or(true) && end.map(|e| d <= e).unwrap_or(true) && !spec.exceptions.iter().any(|x| x == &d.format("%Y-%m-%d").to_string())
    };
    let at = rec.at.as_deref().and_then(parse_hhmm).unwrap_or(NaiveTime::from_hms_opt(0, 0, 0)?);
    match rec.kind.as_str() {
        "once" => {
            let d = parse_date(rec.date.as_deref()?)?;
            if !ok_date(d) {
                return None;
            }
            let ts = zone.to_ts(d.and_time(at))?;
            (ts > after).then_some(ts)
        }
        "daily" | "weekly" | "monthly" => {
            let first = zone.date_of(after) - Duration::days(1);
            for i in 0..(366 * 6) {
                let d = first + Duration::days(i);
                if !ok_date(d) {
                    if end.map(|e| d > e).unwrap_or(false) {
                        return None;
                    }
                    continue;
                }
                let hit = match rec.kind.as_str() {
                    "daily" => true,
                    "weekly" => rec.weekdays.contains(&d.weekday().number_from_monday()),
                    _ => {
                        let want = rec.day_of_month.unwrap_or(1).min(month_len(d.year(), d.month()));
                        d.day() == want
                    }
                };
                if hit {
                    if let Some(ts) = zone.to_ts(d.and_time(at)) {
                        if ts > after {
                            return Some(ts);
                        }
                    }
                }
            }
            None
        }
        "custom" => {
            let iv = rec.interval.as_ref()?;
            let step: i64 = iv.every as i64 * match iv.unit.as_str() {
                "minutes" => 60,
                "hours" => 3600,
                "days" => 86_400,
                _ => return None,
            };
            let anchor_date = start.unwrap_or_else(|| NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());
            let anchor = zone.to_ts(anchor_date.and_time(at))?;
            let n = if after < anchor { 0 } else { (after - anchor) / step + 1 };
            let mut ts = anchor + n * step;
            // skip exception days
            for _ in 0..10_000 {
                let d = zone.date_of(ts);
                if end.map(|e| d > e).unwrap_or(false) {
                    return None;
                }
                if ok_date(d) {
                    return Some(ts);
                }
                ts += step;
            }
            None
        }
        _ => None,
    }
}

/// Decision of the scheduler at time `now` for a schedule whose last processed instant is `cursor`.
#[derive(Debug, PartialEq)]
pub enum Due {
    Nothing,
    /// fire; the `i64` is the occurrence timestamp (used for duplicate protection); bool = missed run
    Fire(i64, bool),
}
pub fn due(spec: &Spec, zone: Zone, cursor: i64, now: i64, grace_secs: i64, run_missed: bool) -> Due {
    let mut on_time: Option<i64> = None;
    let mut missed: Option<i64> = None;
    let mut c = cursor;
    for _ in 0..1000 {
        match next_after(spec, zone, c) {
            Some(t) if t <= now => {
                if now - t <= grace_secs {
                    on_time = Some(t);
                } else {
                    missed = Some(t);
                }
                c = t;
            }
            _ => break,
        }
    }
    match (on_time, missed) {
        (Some(t), _) => Due::Fire(t, false),
        (None, Some(t)) if run_missed => Due::Fire(t, true),
        _ => Due::Nothing,
    }
}

pub fn describe(rec: &Recurrence) -> String {
    let at = rec.at.clone().unwrap_or_default();
    match rec.kind.as_str() {
        "once" => format!("once on {} at {}", rec.date.clone().unwrap_or_default(), at),
        "daily" => format!("every day at {at}"),
        "weekly" => {
            let names = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
            format!("every {} at {}", rec.weekdays.iter().filter_map(|d| names.get(*d as usize - 1)).cloned().collect::<Vec<_>>().join(", "), at)
        }
        "monthly" => format!("monthly on day {} at {}", rec.day_of_month.unwrap_or(1), at),
        _ => rec.interval.as_ref().map(|i| format!("every {} {}", i.every, i.unit)).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Interval;
    const Z: Zone = Zone::Named(chrono_tz::UTC);
    fn ts(s: &str) -> i64 {
        chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap().and_utc().timestamp()
    }
    fn rec(kind: &str) -> Recurrence {
        Recurrence { kind: kind.into(), at: Some("07:30".into()), date: None, weekdays: vec![], day_of_month: None, interval: None }
    }
    #[test]
    fn daily() {
        let r = rec("daily");
        let s = Spec { rec: &r, start_date: None, end_date: None, exceptions: &[] };
        assert_eq!(next_after(&s, Z, ts("2026-03-01 07:00")), Some(ts("2026-03-01 07:30")));
        assert_eq!(next_after(&s, Z, ts("2026-03-01 07:30")), Some(ts("2026-03-02 07:30")));
    }
    #[test]
    fn weekly_weekdays_skip_weekend() {
        let mut r = rec("weekly");
        r.weekdays = vec![1, 2, 3, 4, 5];
        let s = Spec { rec: &r, start_date: None, end_date: None, exceptions: &[] };
        // 2026-03-06 is a Friday
        assert_eq!(next_after(&s, Z, ts("2026-03-06 08:00")), Some(ts("2026-03-09 07:30")));
    }
    #[test]
    fn monthly_clamps_to_month_length() {
        let mut r = rec("monthly");
        r.day_of_month = Some(31);
        let s = Spec { rec: &r, start_date: None, end_date: None, exceptions: &[] };
        assert_eq!(next_after(&s, Z, ts("2026-02-01 00:00")), Some(ts("2026-02-28 07:30")));
        assert_eq!(next_after(&s, Z, ts("2026-02-28 08:00")), Some(ts("2026-03-31 07:30")));
    }
    #[test]
    fn once_and_range_and_exceptions() {
        let mut r = rec("once");
        r.date = Some("2026-12-24".into());
        let s = Spec { rec: &r, start_date: None, end_date: None, exceptions: &[] };
        assert_eq!(next_after(&s, Z, ts("2026-01-01 00:00")), Some(ts("2026-12-24 07:30")));
        assert_eq!(next_after(&s, Z, ts("2026-12-24 07:30")), None);
        let d = rec("daily");
        let ex = vec!["2026-03-02".to_string()];
        let s = Spec { rec: &d, start_date: Some("2026-03-01"), end_date: Some("2026-03-03"), exceptions: &ex };
        assert_eq!(next_after(&s, Z, ts("2026-03-01 07:30")), Some(ts("2026-03-03 07:30")));
        assert_eq!(next_after(&s, Z, ts("2026-03-03 07:30")), None);
        assert_eq!(next_after(&s, Z, ts("2026-02-01 00:00")), Some(ts("2026-03-01 07:30")));
    }
    #[test]
    fn custom_interval() {
        let mut r = rec("custom");
        r.interval = Some(Interval { every: 6, unit: "hours".into() });
        let s = Spec { rec: &r, start_date: Some("2026-03-01"), end_date: None, exceptions: &[] };
        assert_eq!(next_after(&s, Z, ts("2026-03-01 07:30")), Some(ts("2026-03-01 13:30")));
        assert_eq!(next_after(&s, Z, ts("2026-03-01 00:00")), Some(ts("2026-03-01 07:30")));
    }
    #[test]
    fn missed_handling() {
        let r = rec("daily");
        let s = Spec { rec: &r, start_date: None, end_date: None, exceptions: &[] };
        let cursor = ts("2026-03-01 08:00");
        // controller was down until 12:00 next day; grace 60 min
        let now = ts("2026-03-02 12:00");
        assert_eq!(due(&s, Z, cursor, now, 3600, false), Due::Nothing);
        assert_eq!(due(&s, Z, cursor, now, 3600, true), Due::Fire(ts("2026-03-02 07:30"), true));
        // on time within grace
        assert_eq!(due(&s, Z, cursor, ts("2026-03-02 07:31"), 3600, false), Due::Fire(ts("2026-03-02 07:30"), false));
        // nothing new
        assert_eq!(due(&s, Z, ts("2026-03-02 07:31"), ts("2026-03-02 07:45"), 3600, true), Due::Nothing);
    }
    #[test]
    fn validation() {
        assert!(validate(&rec("daily"), None, None).is_ok());
        assert!(validate(&rec("weekly"), None, None).is_err());
        let mut bad = rec("daily");
        bad.at = Some("25:99".into());
        assert!(validate(&bad, None, None).is_err());
        assert!(validate(&rec("daily"), Some("nope"), None).is_err());
    }
}
