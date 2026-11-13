solverforge::planning_model! {
    root = "src/domain";

    mod bucket;
    mod employee;
    mod plan;
    mod shift;

    pub use bucket::{Choice, Day, Whole, Window};
    pub use employee::Employee;
    pub use plan::Plan;
    pub use shift::Shift;
}
