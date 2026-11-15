//! Every rule scored over one timing of the plan.
//!
//! [`evaluate`] reports each match to a [`Sink`]: the solver's hot path only
//! sums the amounts, analysis keeps the matches and the records they blame.
//! Both read the same code, so explanations always add up to the score.

use super::Rule;
use crate::domain::Plan;
use crate::schedule::{Timing, UNASSIGNED};
use crate::time::overlap;

/// A record a match blames, by index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blame {
    Task(usize),
    Resource(usize),
}

/// Receives every match: its rule, its positive amount (minutes, points or
/// reward) and the records it blames.
pub trait Sink {
    fn hit(&mut self, rule: Rule, amount: i64, blame: &[Blame]);
}

/// Scores every rule of `plan` under `timing` into `sink`.
pub fn evaluate(plan: &Plan, timing: &Timing, sink: &mut impl Sink) {
    let facts = &*plan.facts;
    let link = facts.broken_link;
    for (t, tf) in facts.tasks.iter().enumerate() {
        let task = Blame::Task(t);
        let r = timing.resource[t];
        if r == UNASSIGNED {
            sink.hit(Rule::Unassigned, tf.priority, &[task]);
            continue;
        }
        let rf = &facts.resources[r];
        let resource = Blame::Resource(r);
        let (start, end) = (timing.start[t], timing.end[t]);
        if let Some(min_start) = tf.min_start {
            if start < min_start {
                sink.hit(Rule::EarliestStart, min_start - start, &[task]);
            }
        }
        if let Some(max_end) = tf.max_end {
            if end > max_end {
                sink.hit(Rule::Deadline, end - max_end, &[task]);
            }
        }
        let missing: i64 = tf
            .required_tags
            .iter()
            .map(|&tag| match rf.tags[tag] {
                None => (end - start).max(1),
                Some((from, until)) => {
                    let uncovered = (end - start) - overlap(start, end, from, until);
                    if uncovered > 0 || (end == start && !(from..until).contains(&start)) {
                        uncovered.max(1)
                    } else {
                        0
                    }
                }
            })
            .sum();
        if missing > 0 {
            sink.hit(Rule::TagMissing, missing, &[task, resource]);
        }
        if tf.allowed.as_ref().is_some_and(|allowed| !allowed[r]) {
            sink.hit(Rule::ResourceNotAllowed, tf.priority, &[task, resource]);
        }
        if let Some(ideal) = tf.ideal_end {
            if end != ideal {
                sink.hit(Rule::IdealEnd, (end - ideal).abs() * tf.priority, &[task]);
            }
        }
        if let Some(due) = tf.due {
            if end > due {
                sink.hit(Rule::Lateness, end - due, &[task]);
            }
        }
        if let Some(changeover) = timing.changeover[t] {
            let p = timing.position[t];
            let before = (p > 0).then(|| Blame::Task(timing.order[r][p - 1]));
            let blamed: &[Blame] = match &before {
                Some(previous) => &[*previous, task, resource],
                None => &[task, resource],
            };
            sink.hit(Rule::ChangeoverCount, 1, blamed);
            if changeover.minutes > 0 {
                sink.hit(Rule::ChangeoverTime, changeover.minutes, blamed);
            }
        }
        if timing.idle[t] > 0 {
            sink.hit(Rule::IdleTime, timing.idle[t], &[task, resource]);
        }
        for dep in &tf.deps {
            let pred = Blame::Task(dep.task);
            let rp = timing.resource[dep.task];
            if rp == UNASSIGNED {
                sink.hit(Rule::DependencyUnassigned, link, &[task, pred]);
                continue;
            }
            let anchor = if dep.from_end {
                timing.end[dep.task]
            } else {
                timing.start[dep.task]
            };
            let early = (anchor + dep.min_offset - start).max(0);
            let late = dep
                .max_offset
                .map_or(0, |max| (start - anchor - max).max(0));
            if early + late > 0 {
                sink.hit(Rule::DependencyTiming, early + late, &[task, pred]);
            }
            if dep.same_resource && rp != r {
                sink.hit(Rule::DependencySplit, link, &[task, pred]);
            }
            if dep.no_intermediate && rp == r {
                let between = timing.position[t].abs_diff(timing.position[dep.task]) as i64 - 1;
                if between > 0 {
                    sink.hit(Rule::DependencyInterrupted, link * between, &[task, pred]);
                }
            }
            let hand_over = facts.transition(rp, r);
            let ends = [pred, Blame::Resource(rp), task, resource];
            if hand_over.forbidden {
                sink.hit(Rule::ForbiddenHandOver, link, &ends);
            }
            if hand_over.reward != 0 {
                sink.hit(Rule::PreferredHandOver, hand_over.reward, &ends);
            }
        }
    }
    let mut longest: Option<(i64, usize)> = None;
    for (r, order) in timing.order.iter().enumerate() {
        let Some(last) = order.iter().map(|&t| timing.end[t]).max() else {
            continue;
        };
        let span = last - facts.resources[r].start;
        if longest.is_none_or(|(best, _)| span > best) {
            longest = Some((span, r));
        }
    }
    if let Some((span, r)) = longest {
        if span > 0 {
            sink.hit(Rule::Makespan, span, &[Blame::Resource(r)]);
        }
    }
}
