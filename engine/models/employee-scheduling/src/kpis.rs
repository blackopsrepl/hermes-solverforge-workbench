//! Key performance indicators of a plan.

use solverforge_workbench_host::Kpi;

use crate::domain::Plan;

fn kpi(id: &'static str, label: &'static str, value: f64, unit: &'static str) -> Kpi {
    Kpi {
        id,
        label,
        value,
        unit,
    }
}

/// Indicators of an assigned (and prepared) plan.
pub fn kpis(plan: &Plan) -> Vec<Kpi> {
    let in_horizon = |index: usize| {
        plan.shifts[index]
            .ctx
            .0
            .shifts
            .get(index)
            .is_none_or(|info| info.in_horizon)
    };
    let shifts: Vec<_> = plan
        .shifts
        .iter()
        .filter(|shift| in_horizon(shift.index))
        .collect();
    let assigned = shifts.iter().filter(|s| s.employee_idx.is_some()).count();
    vec![
        kpi(
            "assigned_shifts",
            "Assigned shifts",
            assigned as f64,
            "shifts",
        ),
        kpi(
            "unassigned_shifts",
            "Unassigned shifts",
            (shifts.len() - assigned) as f64,
            "shifts",
        ),
    ]
}
