//! Facts derived from the dataset by `prepare`.

mod facts;

pub use facts::{
    meters, BreakFact, Coordination, CostFact, Delay, Dependency, Facts, Fit, GroupFact, Held,
    Kind, Pooling, ShiftFact, Travel, VehicleFact, VisitFact, Window,
};
