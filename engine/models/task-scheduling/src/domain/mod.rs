solverforge::planning_model! {
    root = "src/domain";

    mod plan;
    mod resource;
    mod task;

    pub use plan::Plan;
    pub use resource::{Lane, Resource};
    pub use task::Task;
}
