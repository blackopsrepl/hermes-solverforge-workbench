//! What every constraint means, in the workbench's words.

use solverforge_workbench_host::{ConstraintInfo, Level};

const STAFFING: &str = "Staffing";
const FIT: &str = "Skills and risk";
const AVAILABILITY: &str = "Availability";
const LIMITS: &str = "Work limits";
const TIME_OFF: &str = "Time off";

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
        info("Mandatory shift left open", STAFFING, Medium, "A mandatory shift inside the schedule has no employee; costs its importance."),
        info("Optional shift left open", STAFFING, Soft, "An optional shift inside the schedule has no employee; costs its importance."),
        info("Required skill lacking", FIT, Hard, "The employee does not hold, at the required level and at the shift start, a skill the shift requires."),
        info("Preferred skill lacking", FIT, Soft, "The employee does not hold a skill the shift would like its employee to have."),
        info("Prohibited risk exposure", FIT, Hard, "The shift exposes the employee to a risk factor they must avoid."),
        info("Works while unavailable", AVAILABILITY, Hard, "Minutes of a shift fall inside a time the employee cannot work."),
        info("Works outside available hours", AVAILABILITY, Hard, "Minutes of a shift fall outside the only times the employee said they can work."),
        info("Works in preferred hours", AVAILABILITY, Soft, "Rewards minutes of a shift inside a time the employee would like to work."),
        info("Works in unwanted hours", AVAILABILITY, Soft, "Minutes of a shift fall inside a time the employee would rather not work."),
        info("Minutes per period outside required range", LIMITS, Hard, "Minutes worked in a period is outside a required contract range; costs the distance outside it times rule weight and employee priority."),
        info("Minutes per period outside preferred range", LIMITS, Soft, "Minutes worked in a period is outside a preferred contract range; costs the distance outside it times rule weight and employee priority."),
        info("Minutes in rolling window outside required range", LIMITS, Hard, "Minutes worked in a rolling window is outside a required contract range; costs the distance outside it times rule weight and employee priority."),
        info("Minutes in rolling window outside preferred range", LIMITS, Soft, "Minutes worked in a rolling window is outside a preferred contract range; costs the distance outside it times rule weight and employee priority."),
        info("Logged minutes per period outside required range", LIMITS, Hard, "Logged minutes in a period, including minutes logged before the data, is outside a required contract range; costs the distance outside it times rule weight and employee priority."),
        info("Logged minutes per period outside preferred range", LIMITS, Soft, "Logged minutes in a period, including minutes logged before the data, is outside a preferred contract range; costs the distance outside it times rule weight and employee priority."),
        info("Days worked per period outside required range", LIMITS, Hard, "The number of days worked in a period is outside a required contract range; costs the distance outside it times rule weight and employee priority."),
        info("Days worked per period outside preferred range", LIMITS, Soft, "The number of days worked in a period is outside a preferred contract range; costs the distance outside it times rule weight and employee priority."),
        info("Days worked in rolling window outside required range", LIMITS, Hard, "The number of days worked in a rolling window is outside a required contract range; costs the distance outside it times rule weight and employee priority."),
        info("Days worked in rolling window outside preferred range", LIMITS, Soft, "The number of days worked in a rolling window is outside a preferred contract range; costs the distance outside it times rule weight and employee priority."),
        info("Employee shifts per period outside required range", LIMITS, Hard, "An employee's shift count in a period is outside a required contract range; costs the distance outside it times rule weight and employee priority."),
        info("Employee shifts per period outside preferred range", LIMITS, Soft, "An employee's shift count in a period is outside a preferred contract range; costs the distance outside it times rule weight and employee priority."),
        info("Employee shifts in rolling window outside required range", LIMITS, Hard, "An employee's shift count in a rolling window is outside a required contract range; costs the distance outside it times rule weight and employee priority."),
        info("Employee shifts in rolling window outside preferred range", LIMITS, Soft, "An employee's shift count in a rolling window is outside a preferred contract range; costs the distance outside it times rule weight and employee priority."),
        info("Weekends worked per period outside required range", LIMITS, Hard, "The number of weekends worked in a period is outside a required contract range; costs the distance outside it times rule weight and employee priority."),
        info("Weekends worked per period outside preferred range", LIMITS, Soft, "The number of weekends worked in a period is outside a preferred contract range; costs the distance outside it times rule weight and employee priority."),
        info("Weekends worked in rolling window outside required range", LIMITS, Hard, "The number of weekends worked in a rolling window is outside a required contract range; costs the distance outside it times rule weight and employee priority."),
        info("Weekends worked in rolling window outside preferred range", LIMITS, Soft, "The number of weekends worked in a rolling window is outside a preferred contract range; costs the distance outside it times rule weight and employee priority."),
        info("Weekend minutes per period outside required range", LIMITS, Hard, "Minutes worked on weekend days in a period is outside a required contract range; costs the distance outside it times rule weight and employee priority."),
        info("Weekend minutes per period outside preferred range", LIMITS, Soft, "Minutes worked on weekend days in a period is outside a preferred contract range; costs the distance outside it times rule weight and employee priority."),
        info("Consecutive weekends worked outside required range", LIMITS, Hard, "A run of worked weekends inside a period is outside a required contract range; costs the distance outside it times rule weight and employee priority."),
        info("Consecutive weekends worked outside preferred range", LIMITS, Soft, "A run of worked weekends inside a period is outside a preferred contract range; costs the distance outside it times rule weight and employee priority."),
        info("Consecutive days worked outside required range", LIMITS, Hard, "A run of consecutive worked days is outside a required contract range; costs the distance outside it times rule weight and employee priority."),
        info("Consecutive days worked outside preferred range", LIMITS, Soft, "A run of consecutive worked days is outside a preferred contract range; costs the distance outside it times rule weight and employee priority."),
        info("Consecutive shifts outside required range", LIMITS, Hard, "A run of shifts close together is outside a required contract range; costs the distance outside it times rule weight and employee priority."),
        info("Consecutive shifts outside preferred range", LIMITS, Soft, "A run of shifts close together is outside a preferred contract range; costs the distance outside it times rule weight and employee priority."),
        info("Rest after a run of shifts below required minimum", LIMITS, Hard, "After a long enough run of shifts the employee gets fewer minutes off than required; costs the missing minutes."),
        info("Rest after a run of shifts below preferred minimum", LIMITS, Soft, "After a long enough run of shifts the employee gets fewer minutes off than preferred; costs the missing minutes."),
        info("Gap between differing runs below required minimum", LIMITS, Hard, "A run of one shift category follows a run of another too soon; costs the missing minutes."),
        info("Gap between differing runs below preferred minimum", LIMITS, Soft, "A run of one shift category follows a run of another sooner than preferred; costs the missing minutes."),
        info("Run starts on a disallowed day", LIMITS, Hard, "A run of worked days starts on a weekday a required rule does not allow."),
        info("Run starts on an allowed day", LIMITS, Soft, "Rewards each run of worked days that starts on a weekday a preferred rule allows."),
        info("Run ends on a start day", LIMITS, Soft, "A run of worked days ends on a weekday reserved for starting runs."),
        info("Compact run of one category", LIMITS, Soft, "Rewards each day that continues the previous day's shift category within a run."),
        info("Days off per period outside required range", TIME_OFF, Hard, "The number of days off in a period is outside a required contract range; costs the distance outside it times rule weight and employee priority."),
        info("Days off per period outside preferred range", TIME_OFF, Soft, "The number of days off in a period is outside a preferred contract range; costs the distance outside it times rule weight and employee priority."),
        info("Consecutive days off per period below required minimum", TIME_OFF, Hard, "The longest run of days off in a period is shorter than required; costs the missing days."),
        info("Consecutive days off per period below preferred minimum", TIME_OFF, Soft, "The longest run of days off in a period is shorter than preferred; costs the missing days."),
        info("Consecutive days off in rolling window below required minimum", TIME_OFF, Hard, "The longest run of days off in a rolling window is shorter than required; costs the missing days."),
        info("Consecutive days off in rolling window below preferred minimum", TIME_OFF, Soft, "The longest run of days off in a rolling window is shorter than preferred; costs the missing days."),
        info("Continuous rest in rolling window below required minimum", TIME_OFF, Hard, "The longest stretch without work in a rolling window is shorter than required; costs the missing minutes."),
        info("Continuous rest in rolling window below preferred minimum", TIME_OFF, Soft, "The longest stretch without work in a rolling window is shorter than preferred; costs the missing minutes."),
        info("Consecutive weekends off per period below required minimum", TIME_OFF, Hard, "The longest run of weekends off in a period is shorter than required; costs the missing weekends."),
        info("Consecutive weekends off per period below preferred minimum", TIME_OFF, Soft, "The longest run of weekends off in a period is shorter than preferred; costs the missing weekends."),
        info("Barred shift next to a requested day off", TIME_OFF, Hard, "A shift a contract bars lands on the day before or after a requested day off."),
        info("Unwanted shift next to a requested day off", TIME_OFF, Soft, "A shift a contract dislikes lands on the day before or after a requested day off."),
    ]
}
