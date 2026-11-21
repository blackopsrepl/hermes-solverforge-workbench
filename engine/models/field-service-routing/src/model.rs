//! The field service routing model as a workbench model.

use solverforge::{ConstraintSet, ConstraintWeights, HardMediumSoftScore, SolverManager};
use solverforge_workbench_host::edit::{Compiled, Edit};
use solverforge_workbench_host::fill::{FillAnswer, FillRequest};
use solverforge_workbench_host::roads::{Place, RoadMatrix, Travel};
use solverforge_workbench_host::traffic::Traffic;
use solverforge_workbench_host::view::View;
use solverforge_workbench_host::{
    Analysis, ConstraintInfo, DemoInfo, EngineError, EntityName, Issue, Kpi, Model, ParameterInfo,
    SizeField,
};

use crate::domain::Plan;

static MANAGER: SolverManager<Plan> = SolverManager::new();

/// Assign visits to technicians and route them.
pub struct FieldServiceRouting;

impl Model for FieldServiceRouting {
    type Plan = Plan;

    fn id() -> &'static str {
        "field-service-routing"
    }
    fn name() -> &'static str {
        "Field service routing"
    }
    fn summary() -> &'static str {
        "Routes technicians and their vans through customer visits over several days: skills, time windows, linked and multi-technician visits, breaks and overtime."
    }
    fn demo_info() -> Vec<DemoInfo> {
        vec![
            DemoInfo {
                id: "TURIN_TWO_DAYS",
                name: "Turin, two days".into(),
                summary: "30 technicians on 60 shifts and 223 visits around Turin and Aosta, with linked visits, lunch breaks and a two-day lift job in the Aosta valley.".into(),
            },
        ]
    }
    fn size_fields() -> Vec<SizeField> {
        vec![
            SizeField {
                path: "visits",
                label: "visits",
                one: "visit",
            },
            SizeField {
                path: "vehicles",
                label: "technicians",
                one: "technician",
            },
            SizeField {
                path: "vehicles.shifts",
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
    fn tables() -> Vec<solverforge_workbench_host::data::Table> {
        crate::tables::tables()
    }
    fn edits(plan: &Plan) -> Vec<Edit> {
        crate::edits::edits(plan)
    }
    fn edit(plan: &Plan, id: &str, args: &serde_json::Value) -> Result<Compiled, EngineError> {
        crate::edits::edit(plan, id, args)
    }
    fn travels() -> bool {
        true
    }
    fn places(plan: &Plan) -> Vec<Place> {
        crate::roads::places(plan)
    }
    fn travel(plan: &Plan) -> Travel {
        crate::roads::travel(plan)
    }
    fn traffic(plan: &Plan) -> Vec<Traffic> {
        plan.traffic.clone()
    }
    fn set_roads(plan: &mut Plan, roads: RoadMatrix) {
        crate::roads::set_roads(plan, roads)
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
