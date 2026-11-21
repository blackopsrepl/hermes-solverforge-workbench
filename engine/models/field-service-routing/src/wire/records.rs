//! The dataset as callers write it: vehicles (technicians) with shifts,
//! visits, and visit groups served by several vehicles together. Records
//! name each other by id; times are local date-times
//! (`YYYY-MM-DDTHH:MM[:SS]`); durations are minutes; locations are
//! `[latitude, longitude]`; an area is a list of polygons, each a list of
//! `[latitude, longitude]` corners.

use serde::{Deserialize, Serialize};
pub use solverforge_workbench_host::traffic::{Traffic, TravelPeriod};

/// A technician and their vehicle, with the shifts they work.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vehicle {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub shifts: Vec<VehicleShift>,
    /// Shifts of this vehicle that may also be worked as one, staying out
    /// overnight between them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub linked_shifts: Vec<LinkedShift>,
    /// Minutes worked and minutes available before this plan, for fairness.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub historical_time_utilized: i64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub historical_time_capacity: i64,
    /// Higher is better; the parameter `defaultTechnicianRating` when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub technician_rating: Option<f64>,
    /// Visits must lie inside; anywhere when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_area: Option<Area>,
    /// Visits should lie inside; anywhere when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_area: Option<Area>,
}

fn is_zero(value: &i64) -> bool {
    *value == 0
}

/// Polygons, each a ring of `[latitude, longitude]` corners.
pub type Area = Vec<Vec<[f64; 2]>>;

/// What a shift (or a linked shift) offers and how long it may work.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Limits {
    /// Hard limit on minutes driving from one place to the next visit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_travel_time_per_visit: Option<i64>,
    /// Limits on the shift's minutes of travel, hard and soft.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_travel_time: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_soft_travel_time: Option<i64>,
    /// Limits on the shift's minutes of service, hard and soft.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_service_duration: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_soft_service_duration: Option<i64>,
    /// Limits on the shift's number of visits, hard and soft.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_visits: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_soft_visits: Option<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<Skill>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_breaks: Vec<Break>,
    /// Skills and tags held only for part of the shift.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub temporary_skill_sets: Vec<TemporarySkills>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub temporary_tag_sets: Vec<TemporaryTags>,
    /// Share of the shift's working time movable visits may fill before
    /// they cost; 1 when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub movable_occupation_ratio_threshold: Option<f64>,
    /// What the shift costs when it serves any visit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<Cost>,
    /// Scales and pads every drive of the shift; the parameter
    /// `travelTimeAdjustment` when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub travel_time_adjustment: Option<TravelTimeAdjustment>,
    /// Places closer than this count as one place for co-located grouping;
    /// the parameter when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefer_grouping_colocated_visits_radius_meters: Option<i64>,
    /// Overrides the parameter `pinNextVisitDuringFreeze` for this shift.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin_next_visit_during_freeze: Option<bool>,
    /// Keep the whole itinerary as it is.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pinned: bool,
    /// Visits and breaks in driving order: the plan, once solved.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub itinerary: Vec<ItineraryItem>,
}

/// One working period of a vehicle.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VehicleShift {
    pub id: String,
    pub start_location: [f64; 2],
    /// Where the shift ends; the start location when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_location: Option<[f64; 2]>,
    /// Earliest time the technician leaves the start location.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_start_time: Option<String>,
    /// Earliest arrival at the first visit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_first_visit_arrival_time: Option<String>,
    /// Latest departure from the last visit, hard and soft.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_last_visit_departure_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_soft_last_visit_departure_time: Option<String>,
    /// Latest arrival at the end location, hard and soft.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_end_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_soft_end_time: Option<String>,
    /// Replaces the vehicle's areas for this shift.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_area: Option<Area>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_area: Option<Area>,
    #[serde(flatten)]
    pub limits: Limits,
}

/// Consecutive shifts of one vehicle worked as one: the technician stays
/// out overnight instead of going home. Either the linked shift or its
/// component shifts serve visits, never both.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedShift {
    pub id: String,
    /// Each link joins one shift to the next.
    pub shifts: Vec<ShiftLink>,
    #[serde(flatten)]
    pub limits: Limits,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShiftLink {
    pub id: String,
    pub from_shift_id: String,
    pub to_shift_id: String,
}

/// A skill a shift offers.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    pub name: String,
    /// Higher is better; 1 when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<i64>,
    /// Scales the part of a visit's service that needs this skill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multiplier: Option<f64>,
}

/// Skills held from `start` until `end`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporarySkills {
    pub start: String,
    pub end: String,
    pub skills: Vec<Skill>,
}

/// Tags held from `start` until `end`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporaryTags {
    pub start: String,
    pub end: String,
    pub tags: Vec<String>,
}

/// A break during a shift: fixed at a time, or floating inside a window.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Break {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: BreakKind,
    /// Where the break is taken; where the technician is when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<[f64; 2]>,
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

