//! The employee scheduling model as a workbench model.

use solverforge::{ConstraintSet, ConstraintWeights, HardMediumSoftScore, SolverManager};
use solverforge_workbench_host::view::View;
use solverforge_workbench_host::{
    Analysis, ConstraintInfo, DemoInfo, EngineError, EntityName, Issue, Kpi, Model, SizeField,
};

use crate::domain::Plan;

static MANAGER: SolverManager<Plan> = SolverManager::new();

/// Assign employees to shifts under contracts, preferences and demand.
pub struct EmployeeScheduling;

impl Model for EmployeeScheduling {
    type Plan = Plan;

    fn id() -> &'static str {
        "employee-scheduling"
    }
    fn name() -> &'static str {
        "Employee scheduling"
    }
    fn summary() -> &'static str {
        "Assigns employees to shifts across a roster: skills, availability, rest between shifts, contracts, fairness and preferences."
    }
    fn demo_info() -> Vec<DemoInfo> {
        vec![
            DemoInfo {
                id: "LARGE_MONTH",
                name: "A month of shifts".into(),
                summary: "50 employees and 536 shifts from 28 February to 29 March 2027, under three contracts.".into(),
            },
        ]
    }
    fn size_fields() -> Vec<SizeField> {
        vec![
            SizeField {
                path: "employees",
                label: "employees",
            },
            SizeField {
                path: "shifts",
                label: "shifts",
            },
        ]
    }
    fn constraints() -> impl ConstraintSet<Plan, HardMediumSoftScore> {
        crate::constraints::define_constraints()
    }
    fn manager() -> &'static SolverManager<Plan> {
        &MANAGER
    }
    fn demos() -> Vec<&'static str> {
        crate::demo::ids()
    }
    fn demo(id: &str) -> Option<Plan> {
        crate::demo::build(id)
    }
    fn descriptor() -> solverforge::domain::SolutionDescriptor {
        Plan::descriptor()
    }
    fn prepare(plan: &mut Plan) -> Vec<Issue> {
        crate::prepare::prepare(plan)
    }
    fn present(plan: &Plan) -> serde_json::Value {
        crate::present::present(plan)
    }
    fn weights_mut(plan: &mut Plan) -> Option<&mut ConstraintWeights> {
        Some(&mut plan.weights)
    }
    fn catalog() -> Vec<ConstraintInfo> {
        crate::catalog::catalog()
    }
    fn kpis(plan: &Plan) -> Vec<Kpi> {
        crate::kpis::kpis(plan)
    }
    fn name_entity(plan: &Plan, type_name: &str, planning_id: &str) -> Option<EntityName> {
        crate::names::name_entity(plan, type_name, planning_id)
    }
    fn view_kind() -> Option<&'static str> {
        Some("timeline")
    }
    fn view(plan: &Plan, analysis: &Analysis) -> Option<View> {
        crate::view::view(plan, analysis)
    }
    fn freeze(plan: &mut Plan, until: &str) -> Result<usize, EngineError> {
        crate::freeze::freeze(plan, until)
    }
}
