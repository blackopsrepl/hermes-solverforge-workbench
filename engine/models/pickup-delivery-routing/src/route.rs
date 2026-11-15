//! A shift's route: drive from the start, through each visit in order, to
//! the end. Computes arrival, service, load and every per-visit cost, and
//! the shift's totals, then writes them onto the plan.
//!
//! Runs as the shift's list-shadow update, so it sees one shift at a time:
//! every rule it feeds depends only on that shift's list.

use crate::derived::{Facts, Kind, Window};
use crate::domain::Plan;

/// Recomputes shift `s` and writes its visits and totals.
pub fn refresh(plan: &mut Plan, s: usize) {
    let facts = plan.facts.clone();
    let list = plan.shifts[s].visits.clone();
    let route = if facts.shifts.get(s).is_none_or(|shift| shift.bench) {
        // The bench drives nowhere: its visits carry no times and no costs.
        Route {
            visits: vec![At::default(); list.len()],
            ..Route::default()
        }
    } else {
        drive(&facts, s, &list)
    };
    let benched = plan.shifts[s].bench;
    for (position, at) in route.visits.iter().enumerate() {
        let visit = &mut plan.visits[list[position]];
        visit.benched = benched;
        visit.arrival = at.arrival;
        visit.start = at.start;
        visit.end = at.end;
        visit.travel = at.travel;
        visit.distance = at.distance;
        visit.wait = at.wait;
        visit.unreachable = at.unreachable;
        visit.over_capacity = at.over_capacity;
        visit.late_start = at.late_start;
        visit.late_end = at.late_end;
        visit.sla_late = at.sla_late;
        visit.earliest_day = at.earliest_day;
        visit.pin_moved = at.pin_moved;
        visit.not_required = at.not_required;
        visit.prohibited = at.prohibited;
        visit.not_preferred = at.not_preferred;
        visit.skills_missing = at.skills_missing;
        visit.tags_missing = at.tags_missing;
        visit.overqualified = at.overqualified;
        visit.break_late = at.break_late;
        visit.break_during_job = at.break_during_job;
        visit.loops = at.loops;
        visit.burden = at.burden;
        visit.pooling = at.pooling;
        visit.prohibited_ride = at.prohibited_ride;
    }
    let shift = &mut plan.shifts[s];
    shift.active = route.active;
    shift.start = route.start;
    shift.end_arrival = route.end_arrival;
    shift.last_departure = route.last_departure;
    shift.travel = route.travel;
    shift.distance = route.distance;
    shift.unreachable_legs = route.unreachable_legs;
    shift.end_late = route.end_late;
    shift.end_soft_late = route.end_soft_late;
    shift.last_late = route.last_late;
    shift.last_soft_late = route.last_soft_late;
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
#[derive(Debug, Clone, Copy, Default, PartialEq)]
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
    pub loops: i64,
    pub burden: i64,
    pub pooling: i64,
    pub prohibited_ride: i64,
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

/// Plans shift `s` driving `list`, without touching the plan.
pub fn drive(facts: &Facts, s: usize, list: &[usize]) -> Route {
    let shift = &facts.shifts[s];
    let types = facts.capacity_types.len();
    let mut route = Route {
        visits: vec![At::default(); list.len()],
        start: shift.min_start,
        ..Route::default()
    };
    let mut place = shift.start_location;
    let mut clock = shift.min_start;
    let mut load = vec![0i64; types];
    let mut first_stop = true;
    for (position, &v) in list.iter().enumerate() {
        let fact = &facts.visits[v];
        let at = &mut route.visits[position];
        let leg = fact.location.map(|to| (to, facts.travel.leg(place, to)));
        let (travel, distance, reachable) = match leg {
            Some((_, Some((secs, meters)))) => (secs, meters, true),
            Some((_, None)) => (0, 0, false),
            None => (0, 0, true),
        };
        if !reachable {
            at.unreachable = true;
            route.unreachable_legs += 1;
        }
        at.travel = travel;
        at.distance = distance;
        route.travel += travel;
        route.distance += distance;
        match &fact.kind {
            Kind::Stop { job, .. } => {
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
                    route.start = (start - travel).max(shift.min_start);
                    at.wait = (start - (route.start + travel)).max(0);
                } else {
                    at.wait = start - arrival;
                }
                first_stop = false;
                let end = start + duration;
                if let Some(w) = window {
                    at.late_start = w.max_start.map_or(0, |m| (start - m).max(0));
                    at.late_end = (end - w.max_end).max(0);
                }
                for (k, slot) in load.iter_mut().enumerate() {
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
                route.active = true;
                route.last_departure = end;
                clock = end;
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
                clock = end;
            }
        }
        if let Some((to, _)) = leg {
            place = to;
        }
    }
    if route.active {
        let (travel, distance) = match facts.travel.leg(place, shift.end_location) {
            Some(leg) => leg,
            None => {
                route.unreachable_legs += 1;
                (0, 0)
            }
        };
        route.travel += travel;
        route.distance += distance;
        route.end_arrival = clock + travel;
        route.end_late = shift.max_end.map_or(0, |m| (route.end_arrival - m).max(0));
        route.end_soft_late = shift
            .max_soft_end
            .map_or(0, |m| (route.end_arrival - m).max(0));
        route.last_late = shift
            .max_last_departure
            .map_or(0, |m| (route.last_departure - m).max(0));
        route.last_soft_late = shift
            .max_soft_last_departure
            .map_or(0, |m| (route.last_departure - m).max(0));
    } else {
        // An idle shift drives nowhere: no travel, not even to a break.
        route.travel = 0;
        route.distance = 0;
        route.unreachable_legs = 0;
        route.start = shift.min_start;
        route.end_arrival = shift.min_start;
        for at in &mut route.visits {
            at.travel = 0;
            at.distance = 0;
            at.unreachable = false;
        }
    }
    jobs(facts, list, &mut route);
    route
}

/// Rules over the jobs a shift carries: time burden, pooling, rides past
/// a destination, and breaks while a job is on board.
fn jobs(facts: &Facts, list: &[usize], route: &mut Route) {
    // Each job on this shift: its stops' positions, in service order.
    let mut on: Vec<(usize, Vec<Option<usize>>)> = Vec::new();
    for (position, &v) in list.iter().enumerate() {
        if let Some((job, seq)) = facts.stop(v) {
            let entry = match on.iter().position(|(j, _)| *j == job) {
                Some(i) => &mut on[i],
                None => {
                    on.push((job, vec![None; facts.jobs[job].stops.len()]));
                    on.last_mut().expect("just pushed")
                }
            };
            entry.1[seq] = Some(position);
        }
    }
    // Span on board: from the first stop served to the last, by position.
    let span = |positions: &[Option<usize>]| {
        let served: Vec<usize> = positions.iter().flatten().copied().collect();
        (
            served.iter().copied().min().unwrap_or(0),
            served.iter().copied().max().unwrap_or(0),
        )
    };
    for (job, positions) in &on {
        let fact = &facts.jobs[*job];
        let (first, last) = span(positions);
        let lead = positions.iter().flatten().copied().min().unwrap_or(first);
        // Time burden: on-board time beyond direct service, for whole jobs.
        if let (Some(limit), true) = (fact.max_burden, positions.iter().all(Option::is_some)) {
            let p: Vec<usize> = positions.iter().flatten().copied().collect();
            let mut direct = 0;
            for pair in p.windows(2) {
                let a = &route.visits[pair[0]];
                let from = facts.visits[list[pair[0]]].location;
                let to = facts.visits[list[pair[1]]].location;
                let leg = match (from, to) {
                    (Some(from), Some(to)) => facts.travel.leg(from, to).map_or(0, |l| l.0),
                    _ => 0,
                };
                direct += (a.end - a.start) + leg;
            }
            let taken = route.visits[p[p.len() - 1]].start - route.visits[p[0]].start;
            route.visits[lead].burden = (taken - direct - limit).max(0);
        }
        // Rides past the next stop's place without serving it.
        let served: Vec<usize> = positions.iter().flatten().copied().collect();
        for pair in served.windows(2) {
            let (from, to) = (pair[0], pair[1]);
            if to <= from {
                continue;
            }
            let target = facts.visits[list[to]].location;
            for &v in &list[from + 1..to] {
                if facts.stop(v).is_some() && facts.visits[v].location == target {
                    route.visits[to].loops += 1;
                }
            }
        }
        // Breaks while on board, where the job or the break forbids it.
        for (q, &v) in list.iter().enumerate() {
            if q <= first || q >= last {
                continue;
            }
            if let Kind::Break(b) = &facts.visits[v].kind {
                if !fact.interruptible || b.reject_jobs {
                    let at = &route.visits[q];
                    route.visits[q].break_during_job += at.end - at.start;
                }
            }
        }
    }
    // Pairs of jobs on board together.
    for a in 0..on.len() {
        for b in a + 1..on.len() {
            let (ja, jb) = (on[a].0, on[b].0);
            let (fa, la) = span(&on[a].1);
            let (fb, lb) = span(&on[b].1);
            if la <= fb || lb <= fa {
                continue;
            }
            let together = crate::time::overlap(
                route.visits[fa].start,
                route.visits[la].start,
                route.visits[fb].start,
                route.visits[lb].start,
            )
            .max(1);
            let (job_a, job_b) = (&facts.jobs[ja], &facts.jobs[jb]);
            if job_a.no_pooling || job_b.no_pooling {
                let lead = if job_a.no_pooling { fa } else { fb };
                route.visits[lead].pooling += together;
            }
            if job_a.prohibited_jobs.contains(&jb) || job_b.prohibited_jobs.contains(&ja) {
                route.visits[fa].prohibited_ride += together;
            }
        }
    }
}
