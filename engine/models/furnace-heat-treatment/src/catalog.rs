//! What every constraint means, in the workbench's words.

use solverforge_workbench_host::{ConstraintInfo, Level};

const SCHEDULING: &str = "Scheduling";
const FURNACES: &str = "Furnaces and recipes";
const CHANGEOVERS: &str = "Changeovers";
const ENERGY: &str = "Energy";
const TASKS: &str = "Manual tasks";
const MONITORING: &str = "Monitoring";
const ROSTER: &str = "Roster";
const FAIRNESS: &str = "Fairness";
const DUE: &str = "Due dates";
const STARTS: &str = "Start times";

const fn info(
    name: &'static str,
    group: &'static str,
    level: Level,
    description: &'static str,
) -> ConstraintInfo {
    ConstraintInfo {
        name,
        group,
        level,
        description,
    }
}

/// Every constraint with its group, level and a one-sentence description.
pub fn catalog() -> Vec<ConstraintInfo> {
    use Level::{Hard, Medium, Soft};
    vec![
        info("Work order not scheduled", SCHEDULING, Medium, "A work order has no furnace and start; costs 1 for standard, 4 for urgent and 9 for express orders."),
        info("Furnace cannot run the process", FURNACES, Hard, "A load runs in a furnace that is not equipped for its recipe's process."),
        info("Setpoint above furnace maximum", FURNACES, Hard, "A recipe's setpoint is hotter than its furnace can go; costs 20 per °C over."),
        info("Furnace load over capacity", FURNACES, Hard, "The orders sharing one load weigh more than the furnace takes; costs 10 per kg over."),
        info("Load runs past the schedule", FURNACES, Hard, "A load is still in the furnace when the schedule ends; costs 5 per minute over."),
        info("Loads overlap in a furnace", FURNACES, Hard, "Two different loads are in one furnace at the same time, including two recipes charged together; costs 20 per minute of overlap."),
        info("Load during furnace downtime", FURNACES, Hard, "A load runs while its furnace is down for maintenance; costs 20 per minute of overlap."),
        info("Load starts before parts arrive", FURNACES, Hard, "A load is charged before its parts are released to the furnace; costs 10 per minute early."),
        info("Changeover time too short", CHANGEOVERS, Hard, "A furnace has less time than it needs to heat up or cool down, purge and change atmosphere between two loads; costs 10 per minute missing."),
        info("Changeover time", CHANGEOVERS, Soft, "Minutes each furnace spends changing over between consecutive loads, so loads of similar recipes run back to back."),
        info("Energy cost", ENERGY, Soft, "Cents of electricity each load uses at the peak or off-peak tariff of the hours it runs; orders sharing a load share its cost."),
        info("Load building crosses a shift change", TASKS, Hard, "Building a load's racks is split across two shifts, or falls outside the roster."),
        info("Programming crosses a shift change", TASKS, Hard, "Programming a load's furnace cycle is split across two shifts, or falls outside the roster."),
        info("Quench crosses a shift change", TASKS, Hard, "Quenching a load is split across two shifts, or falls outside the roster."),
        info("Unloading crosses a shift change", TASKS, Hard, "Unloading a load is split across two shifts, or falls outside the roster."),
        info("Crew on an unscheduled order", TASKS, Hard, "An order that has no furnace still has operators on its tasks."),
        info("Task without an operator", TASKS, Hard, "A manual task of a scheduled order has no operator; costs 4500 per task."),
        info("Operator lacks the skill for a task", TASKS, Hard, "An operator does a task without its skill: load building, programming, quench operation, or crane work for heavy unloads."),
        info("Operator not rostered for a task's shift", TASKS, Hard, "An operator does a task during a shift they are not rostered on."),
        info("Operator double-booked", TASKS, Hard, "An operator has two manual tasks at the same time, other than one task of a shared load; costs 900 per minute of overlap."),
        info("Too few operators monitoring", MONITORING, Hard, "In a quarter hour the rostered monitors cannot cover one per two ramping furnaces, one per four soaking ones and everyone busy with a task."),
        info("Shift short of a role", ROSTER, Hard, "A shift has fewer operators of a role than its coverage asks for; costs 900 per operator missing."),
        info("Day-only operator on nights", ROSTER, Hard, "An operator who only works days is rostered on a night shift."),
        info("Rest between shifts too short", ROSTER, Hard, "An operator has less than the minimum rest between shifts on consecutive days; costs 1 per minute missing."),
        info("Too many shifts in a week", ROSTER, Hard, "An operator works more shifts in a week than the most allowed."),
        info("Too many nights in a row", ROSTER, Hard, "An operator works more consecutive night shifts than allowed."),
        info("Operator rostered while unavailable", ROSTER, Hard, "An operator is rostered on a day of leave or training."),
        info("Shifts above the weekly target", ROSTER, Soft, "An operator works more shifts in a week than the target; costs 240 per extra shift."),
        info("Uneven shifts across operators", FAIRNESS, Soft, "Shifts are spread unevenly over the operators."),
        info("Express order late", DUE, Soft, "An express order comes out of the furnace after its due time; costs 60 per minute late."),
        info("Urgent order late", DUE, Soft, "An urgent order comes out of the furnace after its due time; costs 20 per minute late."),
        info("Standard order late", DUE, Soft, "A standard order comes out of the furnace after its due time; costs 5 per minute late."),
        info("Finished long before due", DUE, Soft, "An order waits finished long before it is due; costs 1 per 40 minutes early."),
        info("Day process charged at night", STARTS, Soft, "A load of a short day process is charged on a night shift, when the crew is thin; long carburizing and nitriding cycles are exempt."),
    ]
}
