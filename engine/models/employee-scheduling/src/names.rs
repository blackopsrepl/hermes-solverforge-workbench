//! Names for the entities constraint analysis blames, read from the plan.
//!
//! Analysis knows an entity only by its type name and the `Debug` text the
//! planning macros derive, which for every type here starts with
//! `Type { index: N`. The index finds the record; the label is what a person
//! reads: an employee's name, a shift's day and hours.

use solverforge_workbench_host::EntityName;

use crate::derived::{Derived, GlobalKind};
use crate::domain::Plan;
use crate::time::{civil_from_days, parse_date_time, weekday, DAY};

const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

pub fn name_entity(plan: &Plan, type_name: &str, debug: &str) -> Option<EntityName> {
    let index = index_of(debug)?;
    let named = |id: &str, label: String| EntityName {
        id: Some(id.to_string()),
        label,
    };
    let unnamed = |label: String| EntityName { id: None, label };
    match type_name {
        "Shift" => {
            let shift = plan.shifts.get(index)?;
            let when = match (parse_date_time(&shift.start), parse_date_time(&shift.end)) {
                (Some(start), Some(end)) => format!(
                    "{} {}–{}",
                    date(start.div_euclid(DAY)),
                    clock(start),
                    clock(end)
                ),
                _ => format!("{} – {}", shift.start, shift.end),
            };
            let what = shift
                .tags
                .first()
                .map(|tag| format!(" {tag}"))
                .unwrap_or_default();
            Some(named(&shift.id, format!("{when}{what}")))
        }
        "Employee" => {
            let employee = plan.employees.get(index)?;
            Some(named(
                &employee.id,
                employee.name.clone().unwrap_or_else(|| employee.id.clone()),
            ))
        }
        "Day" => {
            let d = derived(plan)?;
            Some(unnamed(date(d.origin + index as i64)))
        }
        "Window" => {
            let d = derived(plan)?;
            let window = d.windows.get(index)?;
            let what = match window.kind {
                GlobalKind::Shifts => "Shifts",
                GlobalKind::DistinctTags => "Distinct tags",
                GlobalKind::Cost | GlobalKind::CostTotal => "Cost",
            };
            let first = d.origin + window.start.div_euclid(DAY);
            let last = d.origin + (window.end - 1).div_euclid(DAY);
            let span = if first == last {
                date(first)
            } else {
                format!("{} – {}", date(first), date(last))
            };
            Some(unnamed(format!("{what}, {span}")))
        }
        "Choice" => {
            let group = plan.alternatives.get(index)?;
            Some(named(&group.id, format!("Alternatives {}", group.id)))
        }
        "Whole" => Some(unnamed("Whole schedule".to_string())),
        _ => None,
    }
}

/// The `N` of a `Type { index: N, …` rendering.
fn index_of(debug: &str) -> Option<usize> {
    let rest = &debug[debug.find("{ index: ")? + "{ index: ".len()..];
    let digits = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..digits].parse().ok()
}

fn derived(plan: &Plan) -> Option<&Derived> {
    plan.days.first().map(|day| &*day.ctx)
}

/// `Mon 1 Mar` for a day count since 1970-01-01.
fn date(days: i64) -> String {
    let (_, month, day) = civil_from_days(days);
    format!(
        "{} {day} {}",
        WEEKDAYS[weekday(days) as usize],
        MONTHS[(month - 1) as usize]
    )
}

/// `HH:MM` of minutes since 1970-01-01T00:00.
fn clock(minutes: i64) -> String {
    let rest = minutes.rem_euclid(DAY);
    format!("{:02}:{:02}", rest / 60, rest % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_index_is_read_from_the_derived_debug_text() {
        assert_eq!(index_of("Shift { index: 12, id: \"a\" }"), Some(12));
        assert_eq!(index_of("Whole { index: 0, ctx: … }"), Some(0));
        assert_eq!(index_of("Shift { id: \"a\" }"), None);
    }

    #[test]
    fn dates_read_as_weekday_day_and_month() {
        // 2027-03-01 is a Monday.
        assert_eq!(date(crate::time::days_from_civil(2027, 3, 1)), "Mon 1 Mar");
        assert_eq!(clock(7 * 60 + 5), "07:05");
    }
}
