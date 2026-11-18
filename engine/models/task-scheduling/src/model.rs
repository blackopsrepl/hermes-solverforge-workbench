//! The task scheduling model as a workbench model.

use solverforge::{ConstraintSet, ConstraintWeights, HardMediumSoftScore, SolverManager};
use solverforge_workbench_host::edit::{Compiled, Edit};
use solverforge_workbench_host::fill::{FillAnswer, FillRequest};
use solverforge_workbench_host::view::View;
use solverforge_workbench_host::{
    Analysis, ConstraintInfo, DemoInfo, EngineError, EntityName, Issue, Kpi, Model, ParameterInfo,
    SizeField,
};

use crate::domain::Plan;

static MANAGER: SolverManager<Plan> = SolverManager::new();

/// Place tasks on machines and employees, in order and on time.
pub struct TaskScheduling;

impl Model for TaskScheduling {
    type Plan = Plan;

    fn id() -> &'static str {
        "task-scheduling"
    }
    fn name() -> &'static str {
        "Task scheduling"
    }
    fn summary() -> &'static str {
        "Orders jobs on machines and people: dependencies, skills, changeovers, availability, due dates and the length of the schedule."
    }
    fn demo_info() -> Vec<DemoInfo> {
        vec![
            DemoInfo {
                id: "FACTORY",
                name: "Factory week".into(),
                summary: "409 tasks on 24 machines and people over a week in March 2027, with changeovers and due dates.".into(),
            },
        ]
    }
    fn size_fields() -> Vec<SizeField> {
        vec![
            SizeField {
                path: "tasks",
                label: "tasks",
                one: "task",
            },
            SizeField {
                path: "resources",
                label: "machines and people",
                one: "machine or person",
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
    fn edits(plan: &Plan) -> Vec<Edit> {
        crate::edits::edits(plan)
    }
    fn edit(plan: &Plan, id: &str, args: &serde_json::Value) -> Result<Compiled, EngineError> {
        crate::edits::edit(plan, id, args)
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
