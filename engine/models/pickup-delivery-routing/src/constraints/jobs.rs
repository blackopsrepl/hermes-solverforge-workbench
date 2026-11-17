//! Rules over a job's stops together: order, one shift, all or nothing.
//!
//! A stop still pending (not in any list while construction or a ruin move
//! places it) takes no part: only planned and benched stops count, so
//! placing a job one stop at a time never passes through a broken job.

use solverforge::prelude::*;
use solverforge::stream::joiner::equal_bi;

use super::rules::*;
use crate::domain::{Plan, Status, Visit};

pub type Score = HardMediumSoftScore;

/// Hard seconds added to every broken link of a job (a stop split from,
/// planned without, or served before the stop it follows), so a broken job
/// always costs more than the lateness it might save.
pub const BROKEN_LINK: i64 = 3_600;

/// A stop's cost against the job's stop right before it.
type PairAmount = fn(&Visit, &Visit) -> i64;

fn pair(name: &'static str, amount: PairAmount) -> impl IncrementalConstraint<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::visits())
        .filter(|next: &Visit| next.previous.is_some() && next.status() != Status::Pending)
        .join((
            ConstraintFactory::<Plan, Score>::new()
                .for_each(Plan::visits())
                .filter(|previous: &Visit| previous.status() != Status::Pending),
            equal_bi(
                |next: &Visit| next.previous,
                |previous: &Visit| Some(previous.index),
            ),
        ))
        .filter(move |next: &Visit, previous: &Visit| amount(next, previous) != 0)
        .penalize(hard_weight(move |next: &Visit, previous: &Visit| {
            Score::of_hard(amount(next, previous))
        }))
        .named(name)
}

/// Starts before the previous stop has left, on the same shift; or the
/// previous stop is unplanned.
fn out_of_order(next: &Visit, previous: &Visit) -> i64 {
    match (next.status(), previous.status()) {
        (Status::Planned, Status::Planned) if next.shift == previous.shift => {
            let (Some(next_at), Some(previous_at)) = (next.at(), previous.at()) else {
                return 0;
            };
            match previous_at.end - next_at.start {
                overlap if overlap > 0 => overlap + BROKEN_LINK,
                _ => 0,
            }
        }
        (Status::Planned, Status::Benched) => previous.duration + BROKEN_LINK,
        _ => 0,
    }
}

fn split(next: &Visit, previous: &Visit) -> i64 {
    match (next.status(), previous.status()) {
        (Status::Planned, Status::Planned) if next.shift != previous.shift => {
            next.effective_duration() + BROKEN_LINK
        }
        _ => 0,
    }
}

fn previous_unplanned(next: &Visit, previous: &Visit) -> i64 {
    match (next.status(), previous.status()) {
        (Status::Planned, Status::Benched) => {
            next.effective_duration() + previous.duration + BROKEN_LINK
        }
        _ => 0,
    }
}

/// One planned or benched stop of a job: whether it is planned, and its
/// service seconds as given.
type Member = (bool, i64);

fn partly_planned(members: &CollectedVec<Member>) -> i64 {
    let planned = members.iter().any(|m| m.0);
    let unplanned: i64 = members
        .iter()
        .filter(|m| !m.0)
        .map(|m| m.1 + BROKEN_LINK)
        .sum();
    if planned {
        unplanned
    } else {
        0
    }
}

pub fn constraints() -> impl ConstraintSet<Plan, Score> {
    (
        pair(OUT_OF_ORDER, out_of_order),
        pair(SPLIT, split),
        pair(PREVIOUS_UNPLANNED, previous_unplanned),
        ConstraintFactory::<Plan, Score>::new()
            .for_each(Plan::visits())
            .filter(|v: &Visit| v.job.is_some() && v.status() != Status::Pending)
            .group_by(
                |v: &Visit| v.job.unwrap_or(usize::MAX),
                collect_vec(|v: &Visit| (v.planned(), v.duration)),
            )
            .penalize(hard_weight(
                |_job: &usize, members: &CollectedVec<Member>| {
                    Score::of_hard(partly_planned(members))
                },
            ))
            .named(PARTLY_PLANNED),
    )
}
