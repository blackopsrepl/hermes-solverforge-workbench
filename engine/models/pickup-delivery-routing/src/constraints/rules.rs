//! The thirty-eight rules: names, levels and groups, as weights, analysis and
//! the catalog know them.

use solverforge_workbench_host::Level;

pub const DEPENDENCIES: &str = "Dependencies between stops";
pub const CAPACITY: &str = "Vehicle capacity";
pub const REQUIREMENTS: &str = "Job requirements and tags";
pub const BREAKS: &str = "Breaks and appointments";
pub const MULTI_DAY: &str = "Multi-day schedules";
pub const PRIORITY: &str = "Priority and optional jobs";
pub const REPLANNING: &str = "Replanning";
pub const ROUTES: &str = "Route optimization";
pub const SLA: &str = "Service level agreements";
pub const HOURS: &str = "Shift hours and overtime";
pub const SKILLS: &str = "Skills";
pub const WINDOWS: &str = "Time windows";

pub const OUT_OF_ORDER: &str = "Stop served before the stop it follows";
pub const SPLIT: &str = "Job split across shifts";
pub const PREVIOUS_UNPLANNED: &str = "Stop planned while the stop before it is not";
pub const RIDE_PAST: &str = "Ride past a destination";
pub const PARTLY_PLANNED: &str = "Job partly planned";
pub const OVER_CAPACITY: &str = "Vehicle capacity exceeded";
pub const NOT_PREFERRED: &str = "Preferred driver not assigned";
pub const NOT_REQUIRED: &str = "Driver not among the required";
pub const PROHIBITED_DRIVER: &str = "Prohibited driver assigned";
pub const BURDEN: &str = "Time burden over the maximum";
pub const PROHIBITED_RIDE: &str = "Prohibited jobs on board together";
pub const POOLING: &str = "Pooled with a job that forbids pooling";
pub const TAGS: &str = "Required tags missing";
pub const PARTS: &str = "Parts out of stock";
pub const FLOATING_LATE: &str = "Floating break ends too late";
pub const FIXED_LATE: &str = "Late for a fixed break";
pub const APPOINTMENT_LATE: &str = "Late for an appointment";
pub const FIXED_DURING_JOB: &str = "Fixed break while a job is on board";
pub const FLOATING_DURING_JOB: &str = "Floating break while a job is on board";
pub const EARLIEST_DAY: &str = "Stop later than its earliest day";
pub const MANDATORY: &str = "Mandatory stop unplanned";
pub const OPTIONAL: &str = "Optional stop unplanned";
pub const PIN_SHIFT: &str = "Pinned stop off its shift";
pub const PIN_START: &str = "Pinned stop starts late";
pub const UNREACHABLE_ROUTE: &str = "Unreachable leg in a route";
pub const TRAVEL: &str = "Travel time";
pub const WAITING: &str = "Waiting time";
pub const UNREACHABLE_STOP: &str = "Stop unreachable from the previous place";
pub const SLA_MISSED: &str = "SLA missed";
pub const LAST_LATE: &str = "Last stop departs after the latest time";
pub const END_LATE: &str = "Shift ends after the latest time";
pub const LAST_SOFT_LATE: &str = "Last stop departs after the preferred time";
pub const END_SOFT_LATE: &str = "Shift ends after the preferred time";
pub const SKILLS_MISSING: &str = "Required skills missing";
pub const OVERQUALIFIED: &str = "Driver more skilled than needed";
pub const WINDOW_END: &str = "Service ends after its window";
pub const WINDOW_START: &str = "Service starts after its window";
pub const DRIVER_USED: &str = "Driver used";
pub const DRIVER_BALANCE: &str = "Work unbalanced between drivers";

