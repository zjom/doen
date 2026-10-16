//! Date parsing (single dates and ranges) and relative display.

use chrono::{DateTime, Datelike, Duration, FixedOffset, Local, Months, NaiveDate, Weekday};

/// The current moment and the settings that date parsing depends on.
#[derive(Clone, Debug)]
pub struct Clock {
    pub now: DateTime<FixedOffset>,
    pub today: NaiveDate,
    pub week_start: Weekday,
}

impl Clock {
    /// The real clock. The hidden `DOEN_NOW` variable (RFC 3339) fixes the clock for tests.
    pub fn system(week_start: Weekday) -> Clock {
        let now = std::env::var("DOEN_NOW")
            .ok()
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .unwrap_or_else(|| Local::now().fixed_offset());
        Clock::at(now, week_start)
    }

    pub fn at(now: DateTime<FixedOffset>, week_start: Weekday) -> Clock {
        Clock {
            today: now.date_naive(),
            now,
            week_start,
        }
    }

    /// The local day of a timestamp, in the time zone of the clock.
    pub fn local_day(&self, t: &DateTime<FixedOffset>) -> NaiveDate {
        t.with_timezone(self.now.offset()).date_naive()
    }

    fn week_start_of(&self, d: NaiveDate) -> NaiveDate {
        let back = (7 + d.weekday().num_days_from_monday() as i64
            - self.week_start.num_days_from_monday() as i64)
            % 7;
        d - Duration::days(back)
    }
}

pub fn weekday_short(w: Weekday) -> String {
    w.to_string().to_ascii_lowercase()
}

pub fn format_iso(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

/// Parses an ISO date as stored in frontmatter: zero-padded, unlike `NaiveDate::from_str`.
pub fn parse_iso(s: &str) -> Option<NaiveDate> {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

pub fn parse_timestamp(s: &str) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(s).ok()
}

fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn ymd(y: i32, m: &str, d: &str) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(y, m.parse().ok()?, d.parse().ok()?)
}

/// An offset such as `3d`, `+2w`, `1m`. Returns (future, amount, unit).
fn parse_offset(s: &str) -> Option<(bool, u32, char)> {
    let (future, rest) = match s.strip_prefix('+') {
        Some(r) => (true, r),
        None => (false, s),
    };
    let unit = rest.chars().last()?;
    if !matches!(unit, 'd' | 'w' | 'm' | 'y') {
        return None;
    }
    let num = &rest[..rest.len() - 1];
    if !all_digits(num) {
        return None;
    }
    Some((future, num.parse().ok()?, unit))
}

fn apply_offset(today: NaiveDate, future: bool, n: u32, unit: char) -> Option<NaiveDate> {
    if let Some(per) = match unit {
        'm' => Some(1),
        'y' => Some(12),
        _ => None,
    } {
        let months = Months::new(n.checked_mul(per)?);
        return if future {
            today.checked_add_months(months)
        } else {
            today.checked_sub_months(months)
        };
    }
    let days = Duration::try_days(i64::from(n) * if unit == 'w' { 7 } else { 1 })?;
    if future {
        today.checked_add_signed(days)
    } else {
        today.checked_sub_signed(days)
    }
}

fn first_of_month(d: NaiveDate) -> NaiveDate {
    d.with_day(1).expect("every month has a first day")
}

fn end_of_month(d: NaiveDate) -> NaiveDate {
    first_of_month(d)
        .checked_add_months(Months::new(1))
        .unwrap()
        - Duration::days(1)
}

/// The next day after `from`, in the direction `step`, that is on weekday `w`.
fn next_weekday(from: NaiveDate, w: Weekday, step: i64) -> NaiveDate {
    let mut d = from + Duration::days(step);
    while d.weekday() != w {
        d += Duration::days(step);
    }
    d
}

/// Parses a single date in any of the forms of the spec (see "Single dates").
pub fn parse_date(s: &str, clock: &Clock) -> Result<NaiveDate, String> {
    parse_date_opt(s, clock).ok_or_else(|| format!("invalid date: '{s}'"))
}

