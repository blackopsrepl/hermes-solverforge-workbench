solverforge::planning_model! {
    root = "src/domain";

    mod employee;
    mod plan;
    mod shift;

    pub use employee::Employee;
    pub use plan::Plan;
    pub use shift::Shift;
}
