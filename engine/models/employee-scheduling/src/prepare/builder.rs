//! Turns string ids and ISO times into the derived fact tables, collecting
//! every problem found on the way.

use std::collections::HashMap;

use solverforge_workbench_host::Issue;

use crate::derived::{Grant, Need, Span, TagMatch, TagSet};
use crate::time::{parse_date, parse_date_time, DAY};
use crate::wire::{ShiftFilter, SkillRef, TimeSpan};

/// State shared by the preparation passes.
pub struct Builder {
    pub issues: Vec<Issue>,
    names: HashMap<String, u32>,
    pub name_list: Vec<String>,
    /// Plan minute 0 as minutes since 1970-01-01.
    pub origin_minutes: i64,
    pub employee_ids: HashMap<String, usize>,
    pub shift_ids: HashMap<String, usize>,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            issues: Vec::new(),
            names: HashMap::new(),
            name_list: Vec::new(),
            origin_minutes: 0,
            employee_ids: HashMap::new(),
            shift_ids: HashMap::new(),
        }
    }

    pub fn error(&mut self, code: &'static str, message: String) {
        self.issues.push(Issue::error(code, message));
    }

    pub fn warning(&mut self, code: &'static str, message: String) {
        self.issues.push(Issue::warning(code, message));
    }

    /// Id of a tag, skill or risk name.
    pub fn intern(&mut self, name: &str) -> u32 {
        if let Some(&id) = self.names.get(name) {
            return id;
        }
        let id = self.name_list.len() as u32;
        self.name_list.push(name.to_owned());
        self.names.insert(name.to_owned(), id);
        id
    }

    pub fn tag_set(&mut self, names: &[String]) -> TagSet {
        let mut set: TagSet = names.iter().map(|name| self.intern(name)).collect();
        set.sort_unstable();
        set.dedup();
        set
    }

    pub fn tag_match(&mut self, filter: &ShiftFilter) -> TagMatch {
        TagMatch {
            with: self.tag_set(&filter.with_tags),
            without: self.tag_set(&filter.without_tags),
            all: filter.match_all,
        }
    }

    /// Plan minutes of an ISO local date-time; reports `what` when malformed.
    pub fn minutes(&mut self, text: &str, what: impl FnOnce() -> String) -> Option<i64> {
        match parse_date_time(text) {
            Some(minutes) => Some(minutes - self.origin_minutes),
            None => {
                self.error(
                    "invalid_date_time",
                    format!(
                        "{} has date-time {text:?}; expected YYYY-MM-DDTHH:MM",
                        what()
                    ),
                );
                None
            }
        }
    }

    /// Plan day of an ISO local date; reports `what` when malformed.
    pub fn day(&mut self, text: &str, what: impl FnOnce() -> String) -> Option<i64> {
        match parse_date(text) {
            Some(days) => Some(days - self.origin_minutes.div_euclid(DAY)),
            None => {
                self.error(
                    "invalid_date",
                    format!("{} has date {text:?}; expected YYYY-MM-DD", what()),
                );
                None
            }
        }
    }

    pub fn spans(&mut self, spans: &[TimeSpan], what: &str) -> Vec<Span> {
        let mut out = Vec::with_capacity(spans.len());
        for (n, span) in spans.iter().enumerate() {
            let label = || format!("{what} #{}", n + 1);
            let start = self.minutes(&span.from, label);
            let end = self.minutes(&span.to, label);
            let (Some(start), Some(end)) = (start, end) else {
                continue;
            };
            if end <= start {
                self.error(
                    "span_not_after_start",
                    format!("{} ends at or before it starts", label()),
                );
                continue;
            }
            if span.weight < 0 {
                self.error(
                    "negative_weight",
                    format!("{} has a negative weight", label()),
                );
            }
            let shifts = self.tag_match(&span.shifts);
            out.push(Span {
                start,
                end,
                shifts,
                weight: span.weight,
            });
        }
        out
    }

    pub fn grants(&mut self, skills: &[SkillRef], what: &str) -> Vec<Grant> {
        let mut out = Vec::with_capacity(skills.len());
        for skill in skills {
            let (mut from, mut to) = (i64::MIN, i64::MAX);
            if let SkillRef::Detailed(detail) = skill {
                if let Some(text) = &detail.from {
                    from = self
                        .minutes(text, || format!("{what} skill {}", detail.id))
                        .unwrap_or(from);
                }
                if let Some(text) = &detail.to {
                    to = self
                        .minutes(text, || format!("{what} skill {}", detail.id))
                        .unwrap_or(to);
                }
            }
            let skill_id = self.intern(skill.id());
            out.push(Grant {
                skill: skill_id,
                level: skill.level(),
                from,
                to,
            });
        }
        out
    }

    pub fn needs(&mut self, skills: &[SkillRef]) -> Vec<Need> {
        skills
            .iter()
            .map(|skill| Need {
                skill: self.intern(skill.id()),
                level: skill.level(),
            })
            .collect()
    }

    /// Sorted employee indexes of `ids`; reports unknown ids.
    pub fn employee_list(&mut self, ids: &[String], what: impl Fn() -> String) -> Vec<u32> {
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            match self.employee_ids.get(id) {
                Some(&index) => out.push(index as u32),
                None => self.error(
                    "unknown_employee",
                    format!("{} names employee {id:?}, which does not exist", what()),
                ),
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}
