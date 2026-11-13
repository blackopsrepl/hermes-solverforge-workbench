//! What every constraint means, in the workbench's words.

use solverforge_workbench_host::{ConstraintInfo, Level};

const STAFFING: &str = "Staffing";
const FIT: &str = "Skills and risk";
const AVAILABILITY: &str = "Availability";

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
    ]
}
