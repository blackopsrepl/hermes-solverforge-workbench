//! The constraint set: seventeen named constraints over one shared timing.
//!
//! The director retracts and inserts resources around every change. The set
//! defers retractions and, on each insert, re-times the plan and returns the
//! change of every rule's total since the last insert. Deltas telescope, so
//! the score stays exact whatever order the director notifies resources in,
//! including moves that touch two resources.

use solverforge::analysis::{
    ConstraintAnalysis, ConstraintJustification, DetailedConstraintMatch, EntityRef,
};
use solverforge::prelude::*;
use solverforge::Score as _;
use solverforge::{ConstraintMetadata, ConstraintResult};

use super::rules::{evaluate, Blame, Sink};
use super::{Rule, RULES};
use crate::domain::Plan;
use crate::names;
use crate::schedule::Timing;

type Score = HardMediumSoftScore;

/// Entity descriptor of the resources, the list owners.
const RESOURCES: usize = 0;

/// Every constraint of the model.
pub struct TaskConstraints {
    refs: Vec<ConstraintRef>,
    state: Option<State>,
}

struct State {
    timing: Timing,
    totals: [i64; 17],
}

/// Sums amounts per rule.
#[derive(Default)]
struct Totals {
    amounts: [i64; 17],
    counts: [usize; 17],
}

impl Sink for Totals {
    #[inline]
    fn hit(&mut self, rule: Rule, amount: i64, _blame: &[Blame]) {
        self.amounts[rule.index()] += amount;
        self.counts[rule.index()] += 1;
    }
}

/// Keeps every match with the records it blames.
struct Matches<'p> {
    plan: &'p Plan,
    found: Vec<Vec<(i64, ConstraintJustification)>>,
}

impl Sink for Matches<'_> {
    fn hit(&mut self, rule: Rule, amount: i64, blame: &[Blame]) {
        let entities: Vec<EntityRef> = blame
            .iter()
            .map(|blamed| {
                let (type_name, index) = match *blamed {
                    Blame::Task(index) => ("Task", index),
                    Blame::Resource(index) => ("Resource", index),
                };
                EntityRef {
                    type_name: type_name.to_string(),
                    display: names::label(self.plan, type_name, index),
                    planning_id: Some(index.to_string()),
                }
            })
            .collect();
        self.found[rule.index()].push((amount, ConstraintJustification::new(entities)));
    }
}

impl Default for TaskConstraints {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskConstraints {
    pub fn new() -> Self {
        Self {
            refs: RULES
                .iter()
                .map(|rule| ConstraintRef::new("", rule.name()))
                .collect(),
            state: None,
        }
    }

    fn totals(plan: &Plan) -> Totals {
        let mut totals = Totals::default();
        evaluate(plan, &Timing::of(plan), &mut totals);
        totals
    }

    fn sum(amounts: &[i64; 17], multipliers: Option<&[u32]>) -> Score {
        RULES.iter().fold(Score::ZERO, |sum, rule| {
            let score = rule.score(amounts[rule.index()]);
            match multipliers {
                Some(weights) => sum + score.scale(i64::from(weights[rule.index()])),
                None => sum + score,
            }
        })
    }

