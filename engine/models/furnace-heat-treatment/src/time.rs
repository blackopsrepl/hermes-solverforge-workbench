//! Local calendar arithmetic: ISO 8601 local date-times to whole days and
//! minutes, without a time-zone database. Every time in a dataset is a local
//! wall-clock time of the one schedule time zone.

/// Minutes in one day.
pub const DAY: i64 = 1440;

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
pub fn weekday(days: i64) -> u8 {
    // 1970-01-01 was a Thursday.
    (days + 3).rem_euclid(7) as u8
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

/// Parses `YYYY-MM-DDTHH:MM[:SS]` into minutes since 1970-01-01T00:00.
/// Seconds must be zero: the model works in whole minutes.
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
    if parts.next().is_some() || hour > 24 || minute > 59 || second != 0 {
        return None;
    }
    if hour == 24 && minute != 0 {
        return None;
    }
    Some(days * DAY + hour * 60 + minute)
}

/// Parses `HH:MM` into minutes after midnight.
pub fn parse_time_of_day(text: &str) -> Option<i64> {
    let (hour, minute) = text.split_once(':')?;
    let (hour, minute) = (number(hour)?, number(minute)?);
    (hour < 24 && minute < 60).then_some(hour * 60 + minute)
}

/// Formats a day count as `YYYY-MM-DD`.
pub fn format_date(days: i64) -> String {
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Formats minutes since 1970-01-01T00:00 as `YYYY-MM-DDTHH:MM`.
pub fn format_date_time(minutes: i64) -> String {
    let days = minutes.div_euclid(DAY);
    let rest = minutes.rem_euclid(DAY);
    format!("{}T{:02}:{:02}", format_date(days), rest / 60, rest % 60)
}

/// Overlap in minutes of two half-open intervals.
pub fn overlap(a_start: i64, a_end: i64, b_start: i64, b_end: i64) -> i64 {
    (a_end.min(b_end) - a_start.max(b_start)).max(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_dates_and_times() {
        let t = parse_date_time("2027-03-01T07:30").unwrap();
        assert_eq!(format_date_time(t), "2027-03-01T07:30");
        assert_eq!(parse_date_time("2027-03-01T07:30:00"), Some(t));
        assert_eq!(parse_date_time("2027-02-29T07:30"), None);
        assert_eq!(
            parse_date_time("2028-02-29T00:00").map(|m| m % DAY),
            Some(0)
        );
        assert_eq!(
            parse_date_time("2027-03-01T24:00"),
            parse_date_time("2027-03-02T00:00")
        );
        assert_eq!(parse_date("2027-13-01"), None);
        assert_eq!(parse_time_of_day("23:15"), Some(23 * 60 + 15));
    }

    #[test]
    fn knows_the_weekday() {
        // 2027-03-01 is a Monday, 2027-03-07 a Sunday.
        assert_eq!(weekday(parse_date("2027-03-01").unwrap()), 0);
        assert_eq!(weekday(parse_date("2027-03-07").unwrap()), 6);
    }

    #[test]
    fn measures_overlap() {
        assert_eq!(overlap(0, 10, 5, 20), 5);
        assert_eq!(overlap(0, 10, 10, 20), 0);
    }
}
