//! What every constraint means, in the workbench's words.

use solverforge_workbench_host::{ConstraintInfo, Level};

const PLANNING: &str = "Planning";
const DUE: &str = "Due limits";
const BAYS: &str = "Bays";
const UNITS: &str = "Units";
const CREWS: &str = "Technicians";
const PARTS: &str = "Parts";
const READINESS: &str = "Readiness";
const COST: &str = "Cost and replanning";

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
        info("Mandatory task unplanned", PLANNING, Medium, "A task that cannot wait has no bay and day; costs its unit's priority."),
        info("Deferrable task unplanned", PLANNING, Soft, "A task that may wait has no bay and day; costs the deferral cost times its unit's priority."),
        info("Task starts before its earliest start", DUE, Hard, "A task starts before the first day it may; costs each day too early."),
        info("Task finishes after its due date", DUE, Hard, "A task finishes after its due date, or later than it could at the earliest; costs each day late."),
        info("Task starts past its usage limit", DUE, Hard, "A task starts after its unit's usage counter reaches the task's limit; costs each day past it."),
        info("Bay unsuitable for task", BAYS, Hard, "A task is in a bay of a class it cannot use, or one that cannot hold its unit's fleet type; costs its days."),
        info("Bay over capacity", BAYS, Hard, "A bay holds more units on a day than it has room for; costs each unit too many each day."),
        info("Task in a closed bay", BAYS, Hard, "A task occupies a bay on days the bay is closed; costs each day."),
        info("Maintenance while unit is away", UNITS, Hard, "A unit is out of service for a task while committed to operations or before it is available; costs each day."),
        info("Unit in two bays at once", UNITS, Hard, "A unit has tasks in two different bays on the same day; costs each day they overlap."),
        info("Task starts before its predecessor finishes", UNITS, Hard, "A task starts before a task it must follow has finished; costs each day of overlap, or all its days when the predecessor is unplanned."),
        info("Unit work split over visits", UNITS, Soft, "A unit's tasks are spread over more than one workshop visit; costs the induction cost per extra visit."),
        info("Crew over capacity", CREWS, Hard, "A crew needs more technicians on a day than its regular strength plus overtime; costs each technician short."),
        info("Overtime", CREWS, Soft, "A crew needs technicians above its regular strength on a day; costs the overtime cost per technician-day."),
        info("Uneven crew load", CREWS, Soft, "A crew needs more technicians on a day than its even share of the work; costs the load levelling cost per technician-day above it."),
        info("Parts short", PARTS, Hard, "A task starts before enough of a part it needs has arrived; costs each part missing."),
        info("Readiness below minimum", READINESS, Hard, "Fewer units are ready on a day than a readiness rule's minimum; costs each unit missing each day."),
        info("Readiness below target", READINESS, Soft, "Fewer units are ready on a day than a readiness rule's target; costs the target shortfall cost per unit missing each day."),
        info("Unit downtime", READINESS, Soft, "A unit is in the workshop, returning to service or grounded past a due limit; costs its downtime cost per day."),
        info("Bay cost", COST, Soft, "A task uses a bay; costs the bay's daily rate for each of its days."),
        info("Published task moved", COST, Soft, "A task is no longer in the bay and on the day it was published with; costs the change cost."),
    ]
}
