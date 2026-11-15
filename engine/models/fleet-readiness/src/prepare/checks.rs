//! Reading dates and spans and checking ids, with every problem reported.

use std::collections::HashSet;

use solverforge_workbench_host::Issue;

use crate::time::parse_date;
use crate::wire::Span;

/// Collects issues while records are read; `origin` is the horizon start.
pub struct Checks {
    pub issues: Vec<Issue>,
    pub origin: i64,
    pub days: i64,
}

impl Checks {
    pub fn error(&mut self, code: &'static str, message: String) {
        self.issues.push(Issue::error(code, message));
    }

    pub fn warning(&mut self, code: &'static str, message: String) {
        self.issues.push(Issue::warning(code, message));
    }

    pub fn has_errors(&self) -> bool {
        self.issues
            .iter()
            .any(|issue| issue.severity == solverforge_workbench_host::Severity::Error)
    }

    /// A `YYYY-MM-DD` date as a horizon day (may lie outside the horizon).
    pub fn day(&mut self, text: &str, what: impl FnOnce() -> String) -> Option<i64> {
        match parse_date(text) {
            Some(days) => Some(days - self.origin),
            None => {
                self.error(
                    "invalid_date",
                    format!("{} is not a date YYYY-MM-DD: {text:?}", what()),
                );
                None
            }
        }
    }

    /// An inclusive span as a half-open range of horizon days.
    pub fn span(&mut self, span: &Span, what: &str) -> Option<(i64, i64)> {
        let from = self.day(&span.from, || format!("{what} from"))?;
        let to = self.day(&span.to, || format!("{what} to"))?;
        if to < from {
            self.error(
                "invalid_span",
                format!("{what} ends ({}) before it starts ({})", span.to, span.from),
            );
            return None;
        }
        Some((from, to + 1))
    }

    /// Reports ids that appear more than once among `ids`.
    pub fn unique<'a>(&mut self, kind: &str, ids: impl Iterator<Item = &'a str>) {
        let mut seen = HashSet::new();
        for id in ids {
            if !seen.insert(id) {
                self.error("duplicate_id", format!("{kind} id {id:?} is used twice"));
            }
        }
    }

    /// Reports a negative amount.
    pub fn not_negative(&mut self, value: i64, what: impl FnOnce() -> String) {
        if value < 0 {
            self.error(
                "negative_value",
                format!("{} must not be negative: {value}", what()),
            );
        }
    }
}
