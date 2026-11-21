//! A shift's route: drive from the start, through each visit and break in
//! order, to the end, on the shift's working calendar. Computes arrival,
//! service and every per-visit cost.
//!
//! The drive is a step from the state after one element to the state after
//! the next. An element's cascading slot takes one step from the slot
//! before it, and the views read the slots, so the score and what the user
//! sees never time a visit differently. A visit of a multi-vehicle group starts (or ends) together
//! with the group's other planned visits; a visit waits on the departure
//! of the visits it depends on.

use crate::constraints::{
    Rule, BROKEN_LINK, DAY_COST, HOUR, MOVED, PIN_SHIFT_COST, PIN_START_WEIGHT, VISIT_RULES,
};
use crate::derived::{meters, Coordination, Facts, Kind, Pooling, ShiftFact, Window};
use crate::domain::Plan;
use solverforge_workbench_host::driving::{after_stop, on_the_road};
use solverforge_workbench_host::stock::{consume, MAX_PARTS};

/// One element as the route plans it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct At {
    pub arrival: i64,
    pub start: i64,
    pub end: i64,
    pub travel: i64,
    pub distance: i64,
    pub wait: i64,
    pub unreachable: bool,
    /// Service seconds after multipliers and adjustments.
    pub service: i64,
}

/// Where a shift's route stands after an element: the drive so far.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct State {
    /// The place the technician is at.
    pub place: usize,
    /// When the technician is free to leave.
    pub clock: i64,
    /// A visit was served already.
    pub active: bool,
    /// An element (visit or break) came before.
    pub items: bool,
    /// When the technician sets off: just in time for the first visit.
    pub start: i64,
    /// When the last visit served so far ends.
    pub last_departure: i64,
    pub travel: i64,
    pub distance: i64,
    pub unreachable_legs: i64,
    /// Visits served, their service seconds, and the travel and service
    /// seconds of the movable ones.
    pub visits: i64,
    pub service: i64,
    pub movable: i64,
    /// The last visit served (breaks skipped).
    pub last_visit: Option<usize>,
    /// The working span (day) of the last visit, whether it was beyond the
    /// technician's reach from home that day, and the nights spent out
    /// between two visits both within reach.
    pub day: usize,
    pub last_far: bool,
    pub overnights: i64,
    /// Units of each part the route's visits used up so far.
    pub parts_used: [i64; MAX_PARTS],
    /// Seconds driven since the last rest.
    pub driven: i64,
}

/// The drive back to the end location, after the last element, and what
/// the whole route costs should it end there.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct End {
    pub travel: i64,
    pub distance: i64,
    pub unreachable: bool,
    pub arrival: i64,
    /// Travel and service seconds of an active route, and its vehicle's
    /// worked, available and plan seconds, for fairness.
    pub work: i64,
    pub history: (i64, i64, i64),
    /// Amounts of the route rules, by [`route_index`].
    pub amounts: [i64; ROUTE_RULES.len()],
}

/// Rules scored on a shift's last element, from its [`End`].
pub const ROUTE_RULES: [Rule; 18] = [
    Rule::MovableBalance,
    Rule::MaxServiceHard,
    Rule::MaxServiceSoft,
    Rule::MaxTravelHard,
    Rule::MaxTravelSoft,
    Rule::MaxVisitsHard,
    Rule::MaxVisitsSoft,
    Rule::TravelDistance,
    Rule::TravelTime,
    Rule::InactiveDistance,
    Rule::InactiveTime,
    Rule::ReachableItinerary,
    Rule::Overnight,
    Rule::LastDepartureHard,
    Rule::LastDepartureSoft,
    Rule::EndHard,
    Rule::EndSoft,
    Rule::ShiftCost,
];

/// The position of a route rule in [`End::amounts`].
pub const fn route_index(rule: Rule) -> usize {
    let mut i = 0;
    while i < ROUTE_RULES.len() {
        if ROUTE_RULES[i] as usize == rule as usize {
            return i;
        }
        i += 1;
    }
    panic!("not a route rule")
}

impl End {
    pub fn amount(&self, rule: Rule) -> i64 {
        self.amounts[route_index(rule)]
    }
}

