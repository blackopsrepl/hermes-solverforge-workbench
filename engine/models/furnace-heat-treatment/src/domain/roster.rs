use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

use crate::derived::{Ctx, ShiftId, Values};
use crate::wire::ShiftKind;

/// One operator's roster day: which shift they work, or off.
#[planning_entity]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RosterDay {
    /// Position in the plan's roster; set by `prepare`.
    #[planning_id]
    #[serde(skip)]
    pub index: usize,
    /// `<operator>@<day>` unless the dataset names it.
    #[serde(default)]
    pub id: String,
    pub operator: String,
    /// `YYYY-MM-DD`; the day before the schedule holds the carry-in night.
    pub day: String,
    /// The shift worked, `off`, or not decided yet.
    #[serde(default)]
    pub shift: Option<ShiftKind>,
    /// Keep `shift` as given; the solver will not change it.
    #[planning_pin]
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pinned: bool,
    /// Shift values the operator may take this day.
    #[serde(skip)]
    pub allowed: Values,
    #[planning_variable(value_range_provider = "allowed", allows_unassigned = true)]
    #[serde(skip)]
    pub shift_value: Option<usize>,
    #[serde(skip)]
    pub ctx: Ctx,
}

impl RosterDay {
    pub fn operator_idx(&self) -> usize {
        self.ctx.roster[self.index].operator
    }

    pub fn day_index(&self) -> i64 {
        self.ctx.roster[self.index].day
    }

    pub fn kind(&self) -> Option<ShiftKind> {
        self.shift_value.and_then(ShiftKind::from_value)
    }

    pub fn is_working(&self) -> bool {
        self.kind().is_some_and(ShiftKind::is_working)
    }

    /// The shift worked, when working.
    pub fn shift_id(&self) -> Option<ShiftId> {
        self.kind()
            .filter(|kind| kind.is_working())
            .map(|kind| ShiftId { day: self.day_index(), kind })
    }

    /// Working on a day of the schedule itself (not the carry-in night).
    pub fn works_in_schedule(&self) -> bool {
        self.day_index() >= 0 && self.is_working()
    }
}
