//! Contracts: reusable bundles of rules employees work under.

use serde::{Deserialize, Serialize};

use super::{Level, ShiftFilter, Stance, Weekday};

/// A named set of rules.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contract {
    pub id: String,
    #[serde(default)]
    pub rules: Vec<Rule>,
}

/// What a rule limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuleKind {
    /// Minutes worked (shift length minus breaks), per period or rolling window.
    MinutesWorked,
    /// Logged minutes per period, including minutes logged before the data.
    MinutesLogged,
    /// Days with a shift start, per period or rolling window.
    DaysWorked,
    /// Shifts, per period or rolling window.
    ShiftsWorked,
    /// Weekends with a shift starting on a weekend day, per period or rolling window.
    WeekendsWorked,
    /// Minutes worked on weekend days, per period.
    WeekendMinutes,
    /// Length of each run of worked weekends inside a period.
    ConsecutiveWeekendsWorked,
    /// Length of each run of worked days, with optional run categories,
    /// allowed start days, gaps between differing runs and compact runs.
    ConsecutiveDaysWorked,
    /// Length of each run of shifts close together, and rest after a run.
    ConsecutiveShifts,
    /// Days without a shift start, per period.
    DaysOff,
    /// Longest run of days off, per period or rolling window.
    ConsecutiveDaysOff,
    /// Longest stretch of minutes without work, per rolling window.
    ConsecutiveMinutesOff,
    /// Longest run of weekends off, per period.
    ConsecutiveWeekendsOff,
    /// Shifts next to a requested day off.
    NearDayOff,
}

/// One limit of a contract.
///
/// The window is either a `period` (`day`, `week`, `month`, `year`,
/// `schedule` or a custom period id) or `rollingDays`. The penalty is the
/// distance outside `[min, max]`, times `weight` and the employee's priority.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub kind: RuleKind,
    #[serde(default)]
    pub level: Level,
    #[serde(default = "super::one", skip_serializing_if = "super::is_one")]
    pub weight: i64,
    /// Shifts the rule counts; every shift by default.
    #[serde(default, flatten)]
    pub shifts: ShiftFilter,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rolling_days: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<i64>,
    /// Count a shift in a window only when it lies wholly inside it, instead
    /// of when it starts inside it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub whole_shift: bool,
    /// `daysOff`: only these weekdays count.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub weekdays: Vec<Weekday>,
    /// `consecutiveDaysWorked`: tags naming run categories; a change of
    /// category ends a run. A shift carries at most one of them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<String>,
    /// `consecutiveDaysWorked`: weekdays a run may start on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub start_days: Vec<Weekday>,
    /// `consecutiveDaysWorked`: least minutes between runs of different categories.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_gap_minutes: Option<i64>,
    /// `consecutiveDaysWorked`: reward days that continue the previous day's category.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub compact: bool,
    /// `consecutiveShifts`: most minutes from one shift's end to the next
    /// start for both to belong to one run; 720 by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_minutes: Option<i64>,
    /// `consecutiveShifts`: least minutes off after a run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rest_after_minutes: Option<i64>,
    /// `consecutiveShifts`: runs shorter than this need no rest; 1 by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rest_after_runs_of: Option<i64>,
    /// `nearDayOff`: whether such shifts are `barred` or `unwanted`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stance: Option<Stance>,
    /// `nearDayOff`: shifts to avoid on the day before a requested day off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<ShiftFilter>,
    /// `nearDayOff`: shifts to avoid on the day after a requested day off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<ShiftFilter>,
}

impl Rule {
    /// A rule of `kind` with every option at its default.
    pub fn new(kind: RuleKind) -> Self {
        Self {
            kind,
            level: Level::Required,
            weight: 1,
            shifts: ShiftFilter::default(),
            period: None,
            rolling_days: None,
            min: None,
            max: None,
            whole_shift: false,
            weekdays: Vec::new(),
            categories: Vec::new(),
            start_days: Vec::new(),
            min_gap_minutes: None,
            compact: false,
            link_minutes: None,
            rest_after_minutes: None,
            rest_after_runs_of: None,
            stance: None,
            before: None,
            after: None,
        }
    }
}

/// Minutes an employee logged before the data starts, counted by
/// `minutesLogged` rules in the period containing `date`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LoggedMinutes {
    pub date: String,
    pub minutes: i64,
}

/// A break inside a shift; not worked time.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Break {
    pub from: String,
    pub to: String,
}
