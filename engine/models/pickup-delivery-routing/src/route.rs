//! A shift's route: drive from the start, through each visit in order, to
//! the end. Computes arrival, service, load, the jobs on board and every
//! per-visit cost.
//!
//! The drive is a step from the state after one visit to the state after
//! the next. A visit's cascading slot takes one step from the slot before
//! it; the views fold the steps over a whole list. Both run the same code,
//! so the score and what the user sees never time a visit differently.

use smallvec::SmallVec;

use crate::derived::{Facts, Kind, Window};
use crate::domain::Plan;

/// A visit's cascading slot: the visit as planned, the state after it, and
/// the drive back to the end location should the shift stop there.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Slot {
    /// On the bench: no time, no place, no cost but being unplanned.
    pub benched: bool,
    pub at: At,
    pub state: State,
    pub end: End,
}

/// The slot of visit `v`: `None` while it is in no list.
pub fn slot(plan: &Plan, v: usize) -> Option<Slot> {
    let facts = &*plan.facts;
    let visit = &plan.visits[v];
    let s = visit.shift?;
    if facts.shifts.get(s).is_none_or(|shift| shift.bench) {
        return Some(Slot {
            benched: true,
            ..Slot::default()
        });
    }
    let first;
    let before = match visit.route_previous {
        Some(p) => &plan.visits[p].slot.as_ref()?.state,
        None => {
            first = begin(facts, s);
            &first
        }
    };
    let (at, state) = self::visit(facts, s, before, v);
    let end = finish(facts, s, &state);
    Some(Slot {
        benched: false,
        at,
        state,
        end,
    })
}

/// Every shift's route, computed from the lists alone (not from shadows,
/// which only a score director keeps current); the bench's is empty.
pub fn routes(plan: &Plan) -> Vec<Route> {
    (0..plan.shifts.len())
        .map(|s| {
            if plan.facts.shifts[s].bench {
                Route {
                    visits: vec![At::default(); plan.shifts[s].visits.len()],
                    ..Route::default()
                }
            } else {
                drive(&plan.facts, s, &plan.shifts[s].visits)
            }
        })
        .collect()
}

/// Where each visit is: its shift and position, `None` when no driver
/// shift holds it.
pub fn placement(plan: &Plan) -> Vec<Option<(usize, usize)>> {
    let mut at = vec![None; plan.facts.visits.len()];
    for (s, shift) in plan.shifts.iter().enumerate() {
        if plan.facts.shifts[s].bench {
            continue;
        }
        for (position, &v) in shift.visits.iter().enumerate() {
            at[v] = Some((s, position));
        }
    }
    at
}

/// One visit as the route plans it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct At {
    pub arrival: i64,
    pub start: i64,
    pub end: i64,
    pub travel: i64,
    pub distance: i64,
    pub wait: i64,
    pub unreachable: bool,
    pub over_capacity: i64,
    pub late_start: i64,
    pub late_end: i64,
    pub sla_late: i64,
    pub earliest_day: i64,
    pub pin_moved: i64,
    pub not_required: bool,
    pub prohibited: bool,
    pub not_preferred: bool,
    pub skills_missing: i64,
    pub tags_missing: i64,
    pub overqualified: i64,
    pub break_late: i64,
    pub break_during_job: i64,
    /// Rides past the next destination of a job on board.
    pub loops: i64,
    /// On a job's last stop: seconds over its time burden.
    pub burden: i64,
    /// On a job's last stop: seconds it rode with each job still on board
    /// (at least one) that forbids pooling, or that one of the two
    /// prohibits. A job never served in full on its shift is split or
    /// partly planned, which costs more.
    pub pooling: i64,
    pub prohibited_ride: i64,
    /// Unreachable legs charged here: this visit's own, and on a shift's
    /// first stop those of the breaks before it, which an idle shift never
    /// drives.
    pub unreachable_legs: i64,
}

