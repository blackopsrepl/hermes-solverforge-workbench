//! The toy roster as a workbench model.

use serde_json::{json, Value};
use solverforge::{ConstraintSet, ConstraintWeights, HardSoftScore, SolverManager};
use solverforge_workbench_host::edit::{self, Args, Choice, Compiled, Edit, Field};
use solverforge_workbench_host::{ConstraintInfo, EngineError, Issue, Kpi, Level, Model};

use super::domain::{self, Nurse, Roster, Shift};

static MANAGER: SolverManager<Roster> = SolverManager::new();

pub struct Toy;

impl Model for Toy {
    type Plan = Roster;

    fn id() -> &'static str {
        "toy"
    }
    fn name() -> &'static str {
        "Toy roster"
    }
    fn constraints() -> impl ConstraintSet<Roster, HardSoftScore> {
        domain::define_constraints()
    }
    fn manager() -> &'static SolverManager<Roster> {
        &MANAGER
    }
    fn demos() -> Vec<&'static str> {
        vec!["WEEK"]
    }
    fn demo(id: &str) -> Option<Roster> {
        (id == "WEEK").then(week)
    }
    fn descriptor() -> solverforge::domain::SolutionDescriptor {
        Roster::descriptor()
    }
    fn prepare(plan: &mut Roster) -> Vec<Issue> {
        let mut issues = Vec::new();
        let nurses = plan.nurses.len();
        for shift in &plan.shifts {
            if let Some(idx) = shift.nurse_idx.filter(|&idx| idx >= nurses) {
                issues.push(Issue::error(
                    "unknown_nurse",
                    format!("shift {} names nurse {idx}, which does not exist", shift.id),
                ));
            }
        }
        if nurses < 2 {
            issues.push(Issue::warning(
                "few_nurses",
                "fewer than two nurses cannot cover a day",
            ));
        }
        issues
    }
    fn edits(plan: &Roster) -> Vec<Edit> {
        vec![Edit {
            id: "shift_cancelled",
            label: "A shift is cancelled".into(),
            description: "Removes the shift.".into(),
            fields: vec![Field::choice("shift", "Shift", shifts(plan))],
        }]
    }
    fn edit(plan: &Roster, id: &str, args: &Value) -> Result<Compiled, EngineError> {
        if id != "shift_cancelled" {
            return Err(edit::unknown(Self::id(), id, &Self::edits(plan)));
        }
        let options = shifts(plan);
        let shift = Args::new(id, args).choice("shift", &options)?;
        let label = options
            .into_iter()
            .find(|c| c.value == shift)
            .expect("offered")
            .label;
        Ok(Compiled {
            operations: vec![
                json!({"op": "remove", "path": format!("/shifts/{}", edit::by_id(shift))}),
            ],
            summary: format!("{label} is cancelled."),
        })
    }
    fn weights_mut(plan: &mut Roster) -> Option<&mut ConstraintWeights> {
        Some(&mut plan.weights)
    }
    fn catalog() -> Vec<ConstraintInfo> {
        vec![ConstraintInfo {
            name: "Unassigned shift",
            group: "Coverage",
            level: Level::Hard,
            description: "Every shift needs a nurse.",
        }]
    }
    fn kpis(plan: &Roster) -> Vec<Kpi> {
        vec![Kpi {
            id: "assigned",
            label: "Assigned shifts",
            value: plan.shifts.iter().filter(|s| s.nurse_idx.is_some()).count() as f64,
            unit: "shifts",
        }]
    }
}

fn shifts(plan: &Roster) -> Vec<Choice> {
    plan.shifts
        .iter()
        .map(|s| Choice::new(s.id.to_string(), format!("Shift {} on day {}", s.id, s.day)))
        .collect()
}

/// Three nurses, six shifts over three days, nothing assigned.
pub fn week() -> Roster {
    Roster {
        nurses: (0..3)
            .map(|id| Nurse {
                id,
                name: format!("nurse-{id}"),
            })
            .collect(),
        shifts: (0..6)
            .map(|id| Shift {
                id,
                day: (id / 2) as i64,
                pinned: false,
                nurse_idx: None,
            })
            .collect(),
        weights: ConstraintWeights::default(),
        score: None,
    }
}
