//! The derived fact tables.

/// Tag ids as a sorted, deduplicated list.
pub type TagSet = Vec<u32>;

/// A compiled [`ShiftFilter`](crate::wire::ShiftFilter).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TagMatch {
    pub with: TagSet,
    pub without: TagSet,
    pub all: bool,
}

impl TagMatch {
    /// Whether a shift with `tags` passes.
    pub fn accepts(&self, tags: &[u32]) -> bool {
        let has = |tag: &u32| tags.binary_search(tag).is_ok();
        let included = self.with.is_empty()
            || if self.all {
                self.with.iter().all(has)
            } else {
                self.with.iter().any(has)
            };
        let excluded = !self.without.is_empty()
            && if self.all {
                self.without.iter().all(has)
            } else {
                self.without.iter().any(has)
            };
        included && !excluded
    }

    pub fn is_everything(&self) -> bool {
        self.with.is_empty() && self.without.is_empty()
    }
}

/// A compiled [`TimeSpan`](crate::wire::TimeSpan), in plan minutes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Span {
    pub start: i64,
    pub end: i64,
    pub shifts: TagMatch,
    pub weight: i64,
}

/// A skill an employee holds, valid in `[from, to)` plan minutes.
#[derive(Debug, Clone, PartialEq)]
pub struct Grant {
    pub skill: u32,
    pub level: u32,
    pub from: i64,
    pub to: i64,
}

/// A skill a shift asks for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Need {
    pub skill: u32,
    pub level: u32,
}

/// Everything about a shift that never changes while solving.
#[derive(Debug, Clone, Default)]
pub struct ShiftInfo {
    /// Plan minutes (minutes since the first day of the data at 00:00).
    pub start: i64,
    pub end: i64,
    /// Day of the start, and of the last minute.
    pub day: i64,
    pub last_day: i64,
    pub tags: TagSet,
    pub required_skills: Vec<Need>,
    pub any_required_skill: bool,
    pub preferred_skills: Vec<Need>,
    pub risks: TagSet,
    pub optional: bool,
    pub importance: i64,
    /// Employee indexes, sorted.
    pub preferred_employees: Vec<u32>,
    pub unwanted_employees: Vec<u32>,
    pub barred_employees: Vec<u32>,
    /// Inside the planning horizon (outside means history or context).
    pub in_horizon: bool,
}

/// Everything about an employee that never changes while solving.
#[derive(Debug, Clone, Default)]
pub struct EmployeeInfo {
    pub tags: TagSet,
    pub priority: i64,
    pub skills: Vec<Grant>,
    pub avoid_risks: TagSet,
    pub unavailable: Vec<Span>,
    pub available_only: Vec<Span>,
    pub preferred_times: Vec<Span>,
    pub unwanted_times: Vec<Span>,
}

/// All derived facts of one plan.
#[derive(Debug, Clone, Default)]
pub struct Derived {
    /// Calendar day (days since 1970-01-01) of plan day 0.
    pub origin: i64,
    /// Planning horizon in plan minutes.
    pub horizon_start: i64,
    pub horizon_end: i64,
    /// Number of plan days covered by the data.
    pub days: i64,
    /// 0 = Monday.
    pub week_start: u8,
    /// Bit `n` set when weekday `n` is a weekend day.
    pub weekend_mask: u8,
    /// Tag, skill and risk names by id.
    pub names: Vec<String>,
    pub shifts: Vec<ShiftInfo>,
    pub employees: Vec<EmployeeInfo>,
}

impl Derived {
    /// Weekday (0 = Monday) of a plan day.
    pub fn weekday(&self, day: i64) -> u8 {
        crate::time::weekday(self.origin + day)
    }

    pub fn is_weekend(&self, day: i64) -> bool {
        self.weekend_mask & (1 << self.weekday(day)) != 0
    }
}