/// A shift's planned route.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Route {
    pub visits: Vec<At>,
    /// The shift serves at least one stop.
    pub active: bool,
    pub start: i64,
    pub end_arrival: i64,
    pub last_departure: i64,
    pub travel: i64,
    pub distance: i64,
    pub unreachable_legs: i64,
    pub end_late: i64,
    pub end_soft_late: i64,
    pub last_late: i64,
    pub last_soft_late: i64,
}

/// Hard cost per second a pinned stop starts after the start it keeps.
pub const PIN_START_WEIGHT: i64 = 100_000;

/// The window a stop is served in: the first one whose latest start is not
/// yet past at `arrival`, else the last.
fn choose(windows: &[Window], arrival: i64, duration: i64) -> Option<Window> {
    windows
        .iter()
        .copied()
        .find(|w| {
            let start = arrival.max(w.min_start);
            w.max_start.is_none_or(|m| start <= m) && start + duration <= w.max_end
        })
        .or_else(|| windows.last().copied())
}

/// Most capacity types a dataset may declare: a route's running load is a
/// fixed array, so a visit's state never reaches for the heap.
pub const MAX_LOAD_TYPES: usize = 8;

/// A job picked up and not yet served in full.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Aboard {
    pub job: usize,
    /// Stops of the job served so far.
    pub served: usize,
    /// Where the job's next stop is.
    pub next_location: Option<usize>,
    /// Start of service at the job's first stop.
    pub first_start: i64,
    /// Seconds of direct service so far: each served stop's service and the
    /// direct drive to the stop after it.
    pub direct: i64,
    /// The last served stop's place and service seconds.
    pub last_location: Option<usize>,
    pub last_service: i64,
}

/// Where a shift's route stands after a visit: the drive so far.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct State {
    /// The place the driver is at.
    pub place: usize,
    /// When the driver is free to leave.
    pub clock: i64,
    pub load: [i64; MAX_LOAD_TYPES],
    /// A stop was served already.
    pub active: bool,
    /// When the driver sets off: just in time for the first stop.
    pub start: i64,
    /// When the last stop served so far ends.
    pub last_departure: i64,
    pub travel: i64,
    pub distance: i64,
    pub unreachable_legs: i64,
    /// Unreachable legs to breaks before the first stop, charged to it.
    pub deferred_unreachable: i64,
    /// The jobs on board, in the order they were picked up.
    pub aboard: SmallVec<[Aboard; 8]>,
}

/// The drive back to the end location, after the last visit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct End {
    pub travel: i64,
    pub distance: i64,
    pub unreachable: bool,
    pub arrival: i64,
    pub late: i64,
    pub soft_late: i64,
    pub last_late: i64,
    pub last_soft_late: i64,
}

/// Shift `s` before its first visit.
pub fn begin(facts: &Facts, s: usize) -> State {
    let shift = &facts.shifts[s];
    State {
        place: shift.start_location,
        clock: shift.min_start,
        start: shift.min_start,
        ..State::default()
    }
}