fn parse_date_opt(s: &str, clock: &Clock) -> Option<NaiveDate> {
    let today = clock.today;
    let parts: Vec<&str> = s.split('-').collect();
    match parts.as_slice() {
        [y, m, d]
            if matches!(y.len(), 2 | 4)
                && m.len() == 2
                && d.len() == 2
                && parts.iter().all(|p| all_digits(p)) =>
        {
            let century = if y.len() == 2 { 2000 } else { 0 };
            return ymd(century + y.parse::<i32>().ok()?, m, d);
        }
        [m, d]
            if (1..=2).contains(&m.len())
                && (1..=2).contains(&d.len())
                && all_digits(m)
                && all_digits(d) =>
        {
            return ymd(today.year(), m, d);
        }
        [x] if x.len() == 6 && all_digits(x) => {
            return ymd(2000 + x[0..2].parse::<i32>().ok()?, &x[2..4], &x[4..6]);
        }
        _ => {}
    }
    if let Some((future, n, unit)) = parse_offset(s) {
        return apply_offset(today, future, n, unit);
    }
    let lower = s.to_ascii_lowercase();
    match lower.as_str() {
        "today" => return Some(today),
        "yesterday" => return today.pred_opt(),
        "tomorrow" => return today.succ_opt(),
        "eow" => return Some(clock.week_start_of(today) + Duration::days(6)),
        "eom" => return Some(end_of_month(today)),
        "eoy" => return NaiveDate::from_ymd_opt(today.year(), 12, 31),
        _ => {}
    }
    if let Ok(w) = lower.parse() {
        return Some(next_weekday(today, w, 1));
    }
    let w = lower.strip_prefix("last-")?.parse().ok()?;
    Some(next_weekday(today, w, -1))
}

/// A filter range. `start` and `end` are inclusive; `None` means open.
#[derive(Clone, Debug, PartialEq)]
pub enum Range {
    Days {
        start: Option<NaiveDate>,
        end: Option<NaiveDate>,
    },
    /// The field is not set.
    Unset,
}

impl Range {
    pub fn contains(&self, d: Option<NaiveDate>) -> bool {
        match (self, d) {
            (Range::Unset, d) => d.is_none(),
            (Range::Days { .. }, None) => false,
            (Range::Days { start, end }, Some(d)) => {
                start.is_none_or(|s| d >= s) && end.is_none_or(|e| d <= e)
            }
        }
    }
}

/// Parses a range in any of the forms of the spec (see "Ranges").
pub fn parse_range(s: &str, clock: &Clock) -> Result<Range, String> {
    let today = clock.today;
    let days = |start, end| Range::Days {
        start: Some(start),
        end: Some(end),
    };
    let range = if let Some((a, b)) = s.split_once("..") {
        if a.is_empty() && b.is_empty() {
            return Err(format!("invalid range: '{s}'"));
        }
        let start = if a.is_empty() {
            None
        } else {
            Some(parse_date(a, clock)?)
        };
        let end = if b.is_empty() {
            None
        } else {
            Some(parse_date(b, clock)?)
        };
        Range::Days { start, end }
    } else if s == "none" {
        Range::Unset
    } else if let Some(r) = parse_period(s, clock) {
        r
    } else if let Some((future, n, unit)) = parse_offset(s) {
        let d =
            apply_offset(today, future, n, unit).ok_or_else(|| format!("invalid date: '{s}'"))?;
        if future {
            days(today, d)
        } else {
            days(d, today)
        }
    } else {
        let d = parse_date(s, clock)?;
        days(d, d)
    };
    if let Range::Days {
        start: Some(a),
        end: Some(b),
    } = range
        && a > b
    {
        return Err(format!("invalid range: '{s}': the start is after the end"));
    }
    Ok(range)
}

fn parse_period(s: &str, clock: &Clock) -> Option<Range> {
    let today = clock.today;
    let (start, end) = match s {
        "W" | "LW" => {
            let mut start = clock.week_start_of(today);
            if s == "LW" {
                start -= Duration::days(7);
            }
            (start, start + Duration::days(6))
        }
        "M" | "LM" => {
            let mut first = first_of_month(today);
            if s == "LM" {
                first = first.checked_sub_months(Months::new(1))?;
            }
            (first, end_of_month(first))
        }
        "Y" | "LY" => {
            let y = today.year() - if s == "LY" { 1 } else { 0 };
            (
                NaiveDate::from_ymd_opt(y, 1, 1)?,
                NaiveDate::from_ymd_opt(y, 12, 31)?,
            )
        }
        _ => return None,
    };
    Some(Range::Days {
        start: Some(start),
        end: Some(end),
    })
}

