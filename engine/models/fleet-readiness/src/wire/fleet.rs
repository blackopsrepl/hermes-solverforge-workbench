//! The units of the fleet: aircraft, vehicles or vessels.

use serde::{Deserialize, Serialize};

use super::common::{is_one, is_zero, one, Span};

/// One unit of the fleet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Unit {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Fleet type, for bay fit and readiness rules (`A320`, `ATR72`…).
    pub fleet: String,
    /// How much the unit matters to operations; 1 by default. Multiplies the
    /// cost of leaving its work unplanned.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub priority: i64,
    /// What one day out of service costs, in currency units.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub downtime_cost: i64,
    /// First day the unit is at the operator's disposal (back from lease,
    /// deployment or long-term repair); unavailable before it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub available_from: Option<String>,
    /// The unit's usage counter, for tasks due at a usage limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    /// Days the unit is committed to operations and cannot be in the workshop.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commitments: Vec<Span>,
}

impl Unit {
    pub fn new(id: impl Into<String>, fleet: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: None,
            fleet: fleet.into(),
            priority: 1,
            downtime_cost: 0,
            available_from: None,
            usage: None,
            commitments: Vec::new(),
        }
    }
}

/// A usage counter (flight hours, cycles, kilometres) and how fast it grows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Usage {
    /// Counter at the start of the horizon.
    pub current: f64,
    /// Growth per day the unit operates.
    pub per_day: f64,
}
