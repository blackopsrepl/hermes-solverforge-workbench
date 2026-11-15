//! The derived facts of one plan. Days are numbered from the horizon start
//! (day 0); ranges of days are half-open `(first, end)`.

use crate::wire::Parameters;

/// Everything scoring reads that does not change while solving.
#[derive(Debug, Default)]
pub struct Derived {
    /// Day count since 1970-01-01 of the horizon start.
    pub origin: i64,
    /// Days in the horizon.
    pub days: i64,
    pub params: Parameters,
    pub units: Vec<UnitInfo>,
    pub bays: Vec<BayInfo>,
    pub crews: Vec<CrewInfo>,
    pub parts: Vec<PartInfo>,
    pub rules: Vec<RuleInfo>,
    pub tasks: Vec<TaskInfo>,
}

#[derive(Debug, Clone, Default)]
pub struct UnitInfo {
    pub fleet: usize,
    pub priority: i64,
    pub downtime_cost: i64,
    /// First day the unit is at the operator's disposal; 0 when always.
    pub available_from: i64,
    /// Days the unit is committed to operations.
    pub commitments: Vec<(i64, i64)>,
}

#[derive(Debug, Clone, Default)]
pub struct BayInfo {
    pub capacity: i64,
    pub cost_per_day: i64,
    pub closures: Vec<(i64, i64)>,
}

#[derive(Debug, Clone, Default)]
pub struct CrewInfo {
    /// Regular strength on each day of the horizon.
    pub regular: Vec<i64>,
    pub overtime: i64,
    /// The crew's even share of the work: every task's technician-days spread
    /// evenly over the horizon, rounded up.
    pub level: i64,
}

#[derive(Debug, Clone, Default)]
pub struct PartInfo {
    pub on_hand: i64,
    /// `(day, quantity)` of each delivery, by day; days before the horizon
    /// count as day 0.
    pub deliveries: Vec<(i64, i64)>,
}

impl PartInfo {
    /// Parts received up to and including `day`, opening stock included.
    pub fn supplied_by(&self, day: i64) -> i64 {
        self.on_hand
            + self
                .deliveries
                .iter()
                .take_while(|(arrives, _)| *arrives <= day)
                .map(|(_, quantity)| quantity)
                .sum::<i64>()
    }
}

#[derive(Debug, Clone, Default)]
pub struct RuleInfo {
    /// Fleet the rule counts; every unit when `None`.
    pub fleet: Option<usize>,
    pub minimum: i64,
    pub target: i64,
    pub first: i64,
    pub end: i64,
    /// Units the rule counts.
    pub scope: i64,
}

impl RuleInfo {
    pub fn applies(&self, day: i64) -> bool {
        (self.first..self.end).contains(&day)
    }

    pub fn counts(&self, fleet: usize) -> bool {
        self.fleet.is_none_or(|f| f == fleet)
    }
}

#[derive(Debug, Clone, Default)]
pub struct TaskInfo {
    pub unit: usize,
    pub days: i64,
    /// First day the task may start (0 at the earliest).
    pub earliest: i64,
    /// Exclusive end the task must finish by, from its due date; never
    /// earlier than its earliest possible finish.
    pub finish_by: Option<i64>,
    /// Last day the task may start, from its usage limit; never earlier than
    /// its earliest start.
    pub start_by: Option<i64>,
    /// First day the unit may no longer operate without the work done: the
    /// day after its due date or after it reaches its usage limit.
    pub grounded_from: Option<i64>,
    /// Days out of service after the work.
    pub buffer: i64,
    /// Bays the task can be done in, by bay index.
    pub suitable: Vec<bool>,
    /// `(crew, technicians per day)`.
    pub labour: Vec<(usize, i64)>,
    /// `(part, quantity)`.
    pub parts: Vec<(usize, i64)>,
    /// Tasks that must finish before this one starts.
    pub preds: Vec<usize>,
    pub published: Option<usize>,
    pub deferrable: bool,
}

impl Derived {
    /// Bay of a slot.
    pub fn slot_bay(&self, slot: usize) -> usize {
        slot / self.days.max(1) as usize
    }

    /// Day of a slot.
    pub fn slot_day(&self, slot: usize) -> i64 {
        (slot % self.days.max(1) as usize) as i64
    }

    /// The slot of a bay on a day.
    pub fn slot(&self, bay: usize, day: i64) -> usize {
        bay * self.days as usize + day as usize
    }

    /// Whether unit `unit` is away (not yet available) on `day`, whatever the plan.
    pub fn away(&self, unit: usize, day: i64) -> bool {
        day < self.units[unit].available_from
    }
}