/// An element's cascading slot: the element as planned, the state after
/// it, the drive to the end should the shift stop there, and the amount of
/// every per-visit rule.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Slot {
    /// On the bench: no time, no place; only the costs of being unplanned.
    pub benched: bool,
    pub shift: usize,
    pub vehicle: usize,
    pub at: At,
    pub state: State,
    pub end: End,
    pub amounts: [i64; VISIT_RULES],
}

impl Slot {
    pub fn amount(&self, rule: Rule) -> i64 {
        self.amounts[rule.index()]
    }
}

/// A visit's arrival and earliest start, before it is served.
#[derive(Debug, Clone, Copy, Default)]
struct Ready {
    arrival: i64,
    travel: i64,
    /// Seconds of rest the drive takes, and the seconds driven since the
    /// last rest on arrival.
    rest: i64,
    driven: i64,
    distance: i64,
    unreachable: bool,
    /// Earliest start: arrival, dependencies and window opening.
    earliest: i64,
    window: Option<Window>,
    service: i64,
}

/// Shift `s` before its first element.
pub fn begin(facts: &Facts, s: usize) -> State {
    let shift = &facts.shifts[s];
    State {
        place: shift.start_location,
        clock: shift.min_start,
        start: shift.min_start,
        ..State::default()
    }
}

/// The window a visit is served in: the first one it can still start and
/// end in from `earliest`, else the last.
fn choose(windows: &[Window], earliest: i64, duration: i64) -> Option<Window> {
    windows
        .iter()
        .copied()
        .find(|w| {
            let start = earliest.max(w.min_start);
            w.max_start.is_none_or(|m| start <= m) && start + duration <= w.max_end
        })
        .or_else(|| windows.last().copied())
}

/// Skills (and their levels and multipliers) shift `s` holds at `t`, and
/// whether it holds tag `tag` then.
fn held(shift: &ShiftFact, skill: usize, t: i64) -> Option<(i64, Option<f64>)> {
    shift
        .skills
        .iter()
        .chain(
            shift
                .temporary_skills
                .iter()
                .filter(|(from, to, _)| (*from..*to).contains(&t))
                .flat_map(|(_, _, skills)| skills),
        )
        .filter(|h| h.skill == skill)
        .map(|h| (h.level, h.multiplier))
        .max_by_key(|&(level, _)| level)
}

fn has_tag(shift: &ShiftFact, tag: usize, t: i64) -> bool {
    shift.tags.contains(&tag)
        || shift
            .temporary_tags
            .iter()
            .any(|(from, to, tags)| (*from..*to).contains(&t) && tags.contains(&tag))
}

/// Service seconds of visit `v` on shift `s` starting at `t`, coming from
/// `place`.
fn service(facts: &Facts, s: usize, v: usize, place: usize, t: i64) -> i64 {
    let fact = &facts.visits[v];
    let shift = &facts.shifts[s];
    let mut seconds = fact.service + fact.adjustment;
    for &(skill, part) in &fact.parts {
        if let Some((_, Some(multiplier))) = held(shift, skill, t) {
            seconds += (part as f64 * multiplier).round() as i64 - part;
        }
    }
    if fact.location != Some(place) {
        seconds += fact.first_on_location;
    }
    seconds.max(0)
}

/// Visit `v` reached by shift `s` from `state`, not before `not_before`.
fn ready(facts: &Facts, s: usize, state: &State, v: usize, not_before: i64) -> Ready {
    let shift = &facts.shifts[s];
    let fact = &facts.visits[v];
    let to = fact.location.unwrap_or(state.place);
    let depart = fact
        .min_start_travel
        .map_or(state.clock, |m| state.clock.max(m));
    let (travel, distance, unreachable) =
        match facts.travel.trip(shift.vehicle, state.place, to, depart) {
            Some((secs, m)) => (shift.drive(secs), m, false),
            None => (0, 0, true),
        };
    let (rest, driven) = on_the_road(shift.driving_rest, state.driven, travel);
    let mut arrival = shift.advance(depart, travel + rest);
    if !state.active {
        if let Some(m) = shift.min_first_arrival {
            arrival = arrival.max(m);
        }
    }
    let base = service(facts, s, v, state.place, arrival);
    let earliest = arrival.max(not_before);
    let window = choose(&fact.windows, earliest, base);
    let earliest = shift.at_work(window.map_or(earliest, |w| earliest.max(w.min_start)));
    Ready {
        arrival,
        travel,
        rest,
        driven,
        distance,
        unreachable,
        earliest,
        window,
        service: service(facts, s, v, state.place, earliest),
    }
}

