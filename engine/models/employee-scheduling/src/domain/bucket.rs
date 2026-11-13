use solverforge::prelude::*;

use crate::derived::Ctx;

/// A calendar day of the data, derived by `prepare`; groups the shifts that
/// can meet on it.
#[problem_fact]
pub struct Day {
    #[planning_id]
    pub index: usize,
    pub ctx: Ctx,
}

/// One period of a schedule-wide rule, derived by `prepare`.
#[problem_fact]
pub struct Window {
    #[planning_id]
    pub index: usize,
    pub ctx: Ctx,
}
