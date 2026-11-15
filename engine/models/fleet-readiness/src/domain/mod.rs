solverforge::planning_model! {
    root = "src/domain";

    mod bucket;
    mod plan;
    mod task;

    pub use bucket::{BaySlot, Day};
    pub use plan::Plan;
    pub use task::Task;
}
