solverforge::planning_model! {
    root = "src/domain";

    // @solverforge:begin domain-exports
mod visit;
mod route;
mod plan;

pub use visit::Visit;
pub use route::Route;
pub use plan::Plan;
    // @solverforge:end domain-exports
}
