//! Rules over the whole schedule rather than one employee.

use serde::{Deserialize, Serialize};

use super::{Level, TagFilter};

/// Schedule-wide rules; every list is optional.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScheduleRules {
    /// Assigned shifts per period across all employees.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub shift_counts: Vec<CountRule>,
    /// Distinct tags of some types staffed per period across all shifts.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub distinct_tags: Vec<DistinctTagsRule>,
    /// Most matching shifts worked at the same moment.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub concurrency: Vec<ConcurrencyRule>,
    /// Cost of assignments per period across employees.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub costs: Vec<CostRule>,
    /// Staff wanted per time slot.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub demand: Vec<DemandRule>,
    /// Spread work evenly over employees.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fairness: Vec<FairnessRule>,
}

/// The cost of assignments in a period must stay in `[min, max]`, in
/// currency units; penalties count cents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CostRule {
    pub period: String,
    #[serde(default)]
    pub level: Level,
    #[serde(default = "super::one", skip_serializing_if = "super::is_one")]
    pub weight: i64,
    #[serde(default, flatten)]
    pub shifts: TagFilter,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub employees: Option<TagFilter>,
    /// Hourly rates by employee and shift cost group; the first match applies.
    pub rates: Vec<Rate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<i64>,
}

impl Eq for CostRule {}

impl std::hash::Hash for CostRule {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.period.hash(state);
        self.rates.len().hash(state);
    }
}

/// An hourly rate for an employee cost group working a shift cost group;
/// a missing group matches any.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub employee_group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shift_group: Option<String>,
    pub hourly: f64,
}

/// Matching assigned shifts covering each slot must stay in `[min, max]`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DemandRule {
    #[serde(default)]
    pub level: Level,
    #[serde(default, flatten)]
    pub shifts: TagFilter,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub employees: Option<TagFilter>,
    pub slots: Vec<DemandSlot>,
}

/// A stretch of time (at most a day long) and the staff it needs; a shift
/// covers it when it spans the whole slot.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DemandSlot {
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<i64>,
}

/// What a fairness rule balances.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Balance {
    Shifts,
    Minutes,
}

/// Balance shifts or minutes over the matching employees, counting what
/// each already worked (`history`, by employee id).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FairnessRule {
    pub balance: Balance,
    #[serde(default, flatten)]
    pub shifts: TagFilter,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub employees: Option<TagFilter>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub history: std::collections::BTreeMap<String, i64>,
}

impl std::hash::Hash for FairnessRule {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.balance.hash(state);
        self.history.iter().for_each(|entry| entry.hash(state));
    }
}

/// Assigned matching shifts per period must stay in `[min, max]`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CountRule {
    pub period: String,
    #[serde(default)]
    pub level: Level,
    #[serde(default = "super::one", skip_serializing_if = "super::is_one")]
    pub weight: i64,
    #[serde(default, flatten)]
    pub shifts: TagFilter,
    /// Only shifts worked by employees passing this filter count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub employees: Option<TagFilter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<i64>,
}

/// Distinct tags of `tag_types` on assigned matching shifts per period must
/// stay in `[min, max]`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DistinctTagsRule {
    pub period: String,
    #[serde(default)]
    pub level: Level,
    #[serde(default = "super::one", skip_serializing_if = "super::is_one")]
    pub weight: i64,
    pub tag_types: Vec<String>,
    #[serde(default, flatten)]
    pub shifts: TagFilter,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<i64>,
}

/// At most `max` matching shifts may run at the same moment, between `from`
/// and `to` when given.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConcurrencyRule {
    pub max: i64,
    #[serde(default, flatten)]
    pub shifts: TagFilter,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
}