/// Visit `v` served by shift `s` from `state`, reached as `ready` and
/// starting at `start`: the visit as planned, the state after it, and its
/// per-visit amounts that need no other visit.
fn serve(
    facts: &Facts,
    s: usize,
    state: &State,
    v: usize,
    r: &Ready,
    start: i64,
    amounts: &mut [i64; VISIT_RULES],
) -> (At, State) {
    let shift = &facts.shifts[s];
    let fact = &facts.visits[v];
    let fit = facts.fit(v, s);
    let mut add = |rule: Rule, amount: i64| amounts[rule.index()] += amount;
    let mut start = start;
    if let Some((_, kept)) = fact.pin {
        // A pinned visit waits for its kept start; arriving later moves it.
        start = start.max(kept);
        let delay = start - kept;
        add(Rule::PinStartDelay, delay.min(facts.pin_allowed_delay));
        add(
            Rule::PinStart,
            (delay - facts.pin_allowed_delay).max(0) * PIN_START_WEIGHT,
        );
    }
    let end = shift.advance(start, r.service);
    let mut next = *state;
    let mut arrival = r.arrival;
    if !state.items {
        // The technician sets off just in time to start the first visit.
        next.start = (start - r.travel - r.rest).max(shift.min_start);
        arrival = arrival.max((next.start + r.travel + r.rest).min(start));
    }
    let at = At {
        arrival,
        start,
        end,
        travel: r.travel,
        distance: r.distance,
        wait: (start - arrival).max(0),
        unreachable: r.unreachable,
        service: r.service,
    };
    // Days a linked shift spends out: a night between two visits both
    // within reach of home costs.
    let day = shift.span(start);
    let far = shift
        .components
        .get(day)
        .is_some_and(|&c| facts.fit(v, c).far);
    if state.active && day > state.day && !state.last_far && !far {
        next.overnights += 1;
    }
    next.day = day;
    next.last_far = far;
    // Requirements on the shift serving it.
    let missing_tags = fact
        .tags
        .iter()
        .filter(|&&t| !has_tag(shift, t, start))
        .count() as i64;
    add(Rule::TagsMissing, missing_tags * HOUR);
    if let Some(needs) = facts.visit_parts.get(v).filter(|n| !n.is_empty()) {
        let stock = facts.shift_stock.get(s).map_or(&[][..], Vec::as_slice);
        let short = consume(&mut next.parts_used, stock, needs);
        add(Rule::PartsMissing, short * HOUR);
    }
    let (mut missing, mut above) = (0, 0);
    for &(skill, min) in &fact.skills {
        match held(shift, skill, start) {
            None => missing += min.unwrap_or(1),
            Some((level, _)) => {
                let min = min.unwrap_or(1);
                missing += (min - level).max(0);
                above += (level - min).max(0);
            }
        }
    }
    add(Rule::SkillsMissing, missing * HOUR);
    add(Rule::UnnecessarySkills, above * r.service);
    add(Rule::NotRequiredVehicle, i64::from(fit.not_required) * HOUR);
    add(Rule::ProhibitedVehicle, i64::from(fit.prohibited) * HOUR);
    add(Rule::PreferredVehicle, fit.preferred * r.service);
    add(Rule::OutsideArea, i64::from(fit.outside_required) * HOUR);
    add(Rule::OutsidePreferredArea, i64::from(fit.outside_preferred));
    let rating = facts.vehicles[shift.vehicle].rating;
    add(Rule::Rating, (rating * r.service as f64).round() as i64);
    add(Rule::Profit, fact.profit);
    // Time.
    if let Some(w) = r.window {
        add(
            Rule::WindowStart,
            w.max_start.map_or(0, |m| (start - m).max(0)),
        );
        add(Rule::WindowEnd, (end - w.max_end).max(0));
    }
    if !fact.preferred_windows.is_empty() {
        let outside = fact
            .preferred_windows
            .iter()
            .map(|w| (end - start) - crate::time::overlap(start, end, w.min_start, w.max_end))
            .min()
            .unwrap_or(0);
        add(Rule::PreferredWindow, outside);
    }
    if let Some(sla) = fact.sla {
        add(Rule::SlaEnd, (end - sla).max(0));
        let from = facts.window.0;
        add(
            Rule::SlaEarlier,
            (1000 * (end - from) / (sla - from).max(1)).max(0),
        );
    }
    if let Some(day) = fact.earliest_day {
        add(
            Rule::EarliestDay,
            (start.div_euclid(crate::time::DAY) - day).max(0) * DAY_COST,
        );
    }
    if fact.at_risk {
        if let Some(limit) = shift.max_last_departure.or(shift.max_end) {
            add(
                Rule::CompletionRisk,
                (end - (limit - facts.risk_margin)).max(0),
            );
        }
    }
    add(Rule::Waiting, at.wait);
    if let Some(limit) = shift.max_travel_per_visit {
        add(Rule::MaxTravelPerVisit, (r.travel - limit).max(0));
    }
    add(Rule::ReachableVisit, i64::from(r.unreachable) * HOUR);
    if state.active {
        if let Some(to) = fact.location {
            let toward = |from: usize| {
                facts
                    .travel
                    .leg_of(shift.vehicle, from, shift.end_location)
                    .map(|l| l.1)
            };
            if let (Some(before), Some(after)) = (toward(state.place), toward(to)) {
                add(Rule::TowardsEnd, (after - before).max(0));
            }
        }
    }
    if let (Some(group), Some(last)) = (fact.consecutive, state.last_visit) {
        if facts.visits[last].consecutive == Some(group) {
            add(Rule::Consecutive, 1);
        }
    }
    colocated(facts, shift, state, fact.location, amounts);
    // Replanning.
    if let Some((original, kept)) = fact.original {
        // An input itinerary without times still counts a change of shift.
        let mut moved = if kept == i64::MIN {
            0
        } else {
            (start - kept).abs()
        };
        if original != s {
            moved += MOVED;
            if facts.shifts[original].vehicle != shift.vehicle {
                moved += MOVED;
            }
        }
        amounts[Rule::Disruption.index()] += moved;
    }
    if let Some((pinned, _)) = fact.pin {
        if pinned != s {
            amounts[Rule::PinShift.index()] += PIN_SHIFT_COST;
        }
    }
    // The state after the visit.
    next.place = fact.location.unwrap_or(state.place);
    next.clock = end;
    next.active = true;
    next.items = true;
    next.last_departure = end;
    next.travel += r.travel;
    next.distance += r.distance;
    next.unreachable_legs += i64::from(r.unreachable);
    next.visits += 1;
    next.service += r.service;
    if fact.movable {
        next.movable += r.travel + r.service;
    }
    next.last_visit = Some(v);
    next.driven = after_stop(shift.driving_rest, r.driven, at.wait);
    (at, next)
}

