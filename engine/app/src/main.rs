//! `solverforge-workbench`: one JSON request on stdin, JSON Lines on stdout.
//! `solverforge-workbench --version` lists the engine version and its models.

use solverforge_workbench_host::run;

fn main() {
    std::process::exit(run(env!("CARGO_PKG_VERSION"), &[]));
}
