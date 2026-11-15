//! Dates, times and records as people read them.

use crate::derived::Derived;
use crate::domain::{Plan, WorkOrder};
use crate::time::{civil_from_days, parse_date, weekday, DAY};

pub const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
pub const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// `Mon 1 Mar` for a day count since 1970-01-01.
pub fn date(days: i64) -> String {
    let (_, month, day) = civil_from_days(days);
    format!(
        "{} {day} {}",
        WEEKDAYS[weekday(days) as usize],
        MONTHS[(month - 1) as usize]
    )
}

/// `Mon 1 Mar` for a `YYYY-MM-DD` text, or the text itself.
pub fn date_text(text: &str) -> String {
    parse_date(text).map_or_else(|| text.to_string(), date)
}

/// `HH:MM` of a minute.
pub fn clock(minute: i64) -> String {
    let rest = minute.rem_euclid(DAY);
    format!("{:02}:{:02}", rest / 60, rest % 60)
}

/// `Tue 2 Mar 06:15` of minutes from the schedule start.
pub fn moment(d: &Derived, minute: i64) -> String {
    let at = d.origin + minute;
    format!("{} {}", date(at.div_euclid(DAY)), clock(at))
}

/// `Hardening 860 °C` for an order's recipe.
pub fn recipe(order: &WorkOrder) -> String {
    let r = order.ctx.recipe_of(order.index);
    format!("{} {} °C", r.process.label(), r.temperature)
}

/// A furnace's name by index.
pub fn furnace(plan: &Plan, index: usize) -> String {
    plan.furnaces
        .get(index)
        .map_or_else(|| format!("Furnace {index}"), |f| f.label().to_string())
}

/// An operator's name by index.
pub fn operator(plan: &Plan, index: usize) -> String {
    plan.operators
        .get(index)
        .map_or_else(|| format!("Operator {index}"), |o| o.label().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_read_as_weekday_day_and_month() {
        assert_eq!(date(crate::time::days_from_civil(2027, 3, 1)), "Mon 1 Mar");
        assert_eq!(clock(7 * 60 + 5), "07:05");
    }
}