/// One per pair of consecutive items at the same place, within the shift's
/// co-location radius.
fn colocated(
    facts: &Facts,
    shift: &ShiftFact,
    state: &State,
    location: Option<usize>,
    amounts: &mut [i64; VISIT_RULES],
) {
    let Some(here) = location else { return };
    if !state.items {
        return;
    }
    let near = state.place == here
        || (shift.colocated_radius > 0
            && meters(facts.travel.places[state.place], facts.travel.places[here])
                <= shift.colocated_radius as f64);
    if near {
        amounts[Rule::Colocated.index()] += 1;
    }
}

/// Break `v` taken by shift `s` from `state`.
fn rest(
    facts: &Facts,
    s: usize,
    state: &State,
    v: usize,
    amounts: &mut [i64; VISIT_RULES],
) -> (At, State) {
    let shift = &facts.shifts[s];
    let fact = &facts.visits[v];
    let Kind::Break(b) = &fact.kind else {
        unreachable!("rest takes breaks")
    };
    let to = fact.location.unwrap_or(state.place);
    let (travel, distance, unreachable) =
        match facts
            .travel
            .trip(shift.vehicle, state.place, to, state.clock)
        {
            Some((secs, m)) => (shift.drive(secs), m, false),
            None => (0, 0, true),
        };
    let (on_road, driven) = on_the_road(shift.driving_rest, state.driven, travel);
    let arrival = state.clock + travel + on_road;
    let start = arrival.max(b.start);
    let end = start + b.duration;
    if b.fixed {
        // Work that ran past the break's start is a conflict; the rest of
        // the delay is the drive to the break's place.
        let conflict = (state.clock - b.start).max(0);
        let late = (arrival - b.start).max(0);
        if fact.location.is_some() {
            amounts[Rule::LocatedBreakConflict.index()] += conflict;
            amounts[Rule::FixedBreakDelayed.index()] += late - conflict;
        } else {
            amounts[Rule::FixedBreakConflict.index()] += conflict;
        }
    } else if let Some(latest) = b.end {
        amounts[Rule::FloatingBreakLate.index()] += (end - latest).max(0);
    }
    colocated(facts, shift, state, fact.location, amounts);
    let mut next = *state;
    next.place = to;
    next.clock = end;
    next.items = true;
    next.travel += travel;
    next.distance += distance;
    next.unreachable_legs += i64::from(unreachable);
    next.driven = after_stop(shift.driving_rest, driven, end - arrival);
    let at = At {
        arrival,
        start,
        end,
        travel,
        distance,
        wait: start - arrival,
        unreachable,
        service: 0,
    };
    (at, next)
}

