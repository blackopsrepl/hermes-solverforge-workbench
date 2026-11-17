//! Freezing the past: pin every visit that starts before a moment to its
//! shift and start, and the visit each technician is already driving to.

use std::sync::Arc;

use solverforge_workbench_host::EngineError;

use crate::derived::Kind;
use crate::domain::Plan;
use crate::present::timed;
use crate::time::parse_date_time;

pub fn freeze(plan: &mut Plan, until: &str) -> Result<usize, EngineError> {
    let moment = parse_date_time(until).ok_or_else(|| {
        EngineError::new(
            "bad_time",
            format!("{until:?} is not a local date-time YYYY-MM-DDTHH:MM[:SS]"),
        )
    })?;
    let timed = timed(plan);
    let wire: Vec<Option<bool>> = plan
        .vehicles
        .iter()
        .flat_map(|v| {
            v.shifts
                .iter()
                .map(|s| s.limits.pin_next_visit_during_freeze)
        })
        .chain(plan.vehicles.iter().flat_map(|v| {
            v.linked_shifts
                .iter()
                .map(|s| s.limits.pin_next_visit_during_freeze)
        }))
        .collect();
    let mut pins = Vec::new();
    let mut whole = Vec::new();
    for (s, fact) in plan.facts.shifts.iter().enumerate() {
        if fact.bench || plan.shifts[s].pinned {
            continue;
        }
        // A shift that ended before the moment keeps all of it, once its
        // breaks are in its list (a pinned list can take nothing new).
        let breaks_in = plan
            .facts
            .visits
            .iter()
            .enumerate()
            .all(|(v, f)| match &f.kind {
                Kind::Break(b) if b.shift == s => plan.shifts[s].visits.contains(&v),
                _ => true,
            });
        if breaks_in && fact.max_end.is_some_and(|end| end <= moment) {
            whole.push(s);
        }
        let pin_next = wire[s].unwrap_or(plan.parameters.pin_next_visit_during_freeze);
        let mut next_pinned = false;
        for &v in &plan.shifts[s].visits {
            let fact = &plan.facts.visits[v];
            if fact.is_break() || fact.pin.is_some() {
                continue;
            }
            let Some(at) = timed.elements[v].at() else {
                break;
            };
            let started = at.start < moment;
            let on_the_way = pin_next && !next_pinned && at.arrival - at.travel < moment;
            if started || on_the_way {
                if !started {
                    next_pinned = true;
                }
                pins.push((v, s, at.start));
            } else {
                break;
            }
        }
    }
    let count = pins.len();
    let facts = Arc::make_mut(&mut plan.facts);
    for &(v, s, start) in &pins {
        facts.visits[v].pin = Some((s, start));
    }
    for s in whole {
        plan.shifts[s].pinned = true;
    }
    crate::route::note_missing(plan);
    Ok(count)
}
