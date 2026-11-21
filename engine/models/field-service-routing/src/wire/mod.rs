//! The dataset's wire format.

mod parameters;
mod records;

pub use parameters::Parameters;
pub use records::{
    Alignment, Area, Assignment, Break, BreakKind, Coordination, Cost, DateAdjuster, DelayTo,
    ItemKind, ItineraryItem, Limits, LinkedShift, PlanningWindow, PoolingType, PreferredVehicle,
    PriorityWeight, Rate, RateUnit, ServicePart, ShiftLink, Skill, SkillRequirement,
    TemporarySkills, TemporaryTags, TimeWindow, Tool, Traffic, TravelMatrix, TravelPeriod,
    TravelTimeAdjustment, Vehicle, VehicleShift, Visit, VisitDependency, VisitGroup,
};
