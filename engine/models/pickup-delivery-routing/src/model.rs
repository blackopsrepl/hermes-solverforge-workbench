//! The pick-up and delivery routing model as a workbench model.

use solverforge::{ConstraintSet, ConstraintWeights, HardMediumSoftScore, SolverManager};
use solverforge_workbench_host::fill::{FillAnswer, FillRequest};
use solverforge_workbench_host::view::View;
use solverforge_workbench_host::{
    Analysis, ConstraintInfo, DemoInfo, EngineError, EntityName, Issue, Kpi, Model, ParameterInfo,
    SizeField,
};

use crate::domain::Plan;

static MANAGER: SolverManager<Plan> = SolverManager::new();

/// Assign pick-ups and deliveries to drivers and route them.
pub struct PickupDeliveryRouting;

impl Model for PickupDeliveryRouting {
    type Plan = Plan;

    fn id() -> &'static str {
        "pickup-delivery-routing"
    }
    fn name() -> &'static str {
        "Pick-up and delivery routing"
    }
    fn summary() -> &'static str {
        "Routes drivers through pick-ups and drop-offs made in pairs: vehicle capacity, time windows, skills, shift limits and breaks."
    }
    fn demo_info() -> Vec<DemoInfo> {
        vec![DemoInfo {
            id: "MILAN_TWO_DAYS",
            name: "Milan, two days".into(),
            summary: "15 drivers on 30 shifts and 155 jobs around Milan over two days, each job a pick-up and a drop-off."
                .into(),
        }]
    }
    fn size_fields() -> Vec<SizeField> {
        vec![
            SizeField {
                path: "jobs",
                label: "jobs",
                one: "job",
            },
            SizeField {
                path: "drivers",
                label: "drivers",
                one: "driver",
            },
            SizeField {
                path: "drivers.shifts",
                label: "shifts",
                one: "shift",
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
    fn parameters() -> Vec<ParameterInfo> {
        crate::parameters::parameters()
    }
    fn view_kind() -> Option<&'static str> {
        Some("routes")
    }
    fn view(plan: &Plan, analysis: &Analysis) -> Option<View> {
        crate::view::view(plan, analysis)
    }
    fn fill(plan: Plan, request: &FillRequest) -> Result<FillAnswer<Plan>, EngineError> {
        crate::fill::fill(plan, request)
    }
    fn freeze(plan: &mut Plan, until: &str) -> Result<usize, EngineError> {
        crate::freeze::freeze(plan, until)
    }
}
