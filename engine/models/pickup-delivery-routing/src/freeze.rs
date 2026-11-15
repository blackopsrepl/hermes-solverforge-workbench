//! Freezing the past: pin every stop that starts before a moment to its
//! shift and start, and the stop each driver is already driving to.

use std::sync::Arc;

use solverforge_workbench_host::EngineError;

use crate::derived::Kind;
use crate::domain::Plan;
use crate::route::routes;
use crate::time::parse_date_time;

pub fn freeze(plan: &mut Plan, until: &str) -> Result<usize, EngineError> {
    let moment = parse_date_time(until).ok_or_else(|| {
        EngineError::new(
            "bad_time",
            format!("{until:?} is not a local date-time YYYY-MM-DDTHH:MM[:SS]"),
        )
    })?;
    let routes = routes(plan);
    let wire: Vec<Option<bool>> = plan
        .drivers
        .iter()
        .flat_map(|d| &d.shifts)
        .map(|s| s.pin_next_stop_during_freeze)
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
        let pin_next = wire[s].unwrap_or(plan.parameters.pin_next_stop_during_freeze);
        let mut next_pinned = false;
        for (position, &v) in plan.shifts[s].visits.iter().enumerate() {
            let at = &routes[s].visits[position];
            if !matches!(plan.facts.visits[v].kind, Kind::Stop { .. })
                || plan.facts.visits[v].pin.is_some()
            {
                continue;
            }
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
    for (v, s, _) in pins {
        plan.visits[v].pin_shift = Some(s);
    }
    for s in whole {
        plan.shifts[s].pinned = true;
    }
    Ok(count)
}
