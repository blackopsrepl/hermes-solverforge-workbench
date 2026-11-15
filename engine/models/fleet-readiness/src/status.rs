//! The fleet's state day by day, read straight from a plan: which units are
//! ready, and how each readiness rule fares. Shared by the KPIs and the view.

use crate::derived::Derived;
use crate::domain::Plan;

/// Each day of the horizon: whether each unit is ready.
pub struct Fleet<'a> {
    pub d: &'a Derived,
    /// `ready[day][unit]`.
    pub ready: Vec<Vec<bool>>,
    /// `held[day][unit]`: kept down by the plan (not merely away).
    pub held: Vec<Vec<bool>>,
}

impl<'a> Fleet<'a> {
    /// `None` for a plan `prepare` has not derived.
    pub fn of(plan: &'a Plan) -> Option<Self> {
        let d: &Derived = &plan.days.first()?.ctx;
        let (days, units) = (d.days as usize, d.units.len());
        let mut held = vec![vec![false; units]; days];
        for task in &plan.tasks {
            let (first, end) = d.down(task.index, task.slot_idx);
            let unit = d.tasks[task.index].unit;
            for day in first..end {
                held[day as usize][unit] = true;
            }
        }
        let ready = (0..days)
            .map(|day| {
                (0..units)
                    .map(|unit| !held[day][unit] && !d.away(unit, day as i64))
                    .collect()
            })
            .collect();
        Some(Self { d, ready, held })
    }

    /// Ready units on `day`.
    pub fn ready_count(&self, day: usize) -> usize {
        self.ready[day].iter().filter(|r| **r).count()
    }

    /// Ready units each rule counts on `day`, with the rule's index, for the
    /// rules that apply that day.
    pub fn by_rule(&self, day: usize) -> Vec<(usize, i64)> {
        self.d
            .rules
            .iter()
            .enumerate()
            .filter(|(_, rule)| rule.applies(day as i64))
            .map(|(r, rule)| {
                let ready = self.ready[day]
                    .iter()
                    .enumerate()
                    .filter(|(u, ready)| **ready && rule.counts(self.d.units[*u].fleet))
                    .count() as i64;
                (r, ready)
            })
            .collect()
    }

    /// Whether some rule is below its minimum on `day`.
    pub fn below_minimum(&self, day: usize) -> bool {
        self.by_rule(day)
            .iter()
            .any(|&(r, ready)| ready < self.d.rules[r].minimum)
    }

    /// Whether some rule is below its target on `day`.
    pub fn below_target(&self, day: usize) -> bool {
        self.by_rule(day)
            .iter()
            .any(|&(r, ready)| ready < self.d.rules[r].target)
    }
}
