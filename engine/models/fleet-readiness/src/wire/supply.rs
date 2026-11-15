//! Spare parts and when they arrive.

use serde::{Deserialize, Serialize};

/// A spare part or kit tasks consume.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Part {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Stock at the start of the horizon.
    #[serde(default)]
    pub on_hand: i64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deliveries: Vec<Delivery>,
}

/// Parts that arrive on a day, usable from that day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Delivery {
    pub date: String,
    pub quantity: i64,
}
