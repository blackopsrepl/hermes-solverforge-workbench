//! Constraints over all employees at once: per day bucket and per period of
//! a schedule-wide rule. Each family shares one grouped node.

use solverforge::prelude::*;

use crate::derived::{GlobalKind, Want};
use crate::domain::{Day, Plan, Window};
use crate::eval::{
    colleagues, concurrency_excess, teams, window_deviation, ByDay, ByWindow, Members,
    MembersCollector, Row, TeamScore,
};

type Score = HardMediumSoftScore;

pub fn constraints() -> impl ConstraintSet<Plan, Score> {
    (by_day(), by_window())
}

fn by_day() -> impl ConstraintSet<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::shifts())
        .project(ByDay)
        .group_by(|row: &Row| row.bucket as usize, MembersCollector)
        .complement(
            Plan::days(),
            |day: &Day| day.index,
            |day: &Day| Members::empty(day.ctx.clone()),
        )
        .penalize(hard_weight(|day: &usize, m: &Members| {
            Score::of_hard(concurrency_excess(m, *day))
        }))
        .named("Concurrent shifts over limit")
        .penalize(hard_weight(|day: &usize, m: &Members| {
            Score::of_hard(colleagues(m, *day, Want::Required))
        }))
        .named("Required partner absent")
        .reward(|day: &usize, m: &Members| Score::of_soft(colleagues(m, *day, Want::Preferred)))
        .named("Preferred partner present")
        .penalize(|day: &usize, m: &Members| Score::of_soft(colleagues(m, *day, Want::Unwanted)))
        .named("Unwanted partner present")
        .penalize(hard_weight(|day: &usize, m: &Members| {
            Score::of_hard(colleagues(m, *day, Want::Barred))
        }))
        .named("Barred partner present")
        .penalize(hard_weight(|day: &usize, m: &Members| {
            Score::of_hard(teams(m, *day, TeamScore::SplitTags))
        }))
        .named("Team members split across tags")
        .penalize(hard_weight(|day: &usize, m: &Members| {
            Score::of_hard(teams(m, *day, TeamScore::HelperWithoutLead))
        }))
        .named("Team member without a lead")
        .reward(|day: &usize, m: &Members| Score::of_soft(teams(m, *day, TeamScore::LeadWithTeam)))
        .named("Team lead works with the team")
        .penalize(hard_weight(|day: &usize, m: &Members| {
            Score::of_hard(teams(m, *day, TeamScore::ShiftsPerDay))
        }))
        .named("Team shifts per day over limit")
}

fn by_window() -> impl ConstraintSet<Plan, Score> {
    use GlobalKind as G;
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::shifts())
        .project(ByWindow)
        .group_by(|row: &Row| row.bucket as usize, MembersCollector)
        .complement(
            Plan::windows(),
            |window: &Window| window.index,
            |window: &Window| Members::empty(window.ctx.clone()),
        )
        .penalize(hard_weight(|w: &usize, m: &Members| {
            Score::of_hard(window_deviation(m, *w, G::Shifts, false))
        }))
        .named("Shift count per period outside required range")
        .penalize(|w: &usize, m: &Members| Score::of_soft(window_deviation(m, *w, G::Shifts, true)))
        .named("Shift count per period outside preferred range")
        .penalize(hard_weight(|w: &usize, m: &Members| {
            Score::of_hard(window_deviation(m, *w, G::DistinctTags, false))
        }))
        .named("Distinct tags per period across schedule outside required range")
        .penalize(|w: &usize, m: &Members| {
            Score::of_soft(window_deviation(m, *w, G::DistinctTags, true))
        })
        .named("Distinct tags per period across schedule outside preferred range")
}
