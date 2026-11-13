solverforge::planning_model! {
    root = "src/toy/domain";

    mod nurse;
    mod roster;
    mod shift;

    pub use nurse::Nurse;
    pub use roster::{define_constraints, Roster};
    pub use shift::Shift;
}
