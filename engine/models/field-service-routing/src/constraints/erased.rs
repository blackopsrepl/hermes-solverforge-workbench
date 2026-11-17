//! The model's constraint set behind one short, nameable type.
//!
//! The concrete set spells out every stream, closure and terminal in its
//! type, and the solver is generic over that type: every solver function
//! instantiated for it carries the whole name in its debug info, which grew
//! the debug build to gigabytes. Erasing the type once here keeps the
//! solver's instantiations small at the cost of one indirect call per
//! scoring step.

use solverforge::analysis::ConstraintAnalysis;
use solverforge::prelude::*;
use solverforge::ConstraintResult;

use crate::domain::Plan;

type Score = HardMediumSoftScore;

/// Every constraint of the model, type-erased.
pub struct Constraints(Box<dyn ConstraintSet<Plan, Score>>);

impl Constraints {
    /// Wraps a concrete constraint set.
    pub fn new(set: impl ConstraintSet<Plan, Score> + 'static) -> Self {
        Self(Box::new(set))
    }
}

impl ConstraintSet<Plan, Score> for Constraints {
    fn evaluate_all(&self, solution: &Plan) -> Score {
        self.0.evaluate_all(solution)
    }

    fn constraint_count(&self) -> usize {
        self.0.constraint_count()
    }

    fn constraint_metadata(&self) -> Vec<ConstraintMetadata<'_>> {
        self.0.constraint_metadata()
    }

    fn constraint_metadata_entries(&self) -> Vec<ConstraintMetadata<'_>> {
        self.0.constraint_metadata_entries()
    }

    fn evaluate_each<'a>(&'a self, solution: &Plan) -> Vec<ConstraintResult<'a, Score>> {
        self.0.evaluate_each(solution)
    }

    fn evaluate_detailed<'a>(&'a self, solution: &Plan) -> Vec<ConstraintAnalysis<'a, Score>> {
        self.0.evaluate_detailed(solution)
    }

    fn initialize_all(&mut self, solution: &Plan) -> Score {
        self.0.initialize_all(solution)
    }

    fn on_insert_all(&mut self, solution: &Plan, entity: usize, descriptor: usize) -> Score {
        self.0.on_insert_all(solution, entity, descriptor)
    }

    fn on_retract_all(&mut self, solution: &Plan, entity: usize, descriptor: usize) -> Score {
        self.0.on_retract_all(solution, entity, descriptor)
    }

    fn on_insert_all_overflowing(
        &mut self,
        solution: &Plan,
        entity: usize,
        descriptor: usize,
    ) -> (Score, bool) {
        self.0
            .on_insert_all_overflowing(solution, entity, descriptor)
    }

    fn on_retract_all_overflowing(
        &mut self,
        solution: &Plan,
        entity: usize,
        descriptor: usize,
    ) -> (Score, bool) {
        self.0
            .on_retract_all_overflowing(solution, entity, descriptor)
    }

    fn reset_all(&mut self) {
        self.0.reset_all()
    }

    fn constraint_weight_refs<'a>(&'a self, refs: &mut Vec<&'a ConstraintRef>) {
        self.0.constraint_weight_refs(refs)
    }

    fn evaluate_all_weighted(&self, solution: &Plan, multipliers: &[u32]) -> Score {
        self.0.evaluate_all_weighted(solution, multipliers)
    }

    fn evaluate_each_weighted<'a>(
        &'a self,
        solution: &Plan,
        multipliers: &[u32],
    ) -> Vec<ConstraintResult<'a, Score>> {
        self.0.evaluate_each_weighted(solution, multipliers)
    }

    fn evaluate_detailed_weighted<'a>(
        &'a self,
        solution: &Plan,
        multipliers: &[u32],
    ) -> Vec<ConstraintAnalysis<'a, Score>> {
        self.0.evaluate_detailed_weighted(solution, multipliers)
    }

    fn initialize_all_weighted(&mut self, solution: &Plan, multipliers: &[u32]) -> Score {
        self.0.initialize_all_weighted(solution, multipliers)
    }

    fn on_insert_all_weighted(
        &mut self,
        solution: &Plan,
        entity: usize,
        descriptor: usize,
        multipliers: &[u32],
    ) -> Score {
        self.0
            .on_insert_all_weighted(solution, entity, descriptor, multipliers)
    }

    fn on_retract_all_weighted(
        &mut self,
        solution: &Plan,
        entity: usize,
        descriptor: usize,
        multipliers: &[u32],
    ) -> Score {
        self.0
            .on_retract_all_weighted(solution, entity, descriptor, multipliers)
    }

    fn on_insert_all_weighted_overflowing(
        &mut self,
        solution: &Plan,
        entity: usize,
        descriptor: usize,
        multipliers: &[u32],
    ) -> (Score, bool) {
        self.0
            .on_insert_all_weighted_overflowing(solution, entity, descriptor, multipliers)
    }

    fn on_retract_all_weighted_overflowing(
        &mut self,
        solution: &Plan,
        entity: usize,
        descriptor: usize,
        multipliers: &[u32],
    ) -> (Score, bool) {
        self.0
            .on_retract_all_weighted_overflowing(solution, entity, descriptor, multipliers)
    }
}