/// Every rule: name, group, level and one sentence on what it costs.
pub const RULES: [(&str, &str, Level, &str); 39] = [
    (OUT_OF_ORDER, DEPENDENCIES, Level::Hard, "A stop starts before the job's previous stop has left; costs the seconds of overlap, or the previous stop's service seconds when it is unplanned, plus one hour."),
    (SPLIT, DEPENDENCIES, Level::Hard, "A job's stop is planned on another shift than the job's previous stop; costs the stop's service seconds plus one hour."),
    (PREVIOUS_UNPLANNED, DEPENDENCIES, Level::Hard, "A stop is planned while the job's previous stop is unplanned; costs the service seconds of both plus one hour."),
    (RIDE_PAST, DEPENDENCIES, Level::Hard, "The vehicle serves another stop at a job's next destination while the job is on board, without dropping it off; costs one hour per pass."),
    (PARTLY_PLANNED, DEPENDENCIES, Level::Hard, "Some stops of a job are planned and others are not; costs the service seconds of each unplanned one plus one hour."),
    (OVER_CAPACITY, CAPACITY, Level::Hard, "The load on board after a stop exceeds the vehicle's capacity of a type; costs one hour per unit over, per stop and type."),
    (NOT_PREFERRED, REQUIREMENTS, Level::Soft, "A stop's job prefers other drivers than the one serving it; costs one per stop."),
    (NOT_REQUIRED, REQUIREMENTS, Level::Hard, "A stop is served by a driver its job does not list among its required drivers; costs one hour per stop."),
    (PROHIBITED_DRIVER, REQUIREMENTS, Level::Hard, "A stop is served by a driver its job prohibits; costs one hour per stop."),
    (BURDEN, REQUIREMENTS, Level::Hard, "Pooling makes a job's ride longer than direct service plus its maximum time burden; costs the seconds over."),
    (PROHIBITED_RIDE, REQUIREMENTS, Level::Hard, "Two jobs where one prohibits the other are on board at the same time; costs the seconds together."),
    (POOLING, REQUIREMENTS, Level::Hard, "A job that forbids pooling is on board together with another job; costs the seconds together."),
    (TAGS, REQUIREMENTS, Level::Hard, "The shift serving a stop lacks tags its job requires; costs one hour per missing tag."),
    (PARTS, REQUIREMENTS, Level::Hard, "A stop needs parts its shift's stock no longer holds when it is served, its earlier stops having used them; costs one hour per unit short."),
    (FLOATING_LATE, BREAKS, Level::Hard, "A floating break ends after its latest end; costs the seconds late."),
    (FIXED_LATE, BREAKS, Level::Hard, "Work runs into a fixed break, which starts late; costs the seconds late."),
    (APPOINTMENT_LATE, BREAKS, Level::Hard, "The driver reaches a fixed break at a place of its own after it starts; costs the seconds late."),
    (FIXED_DURING_JOB, BREAKS, Level::Hard, "A fixed break falls while a job is on board, and the job or the break forbids it; costs the break seconds."),
    (FLOATING_DURING_JOB, BREAKS, Level::Hard, "A floating break falls while a job is on board, and the job or the break forbids it; costs the break seconds."),
    (EARLIEST_DAY, MULTI_DAY, Level::Soft, "A stop whose windows span several days is served after the first day a shift that may serve it works; costs 1000 per day later."),
    (MANDATORY, PRIORITY, Level::Medium, "A stop of a mandatory job is unplanned; costs its priority weight."),
    (OPTIONAL, PRIORITY, Level::Soft, "A stop of an optional job is unplanned; costs its priority weight."),
    (PIN_SHIFT, REPLANNING, Level::Hard, "A pinned stop is not on the shift it is pinned to; costs 10 000 000 000."),
    (PIN_START, REPLANNING, Level::Hard, "A pinned stop starts after the start it keeps; costs 100 000 per second late."),
    (UNREACHABLE_ROUTE, ROUTES, Level::Hard, "A shift's route has a leg no road connects, from its start through its stops and breaks to its end; costs one hour per leg."),
    (TRAVEL, ROUTES, Level::Soft, "Driving time of a shift, from its start through its stops to its end; costs the seconds."),
    (WAITING, ROUTES, Level::Soft, "A driver waits before starting service, for a window to open; costs the seconds. Off unless weighted."),
    (DRIVER_USED, ROUTES, Level::Soft, "A driver serves at least one stop in the plan; costs one hour per driver, so fewer drivers do the work. Off unless weighted."),
    (DRIVER_BALANCE, ROUTES, Level::Soft, "Drivers work different amounts: each driver's working minutes, start to end over all their shifts, squared; costs the sum, which is least when work is shared evenly. Off unless weighted."),
    (UNREACHABLE_STOP, ROUTES, Level::Hard, "No road leads to a stop or break from the place before it; costs one hour per visit."),
    (SLA_MISSED, SLA, Level::Soft, "A stop ends after its latest SLA end, or stays unplanned with that SLA inside the planning window; costs the seconds late."),
    (LAST_LATE, HOURS, Level::Hard, "A shift leaves its last stop after the latest departure; costs the seconds late."),
    (END_LATE, HOURS, Level::Hard, "A shift reaches its end location after its latest end; costs the seconds late."),
    (LAST_SOFT_LATE, HOURS, Level::Soft, "A shift leaves its last stop after the preferred departure; costs the seconds late."),
    (END_SOFT_LATE, HOURS, Level::Soft, "A shift reaches its end location after its preferred end; costs the seconds of overtime."),
    (SKILLS_MISSING, SKILLS, Level::Hard, "The shift serving a stop lacks a skill its job requires, or holds it below the required level; costs one hour per missing skill and per level short."),
    (OVERQUALIFIED, SKILLS, Level::Soft, "The shift serving a stop holds skills above the level its job requires; costs the service seconds times the levels above."),
    (WINDOW_END, WINDOWS, Level::Hard, "A stop's service ends after its window's latest end; costs the seconds late."),
    (WINDOW_START, WINDOWS, Level::Hard, "A stop's service starts after its window's latest start; costs the seconds late."),
];

/// Hard seconds one broken count-based rule costs (a missing tag or skill,
/// a unit over capacity, a driver the job rules out, an unreachable leg),
/// so it weighs like an hour of lateness rather than a second of it.
pub const HOUR: i64 = 3_600;

/// Hard cost of a pinned stop left off its shift.
pub const PIN_SHIFT_COST: i64 = 10_000_000_000;

/// Penalty per day a movable stop is served after its earliest day.
pub const DAY_COST: i64 = 1_000;
