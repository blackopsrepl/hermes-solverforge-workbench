//! `workbench-testkit`: the workbench engine contract over the toy roster.

use solverforge_workbench_host::{entry, run};
use solverforge_workbench_testkit::toy::Toy;

fn main() {
    std::process::exit(run("0.0.0-testkit", &[entry::<Toy>()]));
}