/// The drive back to the end location from `state`, and the route rules'
/// amounts should the route end there.
pub fn finish(facts: &Facts, s: usize, state: &State) -> End {
    let shift = &facts.shifts[s];
    if !state.items {
        return End {
            arrival: shift.min_start,
            ..End::default()
        };
    }
    let (travel, distance, unreachable) =
        match facts
            .travel
            .trip(shift.vehicle, state.place, shift.end_location, state.clock)
        {
            Some((secs, m)) => (shift.drive(secs), m, false),
            None => (0, 0, true),
        };
    let (rest, _) = on_the_road(shift.driving_rest, state.driven, travel);
    let arrival = shift.advance(state.clock, travel + rest);
    let mut end = End {
        travel,
        distance,
        unreachable,
        arrival,
        ..End::default()
    };
    let mut set = |rule: Rule, amount: i64| end.amounts[route_index(rule)] = amount;
    let total_travel = state.travel + travel;
    let total_distance = state.distance + distance;
    if !state.active {
        // A shift serving no visit still drives to its breaks and home.
        set(Rule::InactiveTime, total_travel);
        set(Rule::InactiveDistance, total_distance);
        return end;
    }
    let over = |value: i64, limit: Option<i64>| limit.map_or(0, |l| (value - l).max(0));
    set(Rule::TravelTime, total_travel);
    set(Rule::TravelDistance, total_distance);
    set(
        Rule::ReachableItinerary,
        (state.unreachable_legs + i64::from(unreachable)) * HOUR,
    );
    set(Rule::MaxTravelHard, over(total_travel, shift.max_travel));
    set(
        Rule::MaxTravelSoft,
        over(total_travel, shift.max_soft_travel),
    );
    set(Rule::MaxServiceHard, over(state.service, shift.max_service));
    set(
        Rule::MaxServiceSoft,
        over(state.service, shift.max_soft_service),
    );
    set(
        Rule::MaxVisitsHard,
        over(state.visits, shift.max_visits) * HOUR,
    );
    set(
        Rule::MaxVisitsSoft,
        over(state.visits, shift.max_soft_visits),
    );
    set(Rule::EndHard, over(arrival, shift.max_end));
    set(Rule::EndSoft, over(arrival, shift.max_soft_end));
    set(
        Rule::LastDepartureHard,
        over(state.last_departure, shift.max_last_departure),
    );
    set(
        Rule::LastDepartureSoft,
        over(state.last_departure, shift.max_soft_last_departure),
    );
    set(
        Rule::MovableBalance,
        (state.movable - (shift.movable_ratio * shift.working_time as f64) as i64).max(0),
    );
    set(Rule::Overnight, state.overnights * facts.overnight_cost);
    if let Some(cost) = &shift.cost {
        set(Rule::ShiftCost, cost.cost(arrival - state.start));
    }
    end.work = state.service + total_travel;
    let vehicle = &facts.vehicles[shift.vehicle];
    end.history = (
        vehicle.historical_utilized,
        vehicle.historical_capacity,
        vehicle.capacity,
    );
    end
}

