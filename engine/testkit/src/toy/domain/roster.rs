use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

use super::{Nurse, Shift};

#[planning_solution(constraints = "define_constraints", solver_toml = "../solver.toml")]
#[derive(Serialize, Deserialize)]
pub struct Roster {
    #[problem_fact_collection]
    pub nurses: Vec<Nurse>,
    #[planning_entity_collection]
    pub shifts: Vec<Shift>,
    #[constraint_weights]
    #[serde(default)]
    pub weights: ConstraintWeights,
    #[planning_score]
    pub score: Option<HardSoftScore>,
}

#[solverforge_constraints]
pub fn define_constraints() -> impl ConstraintSet<Roster, HardSoftScore> {
    let unassigned = ConstraintFactory::<Roster, HardSoftScore>::new()
        .for_each(Roster::shifts())
        .filter(|shift: &Shift| shift.nurse_idx.is_none())
        .penalize(HardSoftScore::ONE_HARD)
        .named("Unassigned shift");

    let same_day = ConstraintFactory::<Roster, HardSoftScore>::new()
        .for_each(Roster::shifts())
        .join((
            ConstraintFactory::<Roster, HardSoftScore>::new().for_each(Roster::shifts()),
            |left: &Shift, right: &Shift| {
                left.id < right.id
                    && left.day == right.day
                    && left.nurse_idx.is_some()
                    && left.nurse_idx == right.nurse_idx
            },
        ))
        .penalize(HardSoftScore::ONE_HARD)
        .named("Two shifts on one day");

    let first_nurse = ConstraintFactory::<Roster, HardSoftScore>::new()
        .for_each(Roster::shifts())
        .filter(|shift: &Shift| shift.nurse_idx == Some(0))
        .penalize(HardSoftScore::ONE_SOFT)
        .named("Shift for the first nurse");

    (unassigned, same_day, first_nurse)
}
