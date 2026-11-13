//! What every constraint means, in the workbench's words.

use solverforge_workbench_host::{ConstraintInfo, Level};

const STAFFING: &str = "Staffing";
const FIT: &str = "Skills and risk";
const AVAILABILITY: &str = "Availability";
const LIMITS: &str = "Work limits";
const TIME_OFF: &str = "Time off";
const PATTERNS: &str = "Rotations and patterns";
const ASSIGNMENT: &str = "Shift assignment";
const VARIETY: &str = "Shift variety";

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
        info("Overlapping shifts", PATTERNS, Hard, "Minutes in which two shifts of one employee overlap, unless a contract allows both to overlap."),
        info("Rest between shifts outside required range", PATTERNS, Hard, "The minutes from one shift's end to the employee's next shift are outside a required range; costs the distance."),
        info("Rest between shifts outside preferred range", PATTERNS, Soft, "The minutes from one shift's end to the employee's next shift are outside a preferred range; costs the distance."),
        info("Start time spread per period outside required range", PATTERNS, Hard, "The earliest and latest start times of day in a period lie further apart than required; costs the excess minutes."),
        info("Start time spread per period outside preferred range", PATTERNS, Soft, "The earliest and latest start times of day in a period lie further apart than preferred; costs the excess minutes."),
        info("Split shifts close together", PATTERNS, Soft, "Rewards successive shifts of a split day for every minute their gap stays under the largest allowed gap."),
        info("Required shift pairing missing", PATTERNS, Hard, "A shift that must come with a paired shift on a set day lacks it."),
        info("Preferred shift pairing missing", PATTERNS, Soft, "A shift that should come with a paired shift on a set day lacks it."),
        info("Barred shift pairing worked", PATTERNS, Hard, "A shift is worked together with a paired shift it must never go with."),
        info("Unwanted shift pairing worked", PATTERNS, Soft, "A shift is worked together with a paired shift it should not go with."),
        info("Required day pattern broken", PATTERNS, Hard, "A day has shifts of a required single-day pattern without completing the pattern."),
        info("Preferred day pattern worked", PATTERNS, Soft, "Rewards each day whose shifts complete a preferred single-day pattern."),
        info("Barred day pattern worked", PATTERNS, Hard, "A day's shifts complete a barred single-day pattern."),
        info("Unwanted day pattern worked", PATTERNS, Soft, "A day's shifts complete an unwanted single-day pattern."),
        info("Required multi-day pattern broken", PATTERNS, Hard, "A run of days starts like a required multi-day pattern but does not follow it."),
        info("Preferred multi-day pattern worked", PATTERNS, Soft, "Rewards each run of days that follows a preferred multi-day pattern."),
        info("Barred multi-day pattern worked", PATTERNS, Hard, "A run of days follows a barred multi-day pattern."),
        info("Unwanted multi-day pattern worked", PATTERNS, Soft, "A run of days follows an unwanted multi-day pattern."),
        info("Required weekly rotation broken", PATTERNS, Hard, "A week mixes rotation groups or does not move on to the next group, against a required rotation."),
        info("Preferred weekly rotation broken", PATTERNS, Soft, "A week mixes rotation groups or does not move on to the next group, against a preferred rotation."),
        info("Rotation block longer than required maximum", PATTERNS, Hard, "A block of days in one rotation group runs longer than required; costs the extra days."),
        info("Rotation block length outside preferred range", PATTERNS, Soft, "A block of days in one rotation group is shorter or longer than preferred; costs the distance."),
        info("Preferred employee on shift", ASSIGNMENT, Soft, "Rewards a shift worked by one of the employees it prefers."),
        info("Unwanted employee on shift", ASSIGNMENT, Soft, "A shift is worked by an employee it would rather not have."),
        info("Barred employee on shift", ASSIGNMENT, Hard, "A shift is worked by an employee it bars."),
        info("Concurrent shifts over limit", ASSIGNMENT, Hard, "More matching shifts run at once than a concurrency rule allows; costs the excess at each shift start."),
        info("Shift count per period outside required range", ASSIGNMENT, Hard, "Assigned matching shifts across all employees in a period fall outside a required range; costs the distance."),
        info("Shift count per period outside preferred range", ASSIGNMENT, Soft, "Assigned matching shifts across all employees in a period fall outside a preferred range; costs the distance."),
        info("Shift outside employee's required tags", VARIETY, Hard, "An employee who only works certain shifts is given another kind."),
        info("Shift has employee's preferred tags", VARIETY, Soft, "Rewards a shift of a kind the employee likes."),
        info("Shift types per period outside required range", VARIETY, Hard, "The shifts of a category an employee works in a period fall outside a required range; costs the distance per category."),
        info("Shift types per period outside preferred range", VARIETY, Soft, "The shifts of a category an employee works in a period fall outside a preferred range; costs the distance per category."),
        info("Distinct tags per period for employee outside required range", VARIETY, Hard, "An employee works more (or fewer) distinct tags of some types in a period than required."),
        info("Distinct tags per period for employee outside preferred range", VARIETY, Soft, "An employee works more (or fewer) distinct tags of some types in a period than preferred."),
        info("Distinct tags per period across schedule outside required range", VARIETY, Hard, "All assigned shifts of a period staff more (or fewer) distinct tags of some types than required."),
        info("Distinct tags per period across schedule outside preferred range", VARIETY, Soft, "All assigned shifts of a period staff more (or fewer) distinct tags of some types than preferred."),
    ]
}
