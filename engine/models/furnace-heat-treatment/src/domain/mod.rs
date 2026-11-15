solverforge::planning_model! {
    root = "src/domain";

    mod coverage;
    mod furnace;
    mod operator;
    mod order;
    mod plan;
    mod roster;

    pub use coverage::Coverage;
    pub use furnace::Furnace;
    pub use operator::Operator;
    pub use order::WorkOrder;
    pub use plan::Plan;
    pub use roster::RosterDay;
}
