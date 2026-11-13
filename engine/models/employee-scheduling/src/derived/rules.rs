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

/// Deviation of `value` from `[min, max]`.
pub fn outside(value: i64, min: Option<i64>, max: Option<i64>) -> i64 {
    let below = min.map_or(0, |min| (min - value).max(0));
    let above = max.map_or(0, |max| (value - max).max(0));
    below + above
}
