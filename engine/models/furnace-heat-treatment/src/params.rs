//! The model's parameters, as `describe` lists them.

use serde_json::json;
use solverforge_workbench_host::ParameterInfo;

const fn info(
    name: &'static str,
    kind: &'static str,
    default: serde_json::Value,
    description: &'static str,
) -> ParameterInfo {
    ParameterInfo {
        name,
        kind,
        default,
        description,
    }
}

/// Every setting a dataset or profile may give under `parameters`.
pub fn parameters() -> Vec<ParameterInfo> {
    vec![
        info("slotMinutes", "duration", json!(15), "Minutes between possible load starts; divides a day."),
        info("shiftStart", "duration", json!(360), "Minute of the day the morning shift starts; the afternoon and night shifts follow, eight hours each."),
        info("loadBuildMinutes", "duration", json!(30), "Minutes to build a load's racks before charging."),
        info("chargeMinutes", "duration", json!(15), "Minutes between the end of load building and programming, while the load is charged."),
        info("programMinutes", "duration", json!(15), "Minutes to program the furnace cycle, ending at the load's start."),
        info("quenchMinutes", "duration", json!(15), "Minutes after discharge spent quenching, or cooling when the recipe has no quench."),
        info("unloadMinutes", "duration", json!(15), "Minutes to unload after the quench."),
        info("heavyLoadKg", "integer", json!(500), "Orders heavier than this need a crane-certified operator to unload."),
        info("purgeMinutes", "duration", json!(20), "Fixed minutes to purge and stabilise a furnace between two different loads, on top of heating or cooling."),
        info("minimumRest", "duration", json!(660), "Least rest between an operator's shifts on consecutive days."),
        info("maxShiftsPerWeek", "integer", json!(6), "Most shifts an operator may work in a week of the schedule."),
        info("targetShiftsPerWeek", "integer", json!(5), "Shifts per week above which an operator works overtime."),
        info("maxConsecutiveNights", "integer", json!(3), "Most night shifts an operator may work in a row."),
        info("peakStart", "duration", json!(480), "Minute of the day the peak electricity tariff starts."),
        info("peakEnd", "duration", json!(1200), "Minute of the day the peak electricity tariff ends; before peakStart for a tariff over midnight."),
        info("peakPrice", "number", json!(22.0), "Cents per kWh at the peak tariff."),
        info("offPeakPrice", "number", json!(12.0), "Cents per kWh off peak."),
        info("weekendOffPeak", "boolean", json!(true), "Saturdays and Sundays are billed off peak all day."),
    ]
}
