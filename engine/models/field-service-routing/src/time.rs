//! Local calendar arithmetic: ISO 8601 local date-times to whole seconds,
//! without a time-zone database. Every time in a dataset is a wall-clock time
//! of the one service area's time zone.

/// Seconds in one day.
pub const DAY: i64 = 86_400;

const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Days since 1970-01-01 of a proleptic Gregorian date.
pub fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The `(year, month, day)` of a day count since 1970-01-01.
pub fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// Day of the week of a day count, 0 = Monday … 6 = Sunday.
pub fn weekday(days: i64) -> usize {
    // 1970-01-01 was a Thursday.
    (days + 3).rem_euclid(7) as usize
}

fn number(text: &str) -> Option<i64> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// Parses `YYYY-MM-DD` into a day count since 1970-01-01.
pub fn parse_date(text: &str) -> Option<i64> {
    let mut parts = text.split('-');
    let year = number(parts.next()?)?;
    let month = number(parts.next()?)?;
    let day = number(parts.next()?)?;
    if parts.next().is_some() || !(1..=12).contains(&month) || day < 1 {
        return None;
    }
    let days = days_from_civil(year, month, day);
    (civil_from_days(days) == (year, month, day)).then_some(days)
}

/// Parses `YYYY-MM-DDTHH:MM[:SS]` into seconds since 1970-01-01T00:00.
pub fn parse_date_time(text: &str) -> Option<i64> {
    let (date, time) = text.split_once('T')?;
    let days = parse_date(date)?;
    let mut parts = time.split(':');
    let hour = number(parts.next()?)?;
    let minute = number(parts.next()?)?;
    let second = match parts.next() {
        Some(s) => number(s)?,
        None => 0,
    };
    if parts.next().is_some() || hour > 24 || minute > 59 || second > 59 {
        return None;
    }
    if hour == 24 && (minute != 0 || second != 0) {
        return None;
    }
    Some(days * DAY + hour * 3600 + minute * 60 + second)
}

/// Formats seconds since 1970-01-01T00:00 as `YYYY-MM-DDTHH:MM:SS`, or
/// `YYYY-MM-DDTHH:MM` on a whole minute.
pub fn format_date_time(seconds: i64) -> String {
    let days = seconds.div_euclid(DAY);
    let rest = seconds.rem_euclid(DAY);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (rest / 3600, rest % 3600 / 60, rest % 60);
    if ss == 0 {
        format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}")
    } else {
        format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}")
    }
}

/// `HH:MM` of a moment.
pub fn clock(seconds: i64) -> String {
    let rest = seconds.rem_euclid(DAY);
    format!("{:02}:{:02}", rest / 3600, rest % 3600 / 60)
}

/// `Mon 1 Feb` of a moment.
pub fn day_label(seconds: i64) -> String {
    let days = seconds.div_euclid(DAY);
    let (_, month, day) = civil_from_days(days);
    format!(
        "{} {day} {}",
        WEEKDAYS[weekday(days)],
        MONTHS[(month - 1) as usize]
    )
}

/// The weekday and month labels of a day count, for timeline axes.
pub fn axis_labels(days: i64) -> (&'static str, String) {
    let (_, month, day) = civil_from_days(days);
    (
        WEEKDAYS[weekday(days)],
        format!("{day} {}", MONTHS[(month - 1) as usize]),
    )
}

/// A duration in seconds as `2h 30m`, rounded to the minute.
pub fn duration_label(seconds: i64) -> String {
    let minutes = (seconds + 30).div_euclid(60);
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

/// Overlap in seconds of two half-open intervals.
pub fn overlap(a_start: i64, a_end: i64, b_start: i64, b_end: i64) -> i64 {
    (a_end.min(b_end) - a_start.max(b_start)).max(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_dates_and_times() {
        let t = parse_date_time("2027-02-01T07:30").unwrap();
        assert_eq!(format_date_time(t), "2027-02-01T07:30");
        assert_eq!(parse_date_time("2027-02-01T07:30:00"), Some(t));
        assert_eq!(format_date_time(t + 15), "2027-02-01T07:30:15");
        assert_eq!(parse_date_time("2027-02-29T07:30"), None);
        assert_eq!(
            parse_date_time("2027-02-01T24:00"),
            parse_date_time("2027-02-02T00:00")
        );
    }

    #[test]
    fn labels_people_read() {
        let t = parse_date_time("2027-02-01T07:00").unwrap();
        assert_eq!(day_label(t), "Mon 1 Feb");
        assert_eq!(clock(t + 5400), "08:30");
        assert_eq!(duration_label(9000), "2h 30m");
        assert_eq!(duration_label(45 * 60), "45m");
        assert_eq!(overlap(0, 10, 5, 20), 5);
    }
}
