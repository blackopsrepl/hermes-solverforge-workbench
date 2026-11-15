//! The model's settings, as the workbench lists them.

use serde_json::json;
use solverforge_workbench_host::ParameterInfo;

use crate::wire::Parameters;

/// Every setting a dataset or profile may give under `parameters`.
pub fn parameters() -> Vec<ParameterInfo> {
    let d = Parameters::default();
    let info = |name, kind, default: i64, description| ParameterInfo {
        name,
        kind,
        default: json!(default),
        description,
    };
    vec![
        info("overtimeCost", "integer", d.overtime_cost, "Cost of one technician-day worked above a crew's regular strength."),
        info("deferralCost", "integer", d.deferral_cost, "Cost of leaving a deferrable task unplanned, multiplied by its unit's priority."),
        info("targetShortfallCost", "integer", d.target_shortfall_cost, "Cost of each ready unit missing below a readiness target on a day."),
        info("changeCost", "integer", d.change_cost, "Cost of moving a published task to another bay or day, or unplanning it."),
        info("loadLevelCost", "integer", d.load_level_cost, "Cost of one technician-day above a crew's even share of the work."),
        info("inductionCost", "integer", d.induction_cost, "Cost of each extra workshop visit a unit makes beyond its first."),
        info("returnToServiceDays", "integer", d.return_to_service_days, "Days a unit stays out of service after a task that sets no return-to-service time of its own."),
    ]
}
