use solverforge::prelude::*;

use crate::derived::Ctx;

/// One bay on one day: the value a task's start takes. Derived by `prepare`;
/// slot `n` is bay `n / days` on day `n % days`.
#[problem_fact]
pub struct BaySlot {
    #[planning_id]
    pub index: usize,
    pub ctx: Ctx,
}

/// A day of the horizon, derived by `prepare`; groups the readiness of the
/// fleet on that day.
#[problem_fact]
pub struct Day {
    #[planning_id]
    pub index: usize,
    pub ctx: Ctx,
}