/// The departure of the planned visits element `v` depends on, the latest
/// start each allows, and the amounts of broken dependencies; `None` when a
/// preceding visit is on a shift but cannot be timed.
fn waits(
    plan: &Plan,
    s: usize,
    state: &State,
    v: usize,
) -> Option<(i64, Vec<i64>, [i64; VISIT_RULES])> {
    let facts = &*plan.facts;
    let fact = &facts.visits[v];
    let mut not_before = i64::MIN;
    let mut latest = Vec::new();
    let mut amounts = [0; VISIT_RULES];
    for dep in &fact.dependencies {
        let preceding = &plan.elements[dep.preceding];
        if preceding.shift.is_none() {
            continue;
        }
        let slot = preceding.slot.as_ref()?;
        if slot.benched {
            amounts[Rule::DependencyPrerequisite.index()] += fact.service + BROKEN_LINK;
            continue;
        }
        not_before = not_before.max(facts.after(slot.at.end, dep.min));
        if let Some(max) = dep.max {
            latest.push(facts.after(slot.at.end, max));
        }
        let split = match dep.coordination {
            Coordination::None => false,
            Coordination::SameVehicle => slot.vehicle != facts.shifts[s].vehicle,
            Coordination::SameShift => slot.shift != s,
        };
        if split {
            let rule = match dep.coordination {
                Coordination::SameVehicle => Rule::DependencySameVehicle,
                _ => Rule::DependencySameShift,
            };
            amounts[rule.index()] += fact.service + BROKEN_LINK;
        }
        let before = state.last_visit;
        let pooled = before != Some(dep.preceding)
            && match dep.pooling {
                Pooling::All => false,
                Pooling::None => true,
                Pooling::Visits => before.is_some_and(|b| facts.visits[b].in_dependency()),
                Pooling::Dependencies => before.is_some_and(|b| !facts.visits[b].in_dependency()),
            };
        if pooled {
            amounts[Rule::DependencyPooling.index()] += HOUR;
        }
    }
    Some((not_before, latest, amounts))
}

/// The state before element `v` on its shift: after the element before
/// it, or the shift's start.
fn before(plan: &Plan, v: usize, s: usize) -> Option<State> {
    match plan.elements[v].route_previous {
        Some(p) => plan.elements[p].slot.as_ref().map(|slot| slot.state),
        None => Some(begin(&plan.facts, s)),
    }
}

/// What a visit left unplanned costs, rule by rule.
pub fn missing(facts: &Facts, v: usize) -> Vec<(Rule, i64)> {
    let fact = &facts.visits[v];
    let mut costs = Vec::new();
    if fact.is_break() {
        return costs;
    }
    let (rule, sla_rule) = if fact.mandatory {
        (Rule::MandatoryUnplanned, Rule::MandatorySla)
    } else {
        (Rule::OptionalUnplanned, Rule::OptionalSla)
    };
    costs.push((rule, fact.weight));
    if let Some(sla) = fact.sla {
        let (from, to) = facts.window;
        costs.push((sla_rule, 1000 * (to - from) / (sla - from).max(1)));
    }
    if fact.pin.is_some() {
        costs.push((Rule::PinShift, PIN_SHIFT_COST));
    }
    if fact.original.is_some() {
        costs.push((Rule::Disruption, 2 * MOVED));
    }
    costs
}

/// Notes on every element what it costs while unplanned, for an element in
/// no list, which has no slot to carry it. Run after the facts change.
pub fn note_missing(plan: &mut Plan) {
    for v in 0..plan.elements.len() {
        plan.elements[v].missing = missing(&plan.facts, v);
    }
}

fn unplanned(facts: &Facts, v: usize) -> [i64; VISIT_RULES] {
    let mut amounts = [0; VISIT_RULES];
    for (rule, amount) in missing(facts, v) {
        amounts[rule.index()] = amount;
    }
    amounts
}

