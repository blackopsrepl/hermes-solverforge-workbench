//! Contracts: reusable bundles of rules employees work under.

use serde::{Deserialize, Serialize};

use super::{Level, Stance, TagFilter, Weekday};

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
    /// Shifts that may overlap each other.
    OverlapAllowed,
    /// Minutes between one shift's end and the next shift's start.
    RestBetween,
    /// Spread of shift start times of day within a period, in minutes.
    StartSpread,
    /// Rewards split shifts close together.
    SplitShifts,
    /// A shift that goes (or must not go) with another shift a fixed number of days later.
    DailyPairing,
    /// Shifts in one day that should (or should not) follow a pattern.
    DayPattern,
    /// Worked and free days that should (or should not) follow a pattern.
    MultiDayPattern,
    /// Groups of shifts worked in turn: by week, or in blocks of days.
    Rotation,
    /// Shifts of each category per period.
    ShiftTypes,
    /// Distinct tags of some types worked per period.
    DistinctTags,
    /// Distance from home to matching shifts, and time to change location.
    Travel,
    /// Distinct shift locations per period.
    LocationsWorked,
    /// What the employee's matching work costs per period, with overtime tiers.
    Cost,
    /// How changes to published assignments are weighed for the employee.
    Disruption,
}

/// One limit of a contract.
///
/// The window is either a `period` (`day`, `week`, `month`, `year`,
/// `schedule` or a custom period id) or `rollingDays`. The penalty is the
/// distance outside `[min, max]`, times `weight` and the employee's priority.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub kind: RuleKind,
    #[serde(default)]
    pub level: Level,
    #[serde(default = "super::one", skip_serializing_if = "super::is_one")]
    pub weight: i64,
    /// Shifts the rule counts; every shift by default.
    #[serde(default, flatten)]
    pub shifts: TagFilter,
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
    /// category ends a run. `shiftTypes`: the categories counted per period.
    /// A shift carries at most one of them.
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
    /// `restBetween`, `splitShifts`: pairs further apart than this are ignored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub within_minutes: Option<i64>,
    /// `dailyPairing`: the shift that goes with the rule's shifts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paired: Option<TagFilter>,
    /// `dailyPairing`: days from the rule's shift to the paired shift; 0 by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub day_offset: Option<i64>,
    /// `dayPattern`: shifts of one day, one filter per step.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<TagFilter>,
    /// `dayPattern`: the steps must happen in this order.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ordered: bool,
    /// `multiDayPattern`: one entry per consecutive day.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub days: Vec<PatternDay>,
    /// `rotation`: the groups rotated through, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<TagFilter>,
    /// `rotation`: `weekly` (a new group each week) or `blocks` (blocks of
    /// days limited by `min`/`max`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cycle: Option<Cycle>,
    /// `distinctTags`: tag types whose distinct tags are counted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tag_types: Vec<String>,
    /// `travel`: most metres from the employee's home to a matching shift.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_meters: Option<i64>,
    /// `travel`: least minutes between two shifts at different locations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_change_minutes: Option<i64>,
    /// `cost`: hourly rate up to `baseMinutes` per period; `min` and `max`
    /// are in currency units, penalties count cents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hourly: Option<f64>,
    /// `cost`: minutes per period paid at `hourly`; the rest is overtime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_minutes: Option<i64>,
    /// `cost`: overtime tiers in order; the last one has no limit.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overtime: Vec<Overtime>,
    /// `disruption`: tags whose loss is made good when a new shift keeps them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keep_tags: Vec<String>,
    /// `nearDayOff`: shifts to avoid on the day before a requested day off;
    /// `restBetween`, `splitShifts`: the earlier shift of a pair.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<TagFilter>,
    /// `nearDayOff`: shifts to avoid on the day after a requested day off;
    /// `restBetween`, `splitShifts`: the later shift of a pair.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<TagFilter>,
}

impl Eq for Rule {}

impl std::hash::Hash for Rule {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.kind.hash(state);
        self.period.hash(state);
        self.rolling_days.hash(state);
        self.min.hash(state);
        self.max.hash(state);
    }
}

impl Rule {
    /// A rule of `kind` with every option at its default.
    pub fn new(kind: RuleKind) -> Self {
        Self {
            kind,
            level: Level::Required,
            weight: 1,
            shifts: TagFilter::default(),
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
            within_minutes: None,
            paired: None,
            day_offset: None,
            steps: Vec::new(),
            ordered: false,
            days: Vec::new(),
            groups: Vec::new(),
            cycle: None,
            tag_types: Vec::new(),
            max_meters: None,
            min_change_minutes: None,
            hourly: None,
            base_minutes: None,
            overtime: Vec::new(),
            keep_tags: Vec::new(),
        }
    }
}

/// One overtime tier: `minutes` (unlimited when absent) at `hourly`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Overtime {
    pub hourly: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minutes: Option<i64>,
}

impl Eq for Overtime {}

impl std::hash::Hash for Overtime {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.hourly.to_bits().hash(state);
        self.minutes.hash(state);
    }
}

/// One day of a multi-day pattern.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatternDay {
    /// `true`: a matching shift starts that day; `false`: none does.
    pub work: bool,
    #[serde(default, flatten)]
    pub shifts: TagFilter,
}

/// How a rotation cycles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Cycle {
    Weekly,
    Blocks,
}

/// Minutes an employee logged before the data starts, counted by
/// `minutesLogged` rules in the period containing `date`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LoggedMinutes {
    pub date: String,
    pub minutes: i64,
}

/// A break inside a shift; not worked time, and no cover for demand. With
/// `minutes` shorter than `from`..`to`, the break floats: it lasts `minutes`
/// somewhere in that window, placed where demand misses the employee least.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Break {
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minutes: Option<i64>,
}
