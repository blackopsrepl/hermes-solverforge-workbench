//! How many units operations need each day.

use serde::{Deserialize, Serialize};

use super::common::is_zero;

/// Units that must be ready each day: of one fleet type, or of the whole fleet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadinessRule {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Fleet type the rule counts; every unit when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fleet: Option<String>,
    /// Ready units operations cannot do without (hard).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub minimum: i64,
    /// Ready units operations plan for (soft); at least `minimum`.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub target: i64,
    /// First day the rule applies; the horizon start when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// Last day the rule applies, included; the horizon end when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
}