/// Visit `v` driven by shift `s` from `state`: the visit as planned and the
/// state after it.
pub fn visit(facts: &Facts, s: usize, state: &State, v: usize) -> (At, State) {
    let shift = &facts.shifts[s];
    let fact = &facts.visits[v];
    let mut at = At::default();
    let mut next = state.clone();
    let leg = fact
        .location
        .map(|to| (to, facts.travel.leg(state.place, to)));
    let (travel, distance, reachable) = match leg {
        Some((_, Some((secs, meters)))) => (secs, meters, true),
        Some((_, None)) => (0, 0, false),
        None => (0, 0, true),
    };
    if !reachable {
        at.unreachable = true;
        next.unreachable_legs += 1;
    }
    at.travel = travel;
    at.distance = distance;
    next.travel += travel;
    next.distance += distance;
    let clock = state.clock;
    match &fact.kind {
        Kind::Stop { job, seq } => {
            let first_stop = !state.active;
            let fit = facts.fit(*job, s);
            let depart = fact.min_start_travel.map_or(clock, |m| clock.max(m));
            let mut arrival = depart + travel;
            if first_stop {
                if let Some(m) = shift.min_first_arrival {
                    arrival = arrival.max(m);
                }
            }
            let duration = ((fact.duration as f64) * fit.multiplier).round() as i64;
            let window = choose(&fact.windows, arrival, duration);
            let mut start = window.map_or(arrival, |w| arrival.max(w.min_start));
            if let Some((_, kept)) = fact.pin {
                // A pinned stop waits for its kept start; arriving later moves it.
                start = start.max(kept);
                at.pin_moved = (start - kept) * PIN_START_WEIGHT;
            }
            if first_stop {
                // The driver sets off just in time for the first stop.
                next.start = (start - travel).max(shift.min_start);
                at.wait = (start - (next.start + travel)).max(0);
            } else {
                at.wait = start - arrival;
            }
            let end = start + duration;
            if let Some(w) = window {
                at.late_start = w.max_start.map_or(0, |m| (start - m).max(0));
                at.late_end = (end - w.max_end).max(0);
            }
            for (k, slot) in next.load.iter_mut().enumerate().take(shift.capacity.len()) {
                *slot += fact.load[k];
                at.over_capacity += (*slot - shift.capacity[k]).max(0);
            }
            at.sla_late = fact.sla.map_or(0, |sla| (end - sla).max(0));
            at.earliest_day = fact
                .earliest_day
                .map_or(0, |day| (start.div_euclid(crate::time::DAY) - day).max(0))
                * crate::constraints::DAY_COST;
            at.not_required = fit.not_required;
            at.prohibited = fit.prohibited;
            at.not_preferred = fit.not_preferred;
            at.skills_missing = fit.skills_missing;
            at.tags_missing = fit.tags_missing;
            at.overqualified = fit.overqualified * duration;
            at.arrival = arrival;
            at.start = start;
            at.end = end;
            ride(facts, &mut next, *job, *seq, v, start, end, &mut at);
            at.unreachable_legs = i64::from(at.unreachable) + state.deferred_unreachable;
            next.deferred_unreachable = 0;
            next.active = true;
            next.last_departure = end;
            next.clock = end;
        }
        Kind::Break(b) => {
            let arrival = clock + travel;
            let (start, end) = if b.fixed {
                at.break_late = (arrival - b.start).max(0);
                let start = arrival.max(b.start);
                (start, start + b.duration)
            } else {
                let start = arrival.max(b.start);
                let end = start + b.duration;
                at.break_late = b.end.map_or(0, |m| (end - m).max(0));
                (start, end)
            };
            at.arrival = arrival;
            at.start = start;
            at.end = end;
            at.wait = start - arrival;
            let held = if b.reject_jobs {
                state.aboard.len()
            } else {
                state
                    .aboard
                    .iter()
                    .filter(|a| !facts.jobs[a.job].interruptible)
                    .count()
            };
            at.break_during_job = held as i64 * (end - start);
            if state.active {
                at.unreachable_legs = i64::from(at.unreachable);
            } else {
                next.deferred_unreachable += i64::from(at.unreachable);
            }
            next.clock = end;
        }
    }
    if let Some((to, _)) = leg {
        next.place = to;
    }
    (at, next)
}

/// The drive back to the end location from `state`, the state after the
/// last visit; nothing for a shift that served no stop.
pub fn finish(facts: &Facts, s: usize, state: &State) -> End {
    let shift = &facts.shifts[s];
    if !state.active {
        return End {
            arrival: shift.min_start,
            ..End::default()
        };
    }
    let (travel, distance, unreachable) = match facts.travel.leg(state.place, shift.end_location) {
        Some((travel, distance)) => (travel, distance, false),
        None => (0, 0, true),
    };
    let arrival = state.clock + travel;
    End {
        travel,
        distance,
        unreachable,
        arrival,
        late: shift.max_end.map_or(0, |m| (arrival - m).max(0)),
        soft_late: shift.max_soft_end.map_or(0, |m| (arrival - m).max(0)),
        last_late: shift
            .max_last_departure
            .map_or(0, |m| (state.last_departure - m).max(0)),
        last_soft_late: shift
            .max_soft_last_departure
            .map_or(0, |m| (state.last_departure - m).max(0)),
    }
}