/// Formats a date for display: relative within `relative_days` of today, else `date_format`.
/// `overdue` selects the wording for a due date of an open todo.
pub fn display(
    d: NaiveDate,
    clock: &Clock,
    relative_days: i64,
    date_format: &str,
    overdue: bool,
) -> String {
    let diff = (d - clock.today).num_days();
    if diff.abs() > relative_days {
        return d.format(date_format).to_string();
    }
    match diff {
        0 => "today".into(),
        n if n < 0 && overdue => format!("{}d overdue", -n),
        -1 => "yesterday".into(),
        1 => "tomorrow".into(),
        n if n < 0 => format!("{}d ago", -n),
        n => format!("{} (+{n}d)", weekday_short(d.weekday())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clock() -> Clock {
        // Friday, 2026-10-16.
        Clock::at(
            DateTime::parse_from_rfc3339("2026-10-16T12:00:00+11:00").unwrap(),
            Weekday::Mon,
        )
    }

    fn d(s: &str) -> NaiveDate {
        parse_iso(s).unwrap()
    }

    #[test]
    fn single_dates() {
        let c = clock();
        let cases = [
            ("2026-10-16", "2026-10-16"),
            ("26-10-18", "2026-10-18"),
            ("261018", "2026-10-18"),
            ("01-05", "2026-01-05"),
            ("today", "2026-10-16"),
            ("Yesterday", "2026-10-15"),
            ("tomorrow", "2026-10-17"),
            ("0d", "2026-10-16"),
            ("3d", "2026-10-13"),
            ("2w", "2026-10-02"),
            ("1m", "2026-09-16"),
            ("1y", "2025-10-16"),
            ("+3d", "2026-10-19"),
            ("+2w", "2026-10-30"),
            ("+1m", "2026-11-16"),
            ("fri", "2026-10-23"),
            ("FRIDAY", "2026-10-23"),
            ("sun", "2026-10-18"),
            ("sat", "2026-10-17"),
            ("last-fri", "2026-10-09"),
            ("last-thursday", "2026-10-15"),
            ("eow", "2026-10-18"),
            ("eom", "2026-10-31"),
            ("eoy", "2026-12-31"),
        ];
        for (input, want) in cases {
            assert_eq!(parse_date(input, &c), Ok(d(want)), "{input}");
        }
        for bad in ["3D", "-3d", "2026-13-01", "W", "foo", "1x", "+", "last-foo"] {
            assert!(parse_date(bad, &c).is_err(), "{bad}");
        }
    }

    #[test]
    fn month_clamps() {
        let c = Clock::at(
            DateTime::parse_from_rfc3339("2026-03-31T12:00:00+11:00").unwrap(),
            Weekday::Mon,
        );
        assert_eq!(parse_date("1m", &c), Ok(d("2026-02-28")));
    }

    #[test]
    fn week_start_changes_eow() {
        let mut c = clock();
        c.week_start = Weekday::Sun;
        assert_eq!(parse_date("eow", &c), Ok(d("2026-10-17")));
    }

    #[test]
    fn ranges() {
        let c = clock();
        let r = |a: &str, b: &str| Range::Days {
            start: Some(d(a)),
            end: Some(d(b)),
        };
        assert_eq!(
            parse_range("26-10-16", &c),
            Ok(r("2026-10-16", "2026-10-16"))
        );
        assert_eq!(
            parse_range("10-01..10-16", &c),
            Ok(r("2026-10-01", "2026-10-16"))
        );
        assert_eq!(
            parse_range("..today", &c),
            Ok(Range::Days {
                start: None,
                end: Some(d("2026-10-16"))
            })
        );
        assert_eq!(
            parse_range("+1d..", &c),
            Ok(Range::Days {
                start: Some(d("2026-10-17")),
                end: None
            })
        );
        assert_eq!(parse_range("1w", &c), Ok(r("2026-10-09", "2026-10-16")));
        assert_eq!(parse_range("+7d", &c), Ok(r("2026-10-16", "2026-10-23")));
        assert_eq!(parse_range("1w..1w", &c), Ok(r("2026-10-09", "2026-10-09")));
        assert_eq!(parse_range("W", &c), Ok(r("2026-10-12", "2026-10-18")));
        assert_eq!(parse_range("LW", &c), Ok(r("2026-10-05", "2026-10-11")));
        assert_eq!(parse_range("M", &c), Ok(r("2026-10-01", "2026-10-31")));
        assert_eq!(parse_range("LM", &c), Ok(r("2026-09-01", "2026-09-30")));
        assert_eq!(parse_range("Y", &c), Ok(r("2026-01-01", "2026-12-31")));
        assert_eq!(parse_range("LY", &c), Ok(r("2025-01-01", "2025-12-31")));
        assert_eq!(parse_range("none", &c), Ok(Range::Unset));
        assert!(parse_range("10-16..10-01", &c).is_err());
        assert!(parse_range("..", &c).is_err());
    }

    #[test]
    fn relative_display() {
        let c = clock();
        let f = |s: &str, o| display(d(s), &c, 7, "%y-%m-%d", o);
        assert_eq!(f("2026-10-16", false), "today");
        assert_eq!(f("2026-10-15", false), "yesterday");
        assert_eq!(f("2026-10-13", true), "3d overdue");
        assert_eq!(f("2026-10-13", false), "3d ago");
        assert_eq!(f("2026-10-18", false), "sun (+2d)");
        assert_eq!(f("2026-10-17", false), "tomorrow");
        assert_eq!(f("2026-11-30", false), "26-11-30");
    }
}
