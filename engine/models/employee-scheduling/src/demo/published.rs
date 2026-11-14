//! A published week as an earlier plan would have made it.
//!
//! Each shift to publish goes to the first employee, in the demo's order of
//! preference, whose assignment adds no hard violation to the assignments
//! published so far. The model scores every try itself, so the demo copies
//! none of its rules: whatever a real plan could not have published, this
//! cannot publish either.

use serde_json::{json, Value};
use solverforge::ScoreDirector;

use crate::constraints::define_constraints;
use crate::domain::Plan;
use crate::prepare::prepare;

/// Records `published` on each shift in `shifts` (indices into the dataset's
/// shifts), trying the employees `order` lists for it. A shift no employee can
/// take without breaking a hard rule stays unpublished.
pub fn publish(
    dataset: &mut Value,
    shifts: &[usize],
    order: impl Fn(usize, &[usize]) -> Vec<usize>,
) {
    let mut plan: Plan =
        serde_json::from_value(dataset.clone()).expect("the demo is valid wire data");
    prepare(&mut plan);
    let ids: Vec<String> = plan.employees.iter().map(|e| e.id.clone()).collect();
    let mut director = ScoreDirector::new(plan, define_constraints());
    let mut hard = director.calculate_score().hard();
    let mut taken = vec![0usize; ids.len()];
    for &shift in shifts {
        for employee in order(shift, &taken) {
            let score = director.do_change(0, shift, |plan: &mut Plan| {
                plan.shifts[shift].employee_idx = Some(employee);
            });
            if score.hard() >= hard {
                hard = score.hard();
                taken[employee] += 1;
                dataset["shifts"][shift]["published"] = json!(ids[employee]);
                break;
            }
            director.do_change(0, shift, |plan: &mut Plan| {
                plan.shifts[shift].employee_idx = None
            });
        }
    }
}
