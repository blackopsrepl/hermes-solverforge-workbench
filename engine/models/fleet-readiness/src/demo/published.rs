//! Work already published, as an earlier plan would have placed it.
//!
//! Each task to publish goes to the first of its candidate slots (earliest
//! day first) whose placement adds no hard violation to what is published so
//! far: no hard rule may score worse than before, not even one that another
//! rule's gain would outweigh. The model scores every try itself, so the demo
//! copies none of its rules: whatever a real plan could not have published, this cannot publish
//! either. The dataset keeps each published placement as the task's
//! assignment, as a real replan starts from the plan people were given.

use solverforge::{ConstraintSet, HardMediumSoftScore, ScoreDirector};

use crate::constraints::define_constraints;
use crate::domain::Plan;
use crate::prepare::prepare;
use crate::time::format_date;
use crate::wire::Placement;

/// The hard score of every constraint, in the set's order.
fn hard_by_constraint<C: ConstraintSet<Plan, HardMediumSoftScore>>(
    director: &mut ScoreDirector<Plan, C>,
) -> Vec<i64> {
    director
        .constraint_results()
        .into_iter()
        .map(|result| result.score.hard())
        .collect()
}

/// Publishes and assigns each task in `tasks` (indexes into the plan's
/// tasks), in that order; pins the ones that start before day `pin_before`.
/// A task no slot can take without a hard violation stays unpublished.
pub fn publish(mut plan: Plan, tasks: &[usize], pin_before: i64) -> Plan {
    prepare(&mut plan);
    let mut director = ScoreDirector::new(plan.clone(), define_constraints());
    director.calculate_score();
    let mut hard = hard_by_constraint(&mut director);
    for &task in tasks {
        let candidates = plan.tasks[task].candidates.clone();
        for slot in candidates {
            director.do_change(0, task, |p: &mut Plan| {
                p.tasks[task].slot_idx = Some(slot);
            });
            let now = hard_by_constraint(&mut director);
            if now.iter().zip(&hard).all(|(after, before)| after >= before) {
                hard = now;
                let ctx = plan.tasks[task].ctx.clone();
                let placement = Placement {
                    bay: plan.bays[ctx.slot_bay(slot)].id.clone(),
                    start: format_date(ctx.origin + ctx.slot_day(slot)),
                };
                let record = &mut plan.tasks[task];
                record.bay = Some(placement.bay.clone());
                record.start = Some(placement.start.clone());
                record.pinned = ctx.slot_day(slot) < pin_before;
                record.published = Some(placement);
                break;
            }
            director.do_change(0, task, |p: &mut Plan| p.tasks[task].slot_idx = None);
        }
    }
    plan
}
