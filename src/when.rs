//! "2h", "45m", "until 07:00", "tonight", "today", "weekend" → a unix time.

use chrono::{Datelike, Duration, Local, NaiveTime, TimeZone, Weekday};

pub fn parse(spec: &str) -> Result<u64, String> {
    let now = Local::now();
    let s = spec.trim().to_ascii_lowercase();
    let s = s.strip_prefix("until ").unwrap_or(&s).to_string();

    let target = match s.as_str() {
        "tonight" => next_at(now, NaiveTime::from_hms_opt(7, 0, 0).unwrap()),
        "today" | "rest of today" => next_at(now, NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
        "weekend" => {
            // Until Monday 07:00.
            let mut d = now.date_naive();
            loop {
                d = d.succ_opt().ok_or("date overflow")?;
                if d.weekday() == Weekday::Mon {
                    break;
                }
            }
            Local
                .from_local_datetime(&d.and_hms_opt(7, 0, 0).unwrap())
                .single()
                .ok_or("ambiguous local time")?
        }
        _ => {
            if let Ok(t) = NaiveTime::parse_from_str(&s, "%H:%M") {
                next_at(now, t)
            } else {
                now + duration(&s)?
            }
        }
    };
    if target <= now {
        return Err("that's in the past".into());
    }
    Ok(target.timestamp() as u64)
}

fn next_at(now: chrono::DateTime<Local>, t: NaiveTime) -> chrono::DateTime<Local> {
    let today = now.date_naive().and_time(t);
    let candidate = Local.from_local_datetime(&today).earliest().unwrap_or(now);
    if candidate > now {
        candidate
    } else {
        Local
            .from_local_datetime(&(today + Duration::days(1)))
            .earliest()
            .unwrap_or(now)
    }
}

/// "90m", "2h", "1h30m", "1d", "45" (minutes).
fn duration(s: &str) -> Result<Duration, String> {
    let mut total = Duration::zero();
    let mut num = String::new();
    let mut any = false;
    for c in s.chars() {
        if c.is_ascii_digit() {
            num.push(c);
            continue;
        }
        let n: i64 = num.parse().map_err(|_| format!("can't read {s:?}"))?;
        num.clear();
        let part = match c {
            'm' => Duration::try_minutes(n),
            'h' => Duration::try_hours(n),
            'd' => Duration::try_days(n),
            _ => return Err(format!("unknown unit {c:?} in {s:?}")),
        };
        total = part
            .and_then(|p| total.checked_add(&p))
            .ok_or_else(|| format!("{s:?} is longer than time itself; 30 days is the most"))?;
        any = true;
    }
    if !num.is_empty() {
        let n: i64 = num.parse().map_err(|_| format!("can't read {s:?}"))?;
        total = Duration::try_minutes(n)
            .and_then(|p| total.checked_add(&p))
            .ok_or_else(|| format!("{s:?} is longer than time itself; 30 days is the most"))?;
        any = true;
    }
    if !any || total <= Duration::zero() {
        return Err(format!(
            "can't read {s:?}; try 2h, 90m, until 07:00, tonight, weekend"
        ));
    }
    Ok(total)
}

pub fn describe(until: u64) -> String {
    let t = Local.timestamp_opt(until as i64, 0).single();
    match t {
        Some(t) if t.date_naive() == Local::now().date_naive() => {
            t.format("%H:%M today").to_string()
        }
        Some(t) => t.format("%H:%M on %A").to_string(),
        None => "?".into(),
    }
}

pub fn remaining(secs: u64) -> String {
    let (h, m) = (secs / 3600, (secs % 3600) / 60);
    match (h, m) {
        (0, 0) => "under a minute".into(),
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        let now = Local::now().timestamp() as u64;
        assert!((parse("2h").unwrap() as i64 - (now + 7200) as i64).abs() <= 1);
        assert!((parse("1h30m").unwrap() as i64 - (now + 5400) as i64).abs() <= 1);
        assert!((parse("45").unwrap() as i64 - (now + 2700) as i64).abs() <= 1);
        assert!(parse("0m").is_err());
        assert!(parse("soon").is_err());
        // Must be an error, never a panic.
        assert!(parse("3000000000000h").is_err());
        assert!(parse("99999999999999999999d").is_err());
    }

    #[test]
    fn named_times_are_in_the_future() {
        let now = Local::now().timestamp() as u64;
        for s in ["tonight", "today", "weekend", "until 07:00", "23:59"] {
            assert!(parse(s).unwrap() > now, "{s}");
        }
        assert!(parse("weekend").unwrap() - now <= 8 * 86400);
    }
}
