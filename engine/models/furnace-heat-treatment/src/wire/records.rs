//! Dataset records that are neither planning entities nor facts.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::Process;

/// The planning horizon: whole days from `start` (a midnight) to `end`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Schedule {
    /// `YYYY-MM-DDT00:00`.
    pub start: String,
    /// `YYYY-MM-DDT00:00`, after `start`.
    pub end: String,
}

/// A furnace program: process, setpoint and cycle time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Recipe {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub process: Process,
    /// Setpoint in °C.
    pub temperature: i64,
    /// Minutes from charging the furnace to discharging it.
    pub cycle_minutes: i64,
    /// Whether the load is quenched on discharge; the process decides by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quench: Option<bool>,
}

/// A stretch of time a furnace cannot run loads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Downtime {
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Model settings a dataset or profile may change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Parameters {
    /// Minutes between possible load starts.
    #[serde(default = "d_slot")]
    pub slot_minutes: i64,
    /// Minute of the day the morning shift starts; shifts last eight hours.
    #[serde(default = "d_shift_start")]
    pub shift_start: i64,
    #[serde(default = "d_load_build")]
    pub load_build_minutes: i64,
    /// Minutes between the end of load building and programming: charging.
    #[serde(default = "d_15")]
    pub charge_minutes: i64,
    #[serde(default = "d_15")]
    pub program_minutes: i64,
    /// Minutes after discharge spent quenching, or cooling when there is no quench.
    #[serde(default = "d_15")]
    pub quench_minutes: i64,
    #[serde(default = "d_15")]
    pub unload_minutes: i64,
    /// Loads heavier than this need a crane-certified operator to unload.
    #[serde(default = "d_heavy")]
    pub heavy_load_kg: i64,
    /// Fixed minutes to purge and stabilise a furnace between different loads.
    #[serde(default = "d_purge")]
    pub purge_minutes: i64,
    #[serde(default = "d_rest")]
    pub minimum_rest: i64,
    #[serde(default = "d_max_shifts")]
    pub max_shifts_per_week: i64,
    #[serde(default = "d_target_shifts")]
    pub target_shifts_per_week: i64,
    #[serde(default = "d_nights")]
    pub max_consecutive_nights: i64,
    /// Minute of the day the peak tariff starts.
    #[serde(default = "d_peak_start")]
    pub peak_start: i64,
    /// Minute of the day the peak tariff ends.
    #[serde(default = "d_peak_end")]
    pub peak_end: i64,
    /// Cents per kWh at peak.
    #[serde(default = "d_peak_price")]
    pub peak_price: f64,
    /// Cents per kWh off peak.
    #[serde(default = "d_off_peak_price")]
    pub off_peak_price: f64,
    /// Saturdays and Sundays are off peak all day.
    #[serde(default = "d_true")]
    pub weekend_off_peak: bool,
    /// Keys the model does not know, reported as warnings.
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    pub unknown: BTreeMap<String, Value>,
}

impl Default for Parameters {
    fn default() -> Self {
        serde_json::from_value(serde_json::json!({})).expect("every parameter has a default")
    }
}

impl Parameters {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

fn d_slot() -> i64 {
    15
}
fn d_shift_start() -> i64 {
    360
}
fn d_load_build() -> i64 {
    30
}
fn d_15() -> i64 {
    15
}
fn d_heavy() -> i64 {
    500
}
fn d_purge() -> i64 {
    20
}
fn d_rest() -> i64 {
    660
}
fn d_max_shifts() -> i64 {
    6
}
fn d_target_shifts() -> i64 {
    5
}
fn d_nights() -> i64 {
    3
}
fn d_peak_start() -> i64 {
    480
}
fn d_peak_end() -> i64 {
    1200
}
fn d_peak_price() -> f64 {
    22.0
}
fn d_off_peak_price() -> f64 {
    12.0
}
fn d_true() -> bool {
    true
}
