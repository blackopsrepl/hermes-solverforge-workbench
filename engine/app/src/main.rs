//! `solverforge-workbench`: one JSON request on stdin, JSON Lines on stdout.
//! `solverforge-workbench --version` lists the engine version and its models.

use solverforge_workbench_employee_scheduling::EmployeeScheduling;
use solverforge_workbench_host::{entry, run};
use solverforge_workbench_pickup_delivery_routing::PickupDeliveryRouting;
use solverforge_workbench_task_scheduling::TaskScheduling;

fn main() {
    std::process::exit(run(
        env!("CARGO_PKG_VERSION"),
        &[
            entry::<EmployeeScheduling>(),
            entry::<TaskScheduling>(),
            entry::<PickupDeliveryRouting>(),
        ],
    ));
}
