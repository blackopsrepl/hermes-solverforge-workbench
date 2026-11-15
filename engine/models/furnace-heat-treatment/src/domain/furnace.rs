use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

use crate::wire::{Downtime, FurnaceKind, Process};

/// A furnace loads run in.
#[problem_fact]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Furnace {
    /// Position in the plan's furnace list; set by `prepare`.
    #[planning_id]
    #[serde(skip)]
    pub index: usize,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub kind: FurnaceKind,
    /// Highest setpoint, °C.
    pub max_temperature: i64,
    /// Heaviest load, kg, all orders of a shared load together.
    pub max_load_kg: i64,
    pub processes: Vec<Process>,
    /// °C per minute when heating up between loads.
    pub heating_rate: i64,
    /// °C per minute when cooling down between loads.
    pub cooling_rate: i64,
    /// Average power drawn while a load runs, kW.
    pub power_kw: i64,
    /// Temperature at the schedule start, °C; the first load changes over from it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_temperature: Option<i64>,
    /// Planned maintenance and other times no load may run.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub downtime: Vec<Downtime>,
}

impl Furnace {
    pub fn label(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.id)
    }
}