/// What a shift costs: a fixed amount once it serves any visit, then rates
/// that apply one after another from the shift's start.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cost {
    #[serde(default)]
    pub fixed_cost: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rates: Vec<Rate>,
}

/// A rate for `duration` minutes after the previous rate ends; the last
/// rate (or one without a duration) runs to the shift's end.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<i64>,
    /// Charged once the rate starts to apply.
    #[serde(default)]
    pub activation_cost: f64,
    #[serde(default)]
    pub cost_per_unit: f64,
    #[serde(default)]
    pub unit: RateUnit,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RateUnit {
    Minute,
    #[default]
    Hour,
}

/// Drive time as `multiplier` times the plain drive, plus `extraTime`
/// minutes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TravelTimeAdjustment {
    pub multiplier: f64,
    pub extra_time: i64,
}

impl Default for TravelTimeAdjustment {
    fn default() -> Self {
        Self {
            multiplier: 1.0,
            extra_time: 0,
        }
    }
}

/// One visit or break of an itinerary.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItineraryItem {
    pub id: String,
    pub kind: ItemKind,
    /// The start of service a pinned visit keeps, or the planned start a
    /// replan compares against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_service_time: Option<String>,
    /// Keep this visit on this shift, starting at `startServiceTime`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pin: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ItemKind {
    #[default]
    Visit,
    Break,
}

/// A place a technician serves.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Visit {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub location: [f64; 2],
    /// Service minutes on site.
    pub service_duration: i64,
    /// Minutes of the service per skill; a matching skill's multiplier
    /// scales its part.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub service_duration_breakdown: Vec<ServicePart>,
    /// Minutes added to (or, negative, taken off) the service.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub service_duration_adjustment: i64,
    /// Minutes added when the visit before it was not at the same place;
    /// the parameter when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_added_for_first_visit_on_location: Option<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub time_windows: Vec<TimeWindow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preferred_time_windows: Vec<TimeWindow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_skills: Vec<SkillRequirement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_vehicles: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prohibited_vehicles: Vec<String>,
    /// Preferred vehicles, each weight 1.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preferred_vehicles: Vec<String>,
    /// Preferred vehicles with their own weights.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preferred_vehicles_weights: Vec<PreferredVehicle>,
    /// `1`…`10` mandatory, `opt-1`…`opt-10` optional, or a name from
    /// `priorityWeights`; the parameter `defaultPriority` when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub visit_dependencies: Vec<VisitDependency>,
    /// The service should end by then.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_sla_end_time: Option<String>,
    /// Visits sharing this id should run one after another on one shift.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_consecutive_group_id: Option<String>,
    /// Visits that may not be planned together with this one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclusions: Vec<String>,
    /// Earned when the visit is planned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profit: Option<f64>,
    /// The technician may not set off towards this visit earlier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_start_travel_time: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServicePart {
    pub skill: String,
    pub duration: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreferredVehicle {
    pub id: String,
    #[serde(default = "one")]
    pub weight: i64,
}

fn one() -> i64 {
    1
}

/// A visit that waits on another one.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisitDependency {
    pub id: String,
    pub preceding_visit: String,
    /// Minutes from the preceding visit's departure to this visit's start:
    /// at least `minDelay`, at most `maxDelay`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_delay: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_delay: Option<i64>,
    /// No earlier than this time of a later day.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_delay_to: Option<DelayTo>,
    /// No later than this time of a later day.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_delay_to: Option<DelayTo>,
    #[serde(default)]
    pub coordination: Coordination,
    /// The parameter `defaultDependencyPoolingType` when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pooling_type: Option<PoolingType>,
}

/// `increment` days (or working days) after the preceding visit's
/// departure, at `time` (`HH:MM`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DelayTo {
    #[serde(default)]
    pub adjuster: DateAdjuster,
    #[serde(default = "one")]
    pub increment: i64,
    #[serde(default = "midnight")]
    pub time: String,
}

fn midnight() -> String {
    "00:00".into()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DateAdjuster {
    #[default]
    NextDay,
    /// Skips weekends.
    NextWorkingDay,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Coordination {
    #[default]
    None,
    SameVehicle,
    SameShift,
}

/// What may run between a preceding visit and the visit that waits on it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PoolingType {
    /// Anything.
    #[default]
    AllowPoolingAll,
    /// Only visits that take part in no dependency.
    AllowPoolingVisits,
    /// Only visits of other dependencies.
    AllowPoolingDependencies,
    /// Nothing: the visit follows its preceding visit right away.
    DisallowPooling,
}

/// Visits served by several vehicles at the same time.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisitGroup {
    pub id: String,
    #[serde(default)]
    pub alignment: Alignment,
    /// At least two.
    pub visits: Vec<Visit>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Alignment {
    /// Every vehicle starts its service together.
    #[default]
    Start,
    /// Every vehicle ends its service together.
    End,
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
    /// The travel of each busy period (`traffic`): measured on the roads for
    /// a period with slowdowns, brought by the dataset with its own matrix.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub periods: Vec<TravelPeriod>,
}
