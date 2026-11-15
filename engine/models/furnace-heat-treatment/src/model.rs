//! The furnace heat treatment model as a workbench model.

use solverforge::{ConstraintSet, ConstraintWeights, HardMediumSoftScore, SolverManager};
use solverforge_workbench_host::fill::{FillAnswer, FillRequest};
use solverforge_workbench_host::view::View;
use solverforge_workbench_host::{
    Analysis, ConstraintInfo, EngineError, EntityName, Issue, Kpi, Model, ParameterInfo,
};

use crate::domain::Plan;

static MANAGER: SolverManager<Plan> = SolverManager::new();

/// Batch work orders into furnace loads, crew their manual tasks and roster the crews.
pub struct FurnaceHeatTreatment;

impl Model for FurnaceHeatTreatment {
    type Plan = Plan;

    fn id() -> &'static str {
        "furnace-heat-treatment"
    }
    fn name() -> &'static str {
        "Furnace heat treatment scheduling"
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
        crate::params::parameters()
    }
    fn view_kind() -> Option<&'static str> {
        Some("timeline")
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
