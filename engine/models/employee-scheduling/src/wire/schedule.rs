//! The schedule header: horizon, calendar conventions and custom periods.

use serde::{Deserialize, Serialize};

/// Day of the week.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl Weekday {
    /// 0 = Monday … 6 = Sunday.
    pub fn number(self) -> u8 {
        self as u8
    }
}

/// What is being planned and how the calendar is read.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Schedule {
    /// First moment being planned, `YYYY-MM-DDTHH:MM` (inclusive).
    pub start: String,
    /// End of the plan, `YYYY-MM-DDTHH:MM` (exclusive). Shifts outside it are
    /// history or context: they count toward rules but are never reassigned.
    pub end: String,
    /// First day of a week for weekly periods.
    #[serde(default = "monday")]
    pub week_start: Weekday,
    /// Days that make up a weekend; consecutive, Saturday and Sunday by default.
    #[serde(default = "saturday_sunday")]
    pub weekend: Vec<Weekday>,
    /// Named periods beyond the built-in ones (day, week, month, year, schedule).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub periods: Vec<CustomPeriod>,
}

/// A named set of date ranges; each range is one period of that name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomPeriod {
    pub id: String,
    pub ranges: Vec<DateRange>,
}

/// Dates `from` through `to`, both inclusive, `YYYY-MM-DD`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DateRange {
    pub from: String,
    pub to: String,
}

fn monday() -> Weekday {
    Weekday::Monday
}

fn saturday_sunday() -> Vec<Weekday> {
    vec![Weekday::Saturday, Weekday::Sunday]
}
