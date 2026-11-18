//! The dataset as callers write it: drivers with shifts, and jobs made of
//! stops. Records name each other by id; times are local date-times
//! (`YYYY-MM-DDTHH:MM[:SS]`); durations are minutes; locations are
//! `[latitude, longitude]`.

use serde::{Deserialize, Serialize};

/// A driver and the shifts they work.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Driver {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub shifts: Vec<DriverShift>,
}

/// One working period of a driver, with its vehicle.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriverShift {
    pub id: String,
    pub start_location: [f64; 2],
    /// Where the shift ends; the start location when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_location: Option<[f64; 2]>,
    /// Earliest time the driver leaves the start location.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_start_time: Option<String>,
    /// Earliest arrival at the first stop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_first_stop_arrival_time: Option<String>,
    /// Hard latest arrival at the end location.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_end_time: Option<String>,
    /// Preferred latest arrival at the end location.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_soft_end_time: Option<String>,
    /// Hard latest departure from the last stop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_last_stop_departure_time: Option<String>,
    /// Preferred latest departure from the last stop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_soft_last_stop_departure_time: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capacities: Vec<Capacity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<Skill>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_breaks: Vec<Break>,
    /// Stops and breaks in driving order: the plan, once solved.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub itinerary: Vec<ItineraryItem>,
    /// Keep the whole itinerary as it is.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pinned: bool,
    /// Overrides the parameter `pinNextStopDuringFreeze` for this shift.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin_next_stop_during_freeze: Option<bool>,
}

/// Room of one kind in a vehicle: seats, wheelchair places, parcels.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capacity {
    #[serde(rename = "type")]
    pub kind: String,
    pub quantity: i64,
}

/// A skill a shift offers.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    pub name: String,
    /// Higher is better; 1 when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<i64>,
    /// Scales the service duration of stops whose job requires this skill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multiplier: Option<f64>,
}

/// A break during a shift: fixed at a time, or floating inside a window.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Break {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: BreakKind,
    /// Where the break is taken; where the driver is when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<[f64; 2]>,
    /// Whether jobs may be on board during the break.
    #[serde(default)]
    pub policy: BreakPolicy,
    /// Fixed breaks: start and end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
    /// Floating breaks: earliest start, latest end and length in minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_start_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_end_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<i64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BreakKind {
    #[default]
    Fixed,
    Floating,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BreakPolicy {
    #[default]
    PermitJobs,
    RejectJobs,
}

/// One stop or break of an itinerary.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItineraryItem {
    pub id: String,
    pub kind: ItemKind,
    /// The start of service a pinned stop keeps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_service_time: Option<String>,
    /// Keep this stop on this shift, starting at `startServiceTime`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pin: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ItemKind {
    #[default]
    Stop,
    Break,
}

/// Something to carry: stops served by one driver shift, in order.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// `1`…`10` mandatory, `opt-1`…`opt-10` optional, or a name from
    /// `priorityWeights`; the parameter `defaultPriority` when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    pub stops: Vec<Stop>,
    /// No other job may be on board while this one is.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub prohibit_job_pooling: bool,
    /// Whether breaks may happen while the job is on board.
    #[serde(default)]
    pub allow_interruption: Interruption,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_drivers: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preferred_drivers: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prohibited_drivers: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_skills: Vec<SkillRequirement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_tags: Vec<String>,
    /// Jobs that may not be on board at the same time as this one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prohibited_jobs: Vec<String>,
    /// Minutes pooling may add to this job's ride; the parameter
    /// `maximumTimeBurden` when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum_time_burden: Option<i64>,
    /// Two-stop jobs only: picked up at the first stop, delivered at the second.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub demands: Vec<JobDemand>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Interruption {
    #[default]
    Breaks,
    None,
}

/// A place a job is picked up from or delivered to.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stop {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub location: [f64; 2],
    /// Service minutes on site.
    pub duration: i64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub time_windows: Vec<TimeWindow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub demands: Vec<Demand>,
    /// The stop should end by then.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_sla_end_time: Option<String>,
    /// The driver may not set off towards this stop earlier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_start_travel_time: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeWindow {
    pub min_start_time: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_start_time: Option<String>,
    pub max_end_time: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Demand {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub pickup: i64,
    #[serde(default)]
    pub deliver: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobDemand {
    #[serde(rename = "type")]
    pub kind: String,
    pub demand: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillRequirement {
    pub name: String,
    /// Any level matches when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_level: Option<i64>,
}

/// A custom priority, or new weights for a built-in one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriorityWeight {
    pub priority: String,
    pub weight: i64,
    #[serde(default)]
    pub assignment: Assignment,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Assignment {
    #[default]
    Mandatory,
    Optional,
}

/// The period the plan is for: `[startDate, endDate)`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanningWindow {
    pub start_date: String,
    pub end_date: String,
}

/// Travel between places: measured along the roads by the engine
/// (`source: "roads"`), or brought by the dataset from its own map service and
/// used as given. `null` marks a leg no road connects.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelMatrix {
    pub locations: Vec<[f64; 2]>,
    pub seconds: Vec<Vec<Option<i64>>>,
    pub meters: Vec<Vec<Option<i64>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Road travel: how far each location is from the road it was snapped to;
    /// `null` where no road is near.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snap_meters: Option<Vec<Option<f64>>>,
}
