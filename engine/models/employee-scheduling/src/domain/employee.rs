use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

use crate::derived::Ctx;
use crate::wire::{LoggedMinutes, Pairing, Place, SkillRef, TagFilter, TimeSpan};

/// Someone who can be given shifts.
#[problem_fact]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Employee {
    /// Position in the plan's employee list; set by `prepare`.
    #[planning_id]
    #[serde(skip)]
    pub index: usize,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Labels used by rule filters, pairings and team rules.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Multiplies the employee's soft preferences and rule penalties; 1 by default.
    #[serde(default = "crate::wire::one", skip_serializing_if = "crate::wire::is_one")]
    pub priority: i64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<SkillRef>,
    /// Risk factors this employee must never be exposed to.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub avoid_risks: Vec<String>,
    /// Times the employee cannot work.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unavailable: Vec<TimeSpan>,
    /// When present, the only times the employee can work (for matching shifts).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub available_only: Vec<TimeSpan>,
    /// Times the employee would like to work.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preferred_times: Vec<TimeSpan>,
    /// Times the employee would rather not work.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unwanted_times: Vec<TimeSpan>,
    /// Shifts the employee may work: others cost hard score.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_shifts: Option<TagFilter>,
    /// Shifts the employee likes: each one worked is rewarded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_shifts: Option<TagFilter>,
    /// Colleagues to work with, or not.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pairings: Vec<Pairing>,
    /// Where the employee travels from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home: Option<Place>,
    /// Ids of the contracts whose rules apply.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contracts: Vec<String>,
    /// Requested days off, `YYYY-MM-DD`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub days_off: Vec<String>,
    /// Minutes logged before the data, for logged-time rules.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub logged_before: Vec<LoggedMinutes>,
    #[serde(skip)]
    pub ctx: Ctx,
}

impl Employee {
    /// An employee with only an id.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            index: 0,
            id: id.into(),
            name: None,
            tags: Vec::new(),
            priority: 1,
            skills: Vec::new(),
            avoid_risks: Vec::new(),
            unavailable: Vec::new(),
            available_only: Vec::new(),
            preferred_times: Vec::new(),
            unwanted_times: Vec::new(),
            required_shifts: None,
            preferred_shifts: None,
            pairings: Vec::new(),
            home: None,
            contracts: Vec::new(),
            days_off: Vec::new(),
            logged_before: Vec::new(),
            ctx: Ctx::default(),
        }
    }
}
