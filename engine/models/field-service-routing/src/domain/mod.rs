solverforge::planning_model! {
    root = "src/domain";

    mod plan;
    mod shift;
    mod visit;

    pub use plan::Plan;
    pub use shift::Shift;
    pub use visit::{Status, Visit};
}
