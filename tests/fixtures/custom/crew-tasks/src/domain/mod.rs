solverforge::planning_model! {
    root = "src/domain";

    // @solverforge:begin domain-exports
mod resource;
mod task;
mod plan;

pub use resource::Resource;
pub use task::Task;
pub use plan::Plan;
    // @solverforge:end domain-exports
}
