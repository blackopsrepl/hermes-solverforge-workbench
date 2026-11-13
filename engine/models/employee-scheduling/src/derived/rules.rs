//! Contract rules compiled per employee: windows in plan minutes, tags as
//! ids, weights already multiplied by the employee's priority.

use super::TagMatch;

/// What a window rule measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Measure {
    Minutes,
    Logged,
    Days,
    Shifts,
    Weekends,
    WeekendMinutes,
    ConsecutiveWeekends,
    DaysOff,
    ConsecutiveDaysOff,
    MinutesOff,
    ConsecutiveWeekendsOff,
    StartSpread,
    ShiftTypes,
    DistinctTags,
}

/// A limit evaluated over a list of windows.
#[derive(Debug, Clone)]
pub struct WindowRule {
    pub measure: Measure,
    pub preferred: bool,
    /// Windows come from `rollingDays` rather than a period.
    pub rolling: bool,
    pub weight: i64,
    pub shifts: TagMatch,
    /// Half-open windows in plan minutes, sorted by start.
    pub windows: Vec<(i64, i64)>,
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub whole_shift: bool,
    /// Weekdays that count for `DaysOff` (bit 0 = Monday).
    pub weekdays: u8,
    /// `ShiftTypes`: category tags; `DistinctTags`: the tags counted. Sorted.
    pub tags: Vec<u32>,
}

/// Limits on runs of consecutive worked days.
#[derive(Debug, Clone)]
pub struct DayRunRule {
    pub preferred: bool,
    pub weight: i64,
    pub shifts: TagMatch,
    pub min: Option<i64>,
    pub max: Option<i64>,
    /// Category tag ids; empty means one category for every shift.
    pub categories: Vec<u32>,
    /// Allowed run start weekdays (bit 0 = Monday); 0 when unrestricted.
    pub start_days: u8,
    pub min_gap: Option<i64>,
    pub compact: bool,
}

/// Limits on runs of shifts close together.
#[derive(Debug, Clone)]
pub struct ShiftRunRule {
    pub preferred: bool,
    pub weight: i64,
    pub shifts: TagMatch,
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub link: i64,
    pub rest_after: Option<i64>,
    pub rest_after_runs_of: i64,
}

/// Shifts to avoid next to a requested day off.
#[derive(Debug, Clone)]
pub struct NearDayOffRule {
    pub barred: bool,
    pub weight: i64,
    pub before: Option<TagMatch>,
    pub after: Option<TagMatch>,
}

/// How a pattern rule wants its situation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Want {
    Required,
    Preferred,
    Unwanted,
    Barred,
}

/// Rest or split-shift limits on pairs of successive shifts.
#[derive(Debug, Clone)]
pub struct GapRule {
    /// `false`: rest range; `true`: split-shift reward with `max` as the largest gap.
    pub split: bool,
    pub preferred: bool,
    pub weight: i64,
    pub before: TagMatch,
    pub after: TagMatch,
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub within: Option<i64>,
}

/// A shift that goes with another one `offset` days later.
#[derive(Debug, Clone)]
pub struct PairingRule {
    pub want: Want,
    pub weight: i64,
    pub first: TagMatch,
    pub paired: TagMatch,
    pub offset: i64,
}

/// Shifts of one day following steps.
#[derive(Debug, Clone)]
pub struct DayPatternRule {
    pub want: Want,
    pub weight: i64,
    pub steps: Vec<TagMatch>,
    pub ordered: bool,
}

/// Worked and free days following a pattern.
#[derive(Debug, Clone)]
pub struct MultiDayRule {
    pub want: Want,
    pub weight: i64,
    pub days: Vec<(bool, TagMatch)>,
}

/// Groups of shifts worked in turn.
#[derive(Debug, Clone)]
pub struct RotationRule {
    pub preferred: bool,
    pub weight: i64,
    pub groups: Vec<TagMatch>,
    /// Weekly cycle windows; empty for block rotations.
    pub weeks: Vec<(i64, i64)>,
    pub min: Option<i64>,
    pub max: Option<i64>,
}

/// What a schedule-wide window counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalKind {
    Shifts,
    DistinctTags,
}

/// One period of a schedule-wide rule.
#[derive(Debug, Clone)]
pub struct GlobalWindow {
    pub kind: GlobalKind,
    pub preferred: bool,
    pub weight: i64,
    pub start: i64,
    pub end: i64,
    /// Only shifts of employees passing this filter count.
    pub employees: Option<TagMatch>,
    /// `DistinctTags`: the tags counted, sorted.
    pub tags: Vec<u32>,
    pub min: Option<i64>,
    pub max: Option<i64>,
}

/// At most `max` matching shifts at any moment of `[from, to)`.
#[derive(Debug, Clone)]
pub struct Concurrency {
    pub max: i64,
    pub shifts: TagMatch,
    pub from: i64,
    pub to: i64,
}

/// Deviation of `value` from `[min, max]`.
pub fn outside(value: i64, min: Option<i64>, max: Option<i64>) -> i64 {
    let below = min.map_or(0, |min| (min - value).max(0));
    let above = max.map_or(0, |max| (value - max).max(0));
    below + above
}