/// The slot of element `v`: `None` while it is in no list, or when it
/// cannot be timed (it waits, directly or through its group, on a visit
/// after it).
pub fn slot(plan: &Plan, v: usize) -> Option<Slot> {
    let facts = &*plan.facts;
    let element = &plan.elements[v];
    let s = element.shift?;
    let shift = facts.shifts.get(s)?;
    if shift.bench {
        return Some(Slot {
            benched: true,
            shift: s,
            vehicle: 0,
            at: At::default(),
            state: State::default(),
            end: End::default(),
            amounts: unplanned(facts, v),
        });
    }
    let state = before(plan, v, s)?;
    let fact = &facts.visits[v];
    let mut amounts = [0; VISIT_RULES];
    let (at, next) = if fact.is_break() {
        rest(facts, s, &state, v, &mut amounts)
    } else {
        let (not_before, latest, broken) = waits(plan, s, &state, v)?;
        amounts = broken;
        let r = ready(facts, s, &state, v, not_before);
        let start = match fact.group {
            Some(g) => joint(plan, g, v, &r)?,
            None => r.earliest,
        };
        let (at, next) = serve(facts, s, &state, v, &r, start, &mut amounts);
        for limit in latest {
            amounts[Rule::DependencyDelay.index()] += (at.start - limit).max(0);
        }
        (at, next)
    };
    let end = finish(facts, s, &next);
    Some(Slot {
        benched: false,
        shift: s,
        vehicle: shift.vehicle,
        at,
        state: next,
        end,
        amounts,
    })
}

/// The start of visit `v` of group `g`, reached as `r`: together with the
/// group's other planned visits, each reached from the element before it
/// on its own shift.
fn joint(plan: &Plan, g: usize, v: usize, r: &Ready) -> Option<i64> {
    let facts = &*plan.facts;
    let group = &facts.groups[g];
    let mut readies = vec![(r.earliest, r.service, v)];
    for &q in &group.visits {
        if q == v {
            continue;
        }
        let peer = &plan.elements[q];
        let Some(s) = peer.shift else { continue };
        if facts.shifts[s].bench {
            continue;
        }
        let state = before(plan, q, s)?;
        let (not_before, _, _) = waits(plan, s, &state, q)?;
        let rq = ready(facts, s, &state, q, not_before);
        readies.push((rq.earliest, rq.service, q));
    }
    if group.align_end {
        let common = readies.iter().map(|&(e, d, _)| e + d).max()?;
        Some((common - r.service).max(r.earliest))
    } else {
        readies.iter().map(|&(e, _, _)| e).max()
    }
}

/// One element as the views draw it.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Drawn {
    pub at: At,
    /// Untimed: it waits on a visit planned after it.
    pub untimed: bool,
}

/// A shift's planned route.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Route {
    pub elements: Vec<Drawn>,
    /// The shift serves at least one visit.
    pub active: bool,
    pub start: i64,
    pub end_arrival: i64,
    pub last_departure: i64,
    pub travel: i64,
    pub distance: i64,
    pub unreachable_legs: i64,
    pub service: i64,
    pub waiting: i64,
    pub breaks: i64,
    /// Seconds over the hard and soft end.
    pub overtime: i64,
    /// Cents.
    pub cost: i64,
}

/// Every shift's route, from the plan's slots (shadows current).
pub fn routes(plan: &Plan) -> Vec<Route> {
    plan.shifts
        .iter()
        .enumerate()
        .map(|(s, shift)| {
            let mut route = Route::default();
            if plan.facts.shifts[s].bench {
                return route;
            }
            let mut last = None;
            for &v in &shift.visits {
                let slot = plan.elements[v].planned_slot();
                let at = slot.map(|slot| slot.at).unwrap_or_default();
                route.elements.push(Drawn {
                    at,
                    untimed: slot.is_none(),
                });
                if plan.facts.visits[v].is_break() {
                    route.breaks += at.end - at.start;
                } else {
                    route.waiting += at.wait;
                }
                if slot.is_some() {
                    last = slot;
                }
            }
            if let Some(slot) = last {
                let state = &slot.state;
                route.active = state.active;
                route.start = state.start;
                route.last_departure = state.last_departure;
                route.travel = state.travel + slot.end.travel;
                route.distance = state.distance + slot.end.distance;
                route.unreachable_legs = state.unreachable_legs + i64::from(slot.end.unreachable);
                route.service = state.service;
                route.end_arrival = slot.end.arrival;
                route.overtime = slot.end.amount(Rule::EndSoft);
                route.cost = slot.end.amount(Rule::ShiftCost);
            } else {
                route.end_arrival = plan.facts.shifts[s].min_start;
            }
            route
        })
        .collect()
}