/// Plans shift `s` driving `list`, without touching the plan.
pub fn drive(facts: &Facts, s: usize, list: &[usize]) -> Route {
    let mut route = Route {
        visits: vec![At::default(); list.len()],
        ..Route::default()
    };
    let mut state = begin(facts, s);
    for (position, &v) in list.iter().enumerate() {
        let (at, next) = visit(facts, s, &state, v);
        route.visits[position] = at;
        state = next;
    }
    let end = finish(facts, s, &state);
    route.active = state.active;
    route.start = state.start;
    route.last_departure = state.last_departure;
    if state.active {
        route.travel = state.travel + end.travel;
        route.distance = state.distance + end.distance;
        route.unreachable_legs = state.unreachable_legs + i64::from(end.unreachable);
        route.end_arrival = end.arrival;
        route.end_late = end.late;
        route.end_soft_late = end.soft_late;
        route.last_late = end.last_late;
        route.last_soft_late = end.last_soft_late;
    } else {
        // An idle shift drives nowhere: no travel, not even to a break.
        route.end_arrival = end.arrival;
        for at in &mut route.visits {
            at.travel = 0;
            at.distance = 0;
            at.unreachable = false;
        }
    }
    route
}

/// Whether jobs `a` and `b` may not ride together: (pooling, prohibited).
fn conflict(facts: &Facts, a: usize, b: usize) -> (bool, bool) {
    let (ja, jb) = (&facts.jobs[a], &facts.jobs[b]);
    (
        ja.no_pooling || jb.no_pooling,
        ja.prohibited_jobs.contains(&b) || jb.prohibited_jobs.contains(&a),
    )
}

/// Stop `seq` of job `job` (visit `v`, served from `start` to `end`) on the
/// ride: rides past other jobs' destinations, the job boarding or moving on
/// to its next stop, and its time burden once its last stop is served.
#[allow(clippy::too_many_arguments)]
fn ride(
    facts: &Facts,
    next: &mut State,
    job: usize,
    seq: usize,
    v: usize,
    start: i64,
    end: i64,
    at: &mut At,
) {
    let fact = &facts.jobs[job];
    let location = facts.visits[v].location;
    at.loops = next
        .aboard
        .iter()
        .filter(|a| a.job != job && location.is_some() && a.next_location == location)
        .count() as i64;
    let next_location = fact
        .stops
        .get(seq + 1)
        .and_then(|&s| facts.visits[s].location);
    let service = end - start;
    let position = match next.aboard.iter().position(|a| a.job == job) {
        Some(i) => {
            let a = &mut next.aboard[i];
            let leg = match (a.last_location, location) {
                (Some(from), Some(to)) => facts.travel.leg(from, to).map_or(0, |l| l.0),
                _ => 0,
            };
            a.direct += a.last_service + leg;
            a.served += 1;
            a.next_location = next_location;
            a.last_location = location;
            a.last_service = service;
            i
        }
        None => {
            next.aboard.push(Aboard {
                job,
                served: 1,
                next_location,
                first_start: start,
                direct: 0,
                last_location: location,
                last_service: service,
            });
            next.aboard.len() - 1
        }
    };
    let a = next.aboard[position];
    if a.served >= fact.stops.len() {
        if let Some(limit) = fact.max_burden {
            at.burden = (start - a.first_start - a.direct - limit).max(0);
        }
        // The job leaves: every job still on board rode with it from the
        // later of their first stops until now, at least a second.
        for other in &next.aboard {
            if other.job == job {
                continue;
            }
            let (pooling, prohibited) = conflict(facts, job, other.job);
            let together = (start - a.first_start.max(other.first_start)).max(1);
            at.pooling += i64::from(pooling) * together;
            at.prohibited_ride += i64::from(prohibited) * together;
        }
        next.aboard.remove(position);
    }
}
