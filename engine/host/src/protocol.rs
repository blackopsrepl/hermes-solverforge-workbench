//! Requests read from stdin and the error envelope.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use solverforge::config::TerminationConfig;

/// What the engine is asked to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    /// Constraints, levels, demos and KPIs of the model.
    Describe,
    /// One demo dataset.
    Demo,
    /// Solve a plan, streaming progress until it finishes or is cancelled.
    Solve,
    /// Score a plan as it stands, without solving it.
    Evaluate,
    /// The plan drawn for people.
    View,
    /// Ranked placements for open items, or one applied.
    Fill,
}

/// One request, read as the first line of stdin.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub command: Command,
    pub model: String,
    /// Demo id for `demo`; defaults to the model's first demo.
    #[serde(default)]
    pub demo: Option<String>,
    /// The plan for `solve` and `evaluate`.
    #[serde(default)]
    pub plan: Option<Value>,
    /// Constraint weight multipliers by constraint name.
    #[serde(default)]
    pub weights: BTreeMap<String, u32>,
    /// Stop conditions for `solve`; the model's own apply when absent.
    #[serde(default)]
    pub termination: Option<Termination>,
    /// Most matches reported per constraint.
    #[serde(default = "default_max_matches")]
    pub max_matches: usize,
    /// Pin everything that starts before this local date-time.
    #[serde(default)]
    pub freeze_before: Option<String>,
    /// What to fill, for `fill`.
    #[serde(default)]
    pub fill: Option<crate::fill::FillRequest>,
}

fn default_max_matches() -> usize {
    100
}

/// Stop conditions for one solve.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Termination {
    pub seconds: Option<u64>,
    pub unimproved_seconds: Option<u64>,
    pub step_count: Option<u64>,
    pub unimproved_step_count: Option<u64>,
    pub best_score: Option<String>,
}

impl Termination {
    pub fn to_config(&self) -> TerminationConfig {
        TerminationConfig {
            seconds_spent_limit: self.seconds,
            unimproved_seconds_spent_limit: self.unimproved_seconds,
            step_count_limit: self.step_count,
            unimproved_step_count_limit: self.unimproved_step_count,
            best_score_limit: self.best_score.clone(),
            ..TerminationConfig::default()
        }
    }
}

/// A refusal or failure, written as the last line before a non-zero exit.
#[derive(Debug, Serialize)]
pub struct EngineError {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Value::is_null")]
    pub detail: Value,
}

impl EngineError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            detail: Value::Null,
        }
    }

    pub fn with_detail(mut self, detail: Value) -> Self {
        self.detail = detail;
        self
    }
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