    fn results<'a>(
        &'a self,
        plan: &Plan,
        multipliers: Option<&[u32]>,
    ) -> Vec<ConstraintResult<'a, Score>> {
        let totals = Self::totals(plan);
        RULES
            .iter()
            .map(|rule| {
                let i = rule.index();
                let weight = multipliers.map_or(1, |weights| weights[i]);
                ConstraintResult {
                    name: &self.refs[i].name,
                    score: rule.score(totals.amounts[i]).scale(i64::from(weight)),
                    match_count: totals.counts[i],
                    is_hard: rule.level() == solverforge_workbench_host::Level::Hard,
                    weight_multiplier: weight,
                }
            })
            .collect()
    }

    fn detailed<'a>(
        &'a self,
        plan: &Plan,
        multipliers: Option<&[u32]>,
    ) -> Vec<ConstraintAnalysis<'a, Score>> {
        let timing = Timing::of(plan);
        let mut matches = Matches {
            plan,
            found: vec![Vec::new(); RULES.len()],
        };
        evaluate(plan, &timing, &mut matches);
        RULES
            .iter()
            .zip(matches.found)
            .map(|(rule, found)| {
                let i = rule.index();
                let weight = i64::from(multipliers.map_or(1, |weights| weights[i]));
                let reference = &self.refs[i];
                let total: i64 = found.iter().map(|(amount, _)| amount).sum();
                let details = found
                    .into_iter()
                    .map(|(amount, justification)| {
                        DetailedConstraintMatch::new(
                            reference,
                            rule.score(amount).scale(weight),
                            justification,
                        )
                    })
                    .collect();
                ConstraintAnalysis::new(
                    reference,
                    weight as u32,
                    rule.score(total).scale(weight),
                    details,
                    rule.level() == solverforge_workbench_host::Level::Hard,
                )
            })
            .collect()
    }

    fn initialize(&mut self, plan: &Plan, multipliers: Option<&[u32]>) -> Score {
        let mut timing = Timing::default();
        timing.compute(plan);
        let mut totals = Totals::default();
        evaluate(plan, &timing, &mut totals);
        let score = Self::sum(&totals.amounts, multipliers);
        self.state = Some(State {
            timing,
            totals: totals.amounts,
        });
        score
    }

    fn insert(&mut self, plan: &Plan, descriptor: usize, multipliers: Option<&[u32]>) -> Score {
        if descriptor != RESOURCES {
            return Score::ZERO;
        }
        let Some(state) = self.state.as_mut() else {
            return Score::ZERO;
        };
        state.timing.compute(plan);
        let mut totals = Totals::default();
        evaluate(plan, &state.timing, &mut totals);
        let mut delta = [0i64; 17];
        for (i, slot) in delta.iter_mut().enumerate() {
            *slot = totals.amounts[i] - state.totals[i];
        }
        state.totals = totals.amounts;
        Self::sum(&delta, multipliers)
    }
}

impl ConstraintSet<Plan, Score> for TaskConstraints {
    fn evaluate_all(&self, plan: &Plan) -> Score {
        Self::sum(&Self::totals(plan).amounts, None)
    }

    fn constraint_count(&self) -> usize {
        RULES.len()
    }

    fn constraint_metadata_entries(&self) -> Vec<ConstraintMetadata<'_>> {
        RULES
            .iter()
            .map(|rule| {
                ConstraintMetadata::new(
                    &self.refs[rule.index()],
                    rule.level() == solverforge_workbench_host::Level::Hard,
                )
            })
            .collect()
    }

    fn evaluate_each<'a>(&'a self, plan: &Plan) -> Vec<ConstraintResult<'a, Score>> {
        self.results(plan, None)
    }

    fn evaluate_detailed<'a>(&'a self, plan: &Plan) -> Vec<ConstraintAnalysis<'a, Score>> {
        self.detailed(plan, None)
    }

    fn initialize_all(&mut self, plan: &Plan) -> Score {
        self.initialize(plan, None)
    }

    fn on_insert_all(&mut self, plan: &Plan, _entity: usize, descriptor: usize) -> Score {
        self.insert(plan, descriptor, None)
    }

    fn on_retract_all(&mut self, _plan: &Plan, _entity: usize, _descriptor: usize) -> Score {
        Score::ZERO
    }

    fn reset_all(&mut self) {
        self.state = None;
    }

    fn constraint_weight_refs<'a>(&'a self, refs: &mut Vec<&'a ConstraintRef>) {
        refs.extend(self.refs.iter());
    }

    fn evaluate_all_weighted(&self, plan: &Plan, multipliers: &[u32]) -> Score {
        Self::sum(&Self::totals(plan).amounts, Some(multipliers))
    }

    fn evaluate_each_weighted<'a>(
        &'a self,
        plan: &Plan,
        multipliers: &[u32],
    ) -> Vec<ConstraintResult<'a, Score>> {
        self.results(plan, Some(multipliers))
    }

    fn evaluate_detailed_weighted<'a>(
        &'a self,
        plan: &Plan,
        multipliers: &[u32],
    ) -> Vec<ConstraintAnalysis<'a, Score>> {
        self.detailed(plan, Some(multipliers))
    }

    fn initialize_all_weighted(&mut self, plan: &Plan, multipliers: &[u32]) -> Score {
        self.initialize(plan, Some(multipliers))
    }

    fn on_insert_all_weighted(
        &mut self,
        plan: &Plan,
        _entity: usize,
        descriptor: usize,
        multipliers: &[u32],
    ) -> Score {
        self.insert(plan, descriptor, Some(multipliers))
    }

    fn on_retract_all_weighted(
        &mut self,
        _plan: &Plan,
        _entity: usize,
        _descriptor: usize,
        _multipliers: &[u32],
    ) -> Score {
        Score::ZERO
    }
}
