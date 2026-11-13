use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

use crate::derived::Ctx;
use crate::wire::SkillRef;

/// A stretch of work that needs one employee.
#[planning_entity]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift {
    /// Position in the plan's shift list; set by `prepare`.
    #[planning_id]
    #[serde(skip)]
    pub index: usize,
    pub id: String,
    /// Inclusive start, `YYYY-MM-DDTHH:MM`.
    pub start: String,
    /// Exclusive end, `YYYY-MM-DDTHH:MM`.
    pub end: String,
    /// Id of the assigned employee, if any.
    #[serde(default)]
    pub employee: Option<String>,
    /// Keep `employee` as given; the solver will not change it.
    #[planning_pin]
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pinned: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Skills the employee must hold.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_skills: Vec<SkillRef>,
    /// With `true`, any one of `required_skills` is enough.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub any_required_skill: bool,
    /// Skills the employee should hold.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preferred_skills: Vec<SkillRef>,
    /// Risk factors the shift exposes its employee to.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub risks: Vec<String>,
    /// An optional shift may stay open at a soft cost; a mandatory one costs medium score.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub optional: bool,
    /// How much leaving the shift open costs; 1 by default.
    #[serde(default = "crate::wire::one", skip_serializing_if = "crate::wire::is_one")]
    pub importance: i64,
    /// Employees the shift would like.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preferred_employees: Vec<String>,
    /// Employees the shift would rather not have.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unwanted_employees: Vec<String>,
    /// Employees who must never work the shift.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub barred_employees: Vec<String>,
    /// The assigned employee as an index into the plan's employees.
    #[planning_variable(value_range_provider = "employees", allows_unassigned = true)]
    #[serde(skip)]
    pub employee_idx: Option<usize>,
    #[serde(skip)]
    pub ctx: Ctx,
}

impl Shift {
    /// An open shift with only an id and its times.
    pub fn new(id: impl Into<String>, start: impl Into<String>, end: impl Into<String>) -> Self {
        Self {
            index: 0,
            id: id.into(),
            start: start.into(),
            end: end.into(),
            employee: None,
            pinned: false,
            tags: Vec::new(),
            required_skills: Vec::new(),
            any_required_skill: false,
            preferred_skills: Vec::new(),
            risks: Vec::new(),
            optional: false,
            importance: 1,
            preferred_employees: Vec::new(),
            unwanted_employees: Vec::new(),
            barred_employees: Vec::new(),
            employee_idx: None,
            ctx: Ctx::default(),
        }
    }
}
