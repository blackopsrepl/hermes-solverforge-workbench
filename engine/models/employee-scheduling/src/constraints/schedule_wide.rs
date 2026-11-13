//! Constraints over all employees at once: per day bucket and per period of
//! a schedule-wide rule. Each family shares one grouped node.

use solverforge::prelude::*;

use crate::derived::{GlobalKind, Want};
use crate::domain::{Choice, Day, Plan, Whole, Window};
use crate::eval::{
    alternatives_off, colleagues, concurrency_excess, demand, share_gap, teams, window_deviation,
    ByChoice, ByDay, ByShare, ByWindow, Coverage, Members, MembersCollector, Row, TeamScore,
};

type Score = HardMediumSoftScore;

pub fn constraints() -> impl ConstraintSet<Plan, Score> {
    (by_day(), by_window(), by_choice(), by_share())
}

fn by_choice() -> impl ConstraintSet<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::shifts())
        .project(ByChoice)
        .group_by(|row: &Row| row.bucket as usize, MembersCollector)
        .complement(
            Plan::choices(),
            |choice: &Choice| choice.index,
            |choice: &Choice| Members::empty(choice.ctx.clone()),
        )
        .penalize(hard_weight(|group: &usize, m: &Members| {
            Score::of_hard(alternatives_off(m, *group))
        }))
        .named("Alternative shifts worked off count")
}

fn by_share() -> impl ConstraintSet<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::shifts())
        .project(ByShare)
        .group_by(|row: &Row| row.bucket as usize, MembersCollector)
        .complement(
            Plan::wholes(),
            |whole: &Whole| whole.index,
            |whole: &Whole| Members::empty(whole.ctx.clone()),
        )
        .penalize(|_: &usize, m: &Members| Score::of_soft(share_gap(m)))
        .named("Activated employees off target share")
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
        .penalize(|day: &usize, m: &Members| {
            Score::of(0, demand(m, *day, Coverage::Under, false), 0)
        })
        .named("Required demand under-staffed")
        .penalize(|day: &usize, m: &Members| {
            Score::of(0, demand(m, *day, Coverage::Over, false), 0)
        })
        .named("Required demand over-staffed")
        .penalize(|day: &usize, m: &Members| Score::of_soft(demand(m, *day, Coverage::Under, true)))
        .named("Preferred demand under-staffed")
        .penalize(|day: &usize, m: &Members| Score::of_soft(demand(m, *day, Coverage::Over, true)))
        .named("Preferred demand over-staffed")
        .penalize(|day: &usize, m: &Members| {
            Score::of_soft(demand(m, *day, Coverage::Surplus, false))
        })
        .named("Surplus staff unevenly spread over demand")
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
        .penalize(hard_weight(|w: &usize, m: &Members| {
            Score::of_hard(window_deviation(m, *w, G::Cost, false))
        }))
        .named("Cost per period outside required range")
        .penalize(|w: &usize, m: &Members| Score::of_soft(window_deviation(m, *w, G::Cost, true)))
        .named("Cost per period outside preferred range")
        .penalize(|w: &usize, m: &Members| {
            Score::of_soft(window_deviation(m, *w, G::CostTotal, true))
        })
        .named("Cost per period")
}
