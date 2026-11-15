//! The dataset's wire format.

mod parameters;
mod records;

pub use parameters::Parameters;
pub use records::{
    Assignment, Break, BreakKind, BreakPolicy, Capacity, Demand, Driver, DriverShift, Interruption,
    ItemKind, ItineraryItem, Job, JobDemand, PlanningWindow, PriorityWeight, Skill,
    SkillRequirement, Stop, TimeWindow, TravelMatrix,
};
