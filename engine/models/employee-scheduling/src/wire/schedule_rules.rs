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
