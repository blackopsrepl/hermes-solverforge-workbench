//! Local calendar arithmetic: ISO 8601 dates to whole days, without a
//! time-zone database. The model plans in whole days; a task occupies its
//! bay from the start of its first day to the end of its last.

/// Minutes in one day.
pub const DAY: i64 = 1440;

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

/// Parses `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM[:SS]` into minutes since
/// 1970-01-01T00:00; a bare date is its midnight.
pub fn parse_moment(text: &str) -> Option<i64> {
    let Some((date, time)) = text.split_once('T') else {
        return parse_date(text).map(|days| days * DAY);
    };
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
    Some(days * DAY + hour * 60 + minute)
}

/// Formats a day count as `YYYY-MM-DD`.
pub fn format_date(days: i64) -> String {
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// `Mon 2 Mar` for a day count since 1970-01-01.
pub fn day_label(days: i64) -> String {
    let (_, month, day) = civil_from_days(days);
    format!(
        "{} {day} {}",
        WEEKDAYS[weekday(days)],
        MONTHS[(month - 1) as usize]
    )
}

/// `Mon` for a day count since 1970-01-01.
pub fn weekday_label(days: i64) -> &'static str {
    WEEKDAYS[weekday(days)]
}

/// `2 Mar` for a day count since 1970-01-01.
pub fn short_date(days: i64) -> String {
    let (_, month, day) = civil_from_days(days);
    format!("{day} {}", MONTHS[(month - 1) as usize])
}

/// `Mon 2 Mar` for one day, `Mon 2 Mar–Thu 5 Mar` for a span of several,
/// from the first day and the exclusive end day.
pub fn span_label(first: i64, end: i64) -> String {
    if end - first <= 1 {
        day_label(first)
    } else {
        format!("{}–{}", day_label(first), day_label(end - 1))
    }
}

/// Days two half-open day ranges share.
pub fn overlap(a_start: i64, a_end: i64, b_start: i64, b_end: i64) -> i64 {
    (a_end.min(b_end) - a_start.max(b_start)).max(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_dates() {
        let d = parse_date("2027-03-01").unwrap();
        assert_eq!(format_date(d), "2027-03-01");
        assert_eq!(parse_date("2027-02-29"), None);
        assert_eq!(parse_date("2027-13-01"), None);
        assert_eq!(
            parse_date("2028-02-29").map(format_date).unwrap(),
            "2028-02-29"
        );
    }

    #[test]
    fn reads_moments_with_or_without_a_time() {
        let d = parse_date("2027-03-01").unwrap();
        assert_eq!(parse_moment("2027-03-01"), Some(d * DAY));
        assert_eq!(parse_moment("2027-03-01T07:30"), Some(d * DAY + 450));
        assert_eq!(parse_moment("2027-03-01T24:00"), Some((d + 1) * DAY));
        assert_eq!(parse_moment("2027-03-01T07:61"), None);
    }

    #[test]
    fn labels_days_and_spans() {
        // 2027-03-01 is a Monday.
        let d = parse_date("2027-03-01").unwrap();
        assert_eq!(weekday(d), 0);
        assert_eq!(day_label(d), "Mon 1 Mar");
        assert_eq!(span_label(d, d + 1), "Mon 1 Mar");
        assert_eq!(span_label(d, d + 4), "Mon 1 Mar–Thu 4 Mar");
        assert_eq!(short_date(d + 6), "7 Mar");
    }

    #[test]
    fn measures_overlap() {
        assert_eq!(overlap(0, 10, 5, 20), 5);
        assert_eq!(overlap(0, 10, 10, 20), 0);
    }
}
