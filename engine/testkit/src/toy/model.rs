//! The toy roster as a workbench model.

use solverforge::{ConstraintSet, ConstraintWeights, HardSoftScore, SolverManager};
use solverforge_workbench_host::{ConstraintInfo, Issue, Kpi, Level, Model};

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
