//! The planning horizon and the model's settings.

use serde::{Deserialize, Serialize};

/// The days the plan covers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Horizon {
    /// First day, `YYYY-MM-DD`.
    pub start: String,
    /// Number of days.
    pub days: i64,
}

/// Model settings a dataset or profile sets under `parameters`. Costs are in
/// the same currency units as bay and downtime costs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct Parameters {
    /// Cost of one technician-day above a crew's regular strength.
    pub overtime_cost: i64,
    /// Cost of leaving a deferrable task unplanned, times its unit's priority.
    pub deferral_cost: i64,
    /// Cost of one unit-day below a readiness target.
    pub target_shortfall_cost: i64,
    /// Cost of moving a published task to another bay or day.
    pub change_cost: i64,
    /// Cost of one technician-day above a crew's even share of the work.
    pub load_level_cost: i64,
    /// Cost of each extra workshop visit of a unit beyond its first.
    pub induction_cost: i64,
    /// Days a unit stays out of service after a task, for tasks that set none.
    pub return_to_service_days: i64,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            overtime_cost: 380,
            deferral_cost: 4_000,
            target_shortfall_cost: 15_000,
            change_cost: 3_000,
            load_level_cost: 120,
            induction_cost: 2_500,
            return_to_service_days: 0,
        }
    }
}
