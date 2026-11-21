//! Derives the facts, the elements and the shifts from the wire records.

use std::collections::HashMap;
use std::sync::Arc;

use crate::tools::{ToolUse, Tools};
use solverforge_workbench_host::roads::TravelMode;
use solverforge_workbench_host::stock::PartNames;
use solverforge_workbench_host::traffic;
use solverforge_workbench_host::Issue;

use crate::derived::{
    BreakFact, Coordination, CostFact, Delay, Dependency, Facts, Fit, GroupFact, Held, Kind,
    Pooling, ShiftFact, Travel, VehicleFact, VisitFact, Window,
};
use crate::domain::{Plan, Shift, Visit};
use crate::time::{parse_date, parse_date_time, DAY};
use crate::wire::{self, Alignment, Assignment, BreakKind, DateAdjuster, Limits, RateUnit};

/// Earliest start assumed for a shift that only sets its first arrival.
const LEAD_TIME: i64 = 6 * 3600;

/// Largest co-location radius, in meters.
const MAX_RADIUS: i64 = 200;

/// Parses a local date-time field, reporting a bad one.
fn at(issues: &mut Vec<Issue>, what: &str, text: &str) -> Option<i64> {
    let parsed = parse_date_time(text);
    if parsed.is_none() {
        issues.push(Issue::error(
            "bad_time",
            format!("{what}: {text:?} is not a local date-time YYYY-MM-DDTHH:MM[:SS]"),
        ));
    }
    parsed
}

fn at_opt(issues: &mut Vec<Issue>, what: &str, text: &Option<String>) -> Option<i64> {
    text.as_deref().and_then(|t| at(issues, what, t))
}

fn valid_place(p: &[f64; 2]) -> bool {
    p[0].is_finite() && p[1].is_finite() && p[0].abs() <= 90.0 && p[1].abs() <= 180.0
}

/// Whether `p` lies inside any polygon of `area` (even-odd rule).
pub fn inside(area: &wire::Area, p: [f64; 2]) -> bool {
    area.iter().any(|ring| {
        let mut odd = false;
        let n = ring.len();
        for i in 0..n {
            let (a, b) = (ring[i], ring[(i + n - 1) % n]);
            if (a[1] > p[1]) != (b[1] > p[1])
                && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
            {
                odd = !odd;
            }
        }
        odd
    })
}

/// Whether `shift` holds `skill` at level `min` (1 when unset) at some
/// moment of its working time.
fn offers_skill(shift: &ShiftFact, skill: usize, min: Option<i64>) -> bool {
    let min = min.unwrap_or(1);
    shift
        .skills
        .iter()
        .chain(shift.temporary_skills.iter().flat_map(|(_, _, held)| held))
        .any(|h| h.skill == skill && h.level >= min)
}

/// Whether `shift` holds `tag` at some moment of its working time.
fn offers_tag(shift: &ShiftFact, tag: usize) -> bool {
    shift.tags.contains(&tag)
        || shift
            .temporary_tags
            .iter()
            .any(|(_, _, tags)| tags.contains(&tag))
}

/// Every distinct place, by its coordinates.
#[derive(Default)]
struct Places {
    points: Vec<[f64; 2]>,
    index: HashMap<(u64, u64), usize>,
}

impl Places {
    fn add(&mut self, p: [f64; 2]) -> usize {
        let key = (p[0].to_bits(), p[1].to_bits());
        if let Some(&i) = self.index.get(&key) {
            return i;
        }
        self.points.push(p);
        self.index.insert(key, self.points.len() - 1);
        self.points.len() - 1
    }
}

fn travel(
    plan: &Plan,
    places: Vec<[f64; 2]>,
    days: (i64, i64),
    issues: &mut Vec<Issue>,
) -> Option<Travel> {
    let n = places.len();
    let Some(matrix) = &plan.travel_matrix else {
        // The engine measures travel along the roads before preparing a plan.
        issues.push(Issue::error(
            "no_travel",
            "the plan has no travel between its places: the engine measures it on the roads",
        ));
        return None;
    };
    let m = matrix.locations.len();
    let square = |seconds: &Vec<Vec<Option<i64>>>, meters: &Vec<Vec<Option<i64>>>| {
        seconds.len() == m
            && meters.len() == m
            && seconds.iter().chain(meters).all(|r| r.len() == m)
    };
    if !square(&matrix.seconds, &matrix.meters) {
        issues.push(Issue::error(
            "bad_travel_matrix",
            "travelMatrix: seconds and meters must be square and match locations",
        ));
        return None;
    }
    let find = |p: &[f64; 2]| {
        matrix
            .locations
            .iter()
            .position(|q| (q[0] - p[0]).abs() < 1e-6 && (q[1] - p[1]).abs() < 1e-6)
    };
    let mut at = Vec::with_capacity(n);
    for p in &places {
        match find(p) {
            Some(i) => at.push(i),
            None => {
                issues.push(Issue::error(
                    "place_not_in_travel_matrix",
                    format!("travelMatrix has no location at {p:?}"),
                ));
                return None;
            }
        }
    }
    let legs_of = |seconds: &Vec<Vec<Option<i64>>>, meters: &Vec<Vec<Option<i64>>>| {
        let mut legs = vec![None; n * n];
        for a in 0..n {
            for b in 0..n {
                legs[a * n + b] = if a == b {
                    Some((0, 0))
                } else {
                    seconds[at[a]][at[b]].zip(meters[at[a]][at[b]])
                };
            }
        }
        legs
    };
    let legs = legs_of(&matrix.seconds, &matrix.meters);
    // Vehicles that go by bicycle or on foot travel on their mode's own legs.
    let mut modes: Vec<TravelMode> = Vec::new();
    let mut vehicle_modes = Vec::with_capacity(plan.vehicles.len());
    for vehicle in &plan.vehicles {
        match vehicle.travel_mode.filter(|m| *m != TravelMode::Car) {
            None => vehicle_modes.push(0),
            Some(mode) => {
                let index = match modes.iter().position(|m| *m == mode) {
                    Some(i) => i,
                    None => {
                        modes.push(mode);
                        modes.len() - 1
                    }
                };
                vehicle_modes.push(index + 1);
            }
        }
    }
    let mut mode_legs = Vec::with_capacity(modes.len());
    for mode in &modes {
        let Some(measured) = matrix.modes.iter().find(|m| m.mode == *mode) else {
            issues.push(Issue::error(
                "mode_without_travel",
                format!(
                    "travelMatrix.modes has no travel {}: a vehicle travels that way",
                    match mode {
                        TravelMode::Foot => "on foot".to_string(),
                        other => format!("by {}", other.as_str()),
                    }
                ),
            ));
            return None;
        };
        if !square(&measured.seconds, &measured.meters) {
            issues.push(Issue::error(
                "bad_travel_matrix",
                format!(
                    "travelMatrix.modes {}: seconds and meters must be square and match locations",
                    mode.as_str()
                ),
            ));
            return None;
        }
        mode_legs.push(legs_of(&measured.seconds, &measured.meters));
    }
    let travel = Travel::new(places, legs.clone()).with_modes(mode_legs, vehicle_modes);
    if plan.traffic.is_empty() {
        return Some(travel);
    }
    let windows = match traffic::windows(&plan.traffic, days.0, days.1) {
        Ok(windows) => windows,
        Err(message) => {
            issues.push(Issue::error("bad_traffic", message));
            return None;
        }
    };
    let roads = matrix.source.as_deref() == Some(crate::roads::ROADS);
    let mut periods = Vec::with_capacity(plan.traffic.len());
    for period in &plan.traffic {
        let Some(measured) = matrix.periods.iter().find(|p| p.id == period.id) else {
            if roads && period.slowdowns.is_empty() {
                // Nothing slows the roads: travel as usual.
                periods.push(None);
                continue;
            }
            issues.push(Issue::error(
                "traffic_without_travel",
                format!(
                    "busy period {}: travelMatrix.periods has no travel for it",
                    period.id
                ),
            ));
            return None;
        };
        if !square(&measured.seconds, &measured.meters) {
            issues.push(Issue::error(
                "bad_travel_matrix",
                format!(
                    "travelMatrix.periods {}: seconds and meters must be square and match locations",
                    period.id
                ),
            ));
            return None;
        }
        let during = legs_of(&measured.seconds, &measured.meters);
        if during
            .iter()
            .zip(&legs)
            .any(|(a, b)| a.is_some() != b.is_some())
        {
            issues.push(Issue::error(
                "bad_travel_matrix",
                format!(
                    "travelMatrix.periods {}: a busy period cannot open or close a road; its \
                     missing legs must be the usual travel's",
                    period.id
                ),
            ));
            return None;
        }
        periods.push(Some(during));
    }
    if !roads && plan.traffic.iter().any(|t| !t.slowdowns.is_empty()) {
        issues.push(Issue::warning(
            "slowdowns_unused",
            "slowdowns apply to travel measured on the roads; this plan brings its own travel \
             matrix, so each busy period travels by its own matrix",
        ));
    }
    Some(travel.with_traffic(periods, windows))
}

/// A priority's weight and whether it is mandatory.
fn priority(plan: &Plan, name: &str) -> Option<(i64, bool)> {
    if let Some(custom) = plan.priority_weights.iter().find(|p| p.priority == name) {
        return Some((custom.weight, custom.assignment == Assignment::Mandatory));
    }
    let (level, mandatory) = match name.strip_prefix("opt-") {
        Some(rest) => (rest, false),
        None => (name, true),
    };
    let level: u32 = level.parse().ok().filter(|l| (1..=10).contains(l))?;
    Some((10i64.pow(10 - level), mandatory))
}

/// The connected set of each of `n` items under `links`, named by its
/// smallest item.
fn components(n: usize, links: impl Iterator<Item = (usize, usize)>) -> Vec<usize> {
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let mut parent: Vec<usize> = (0..n).collect();
    for (a, b) in links {
        let (ra, rb) = (root(&mut parent, a), root(&mut parent, b));
        parent[ra.max(rb)] = ra.min(rb);
    }
    (0..n).map(|i| root(&mut parent, i)).collect()
}

/// The visits joined by same-shift dependencies, each chain in service
/// order (a visit after every visit it waits on); notes each visit's chain.
fn chain(visits: &mut [VisitFact], first_break: usize) -> Vec<Vec<usize>> {
    let same_shift = |v: &VisitFact| {
        v.dependencies
            .iter()
            .filter(|d| d.coordination == Coordination::SameShift)
            .map(|d| d.preceding)
            .collect::<Vec<_>>()
    };
    let sets = components(
        first_break,
        (0..first_break).flat_map(|v| same_shift(&visits[v]).into_iter().map(move |p| (v, p))),
    );
    let mut members: Vec<Vec<usize>> = vec![Vec::new(); first_break];
    for v in 0..first_break {
        members[sets[v]].push(v);
    }
    let mut chains = Vec::new();
    for set in members.into_iter().filter(|m| m.len() > 1) {
        // Kahn's order, lowest index first; a cycle keeps dataset order.
        let mut left = set.clone();
        let mut ordered = Vec::with_capacity(set.len());
        while !left.is_empty() {
            let next = left
                .iter()
                .position(|&v| same_shift(&visits[v]).iter().all(|p| ordered.contains(p)))
                .unwrap_or(0);
            ordered.push(left.remove(next));
        }
        for &v in &ordered {
            visits[v].chain = Some(chains.len());
        }
        chains.push(ordered);
    }
    chains
}

/// List construction's order: the visits only a linked shift reaches
/// first, then the more important before the less, in dataset order; a
/// multi-vehicle group or a same-shift chain goes as one unit, in service
/// order, as early as its most demanding visit.
fn order(
    visits: &mut [VisitFact],
    groups: &[GroupFact],
    chains: &[Vec<usize>],
    first_break: usize,
) {
    let units = components(
        first_break,
        groups
            .iter()
            .map(|g| &g.visits)
            .chain(chains)
            .flat_map(|members| members.windows(2).map(|w| (w[0], w[1]))),
    );
    let mut rank: Vec<(i64, i64)> = vec![(1, i64::MAX); first_break];
    for v in 0..first_break {
        let fact = &visits[v];
        // Priority weights are powers of ten up to 10^9, or custom.
        let importance = 20 - i64::from(fact.weight.max(1).ilog10()).min(19);
        let unit = &mut rank[units[v]];
        unit.0 = unit.0.min(i64::from(!fact.remote));
        unit.1 = unit.1.min(importance);
    }
    for v in 0..first_break {
        let unit = units[v];
        let within = visits[v]
            .chain
            .and_then(|c| chains[c].iter().position(|&m| m == v))
            .unwrap_or(0);
        let (reach, importance) = rank[unit];
        visits[v].order =
            (reach << 50) | (importance << 40) | ((unit * 64 + within * 8 + v % 8) as i64);
    }
}

/// Names, indexed in order of first use.
#[derive(Default)]
struct Names(Vec<String>);

impl Names {
    fn of(&mut self, name: &str) -> usize {
        match self.0.iter().position(|n| n == name) {
            Some(i) => i,
            None => {
                self.0.push(name.to_string());
                self.0.len() - 1
            }
        }
    }
}

/// A shift as the wire gives it, regular or linked.
struct WireShift<'a> {
    vehicle: usize,
    limits: &'a Limits,
    required_area: Option<&'a wire::Area>,
    preferred_area: Option<&'a wire::Area>,
}

pub type Built = (Facts, Vec<Visit>, Vec<Shift>);

/// The skills a shift or temporary set holds.
fn holds(skills: &[wire::Skill], names: &mut Names) -> Vec<Held> {
    skills
        .iter()
        .map(|k| Held {
            skill: names.of(&k.name),
            level: k.level.unwrap_or(1),
            multiplier: k.multiplier,
        })
        .collect()
}

/// The limits, skills, tags, cost and travel adjustment of a shift.
fn offer(
    plan: &Plan,
    fact: &mut ShiftFact,
    limits: &Limits,
    what: &str,
    skills: &mut Names,
    tags: &mut Names,
    issues: &mut Vec<Issue>,
) {
    let secs = |m: Option<i64>| m.map(|m| m * 60);
    fact.max_travel_per_visit = secs(limits.max_travel_time_per_visit);
    fact.max_travel = secs(limits.max_travel_time);
    fact.max_soft_travel = secs(limits.max_soft_travel_time);
    fact.max_service = secs(limits.max_service_duration);
    fact.max_soft_service = secs(limits.max_soft_service_duration);
    fact.max_visits = limits.max_visits;
    fact.max_soft_visits = limits.max_soft_visits;
    fact.skills = holds(&limits.skills, skills);
    for set in &limits.temporary_skill_sets {
        if let (Some(from), Some(to)) = (at(issues, what, &set.start), at(issues, what, &set.end)) {
            fact.temporary_skills
                .push((from, to, holds(&set.skills, skills)));
        }
    }
    fact.tags = limits.tags.iter().map(|t| tags.of(t)).collect();
    for set in &limits.temporary_tag_sets {
        if let (Some(from), Some(to)) = (at(issues, what, &set.start), at(issues, what, &set.end)) {
            fact.temporary_tags
                .push((from, to, set.tags.iter().map(|t| tags.of(t)).collect()));
        }
    }
    fact.movable_ratio = limits.movable_occupation_ratio_threshold.unwrap_or(1.0);
    fact.cost = limits.cost.as_ref().map(|c| CostFact {
        fixed: (c.fixed_cost * 100.0).round() as i64,
        rates: c
            .rates
            .iter()
            .map(|r| {
                let unit = match r.unit {
                    RateUnit::Minute => 60.0,
                    RateUnit::Hour => 3600.0,
                };
                (
                    r.duration.map(|d| d * 60),
                    (r.activation_cost * 100.0).round() as i64,
                    r.cost_per_unit * 100.0 / unit,
                )
            })
            .collect(),
    });
    let adjust = limits
        .travel_time_adjustment
        .unwrap_or(plan.parameters.travel_time_adjustment);
    if !(adjust.multiplier > 0.0 && adjust.multiplier.is_finite()) || adjust.extra_time < 0 {
        issues.push(Issue::error(
            "bad_travel_adjustment",
            format!("{what}: travel multiplier must be positive and extra time not negative"),
        ));
    }
    fact.travel_multiplier = adjust.multiplier;
    fact.travel_extra = adjust.extra_time * 60;
    fact.colocated_radius = limits
        .prefer_grouping_colocated_visits_radius_meters
        .unwrap_or(
            plan.parameters
                .prefer_grouping_colocated_visits_radius_meters,
        )
        .clamp(0, MAX_RADIUS);
}

/// Builds the facts, elements and shifts, or reports why it cannot.
pub fn build(plan: &Plan, issues: &mut Vec<Issue>) -> Option<Built> {
    let errors_before = issues.len();
    let mut places = Places::default();
    let mut skills = Names::default();
    let mut tags = Names::default();
    for s in &plan.skills {
        skills.of(s);
    }
    for t in &plan.tags {
        tags.of(t);
    }

    // Ids are unique across vehicles, shifts, visits, groups and breaks.
    let mut seen: HashMap<String, &'static str> = HashMap::new();
    let mut claim = |issues: &mut Vec<Issue>, kind: &'static str, id: &str| {
        if let Some(other) = seen.insert(id.to_string(), kind) {
            issues.push(Issue::error(
                "duplicate_id",
                format!("{kind} id {id:?} is already used by a {other}"),
            ));
        }
    };
    if plan.vehicles.is_empty() {
        issues.push(Issue::error("no_vehicles", "the dataset has no vehicles"));
    }
    if plan.visits.is_empty() && plan.visit_groups.is_empty() {
        issues.push(Issue::error("no_visits", "the dataset has no visits"));
    }

    // Vehicle shifts.
    let mut shift_facts: Vec<ShiftFact> = Vec::new();
    let mut wire_shifts: Vec<WireShift> = Vec::new();
    let mut vehicle_facts = Vec::new();
    let mut shift_index: HashMap<&str, usize> = HashMap::new();
    for (vi, vehicle) in plan.vehicles.iter().enumerate() {
        claim(issues, "vehicle", &vehicle.id);
        let name = vehicle.name.clone().unwrap_or_else(|| vehicle.id.clone());
        if vehicle.shifts.is_empty() {
            issues.push(Issue::warning(
                "vehicle_without_shifts",
                format!("vehicle {:?} has no shifts", vehicle.id),
            ));
        }
        for shift in &vehicle.shifts {
            claim(issues, "shift", &shift.id);
            let what = format!("shift {:?}", shift.id);
            for p in std::iter::once(&shift.start_location).chain(&shift.end_location) {
                if !valid_place(p) {
                    issues.push(Issue::error(
                        "bad_location",
                        format!("{what}: {p:?} is not a [latitude, longitude]"),
                    ));
                }
            }
            let min_start = at_opt(issues, &what, &shift.min_start_time);
            let min_first_arrival = at_opt(issues, &what, &shift.min_first_visit_arrival_time);
            let min_start = match (min_start, min_first_arrival) {
                (Some(s), _) => s,
                (None, Some(f)) => f - LEAD_TIME,
                (None, None) => {
                    issues.push(Issue::error(
                        "shift_without_start",
                        format!("{what} sets neither minStartTime nor minFirstVisitArrivalTime"),
                    ));
                    0
                }
            };
            let max_end = at_opt(issues, &what, &shift.max_end_time);
            if max_end.is_some_and(|e| e <= min_start) {
                issues.push(Issue::error(
                    "shift_ends_before_start",
                    format!("{what} ends before it starts"),
                ));
            }
            let mut fact = ShiftFact {
                id: shift.id.clone(),
                label: format!("{name} · {}", crate::time::day_label(min_start)),
                vehicle: vi,
                start_location: places.add(shift.start_location),
                end_location: places.add(shift.end_location.unwrap_or(shift.start_location)),
                spans: vec![(min_start, i64::MAX)],
                min_start,
                min_first_arrival,
                max_end,
                max_soft_end: at_opt(issues, &what, &shift.max_soft_end_time),
                max_last_departure: at_opt(issues, &what, &shift.max_last_visit_departure_time),
                max_soft_last_departure: at_opt(
                    issues,
                    &what,
                    &shift.max_soft_last_visit_departure_time,
                ),
                ..ShiftFact::default()
            };
            offer(
                plan,
                &mut fact,
                &shift.limits,
                &what,
                &mut skills,
                &mut tags,
                issues,
            );
            shift_index.insert(&shift.id, shift_facts.len());
            shift_facts.push(fact);
            wire_shifts.push(WireShift {
                vehicle: vi,
                limits: &shift.limits,
                required_area: shift
                    .required_area
                    .as_ref()
                    .or(vehicle.required_area.as_ref()),
                preferred_area: shift
                    .preferred_area
                    .as_ref()
                    .or(vehicle.preferred_area.as_ref()),
            });
        }
        vehicle_facts.push(VehicleFact {
            id: vehicle.id.clone(),
            label: name,
            rating: vehicle
                .technician_rating
                .unwrap_or(plan.parameters.default_technician_rating),
            historical_utilized: vehicle.historical_time_utilized * 60,
            historical_capacity: vehicle.historical_time_capacity * 60,
            capacity: 0,
        });
    }

    // Linked shifts: a chain of a vehicle's shifts worked as one.
    for (vi, vehicle) in plan.vehicles.iter().enumerate() {
        for linked in &vehicle.linked_shifts {
            claim(issues, "linked shift", &linked.id);
            let what = format!("linked shift {:?}", linked.id);
            let mut chain: Vec<usize> = Vec::new();
            for link in &linked.shifts {
                claim(issues, "shift link", &link.id);
                let from = shift_index.get(link.from_shift_id.as_str()).copied();
                let to = shift_index.get(link.to_shift_id.as_str()).copied();
                match (from, to) {
                    (Some(f), Some(t))
                        if shift_facts[f].vehicle == vi && shift_facts[t].vehicle == vi =>
                    {
                        if chain.is_empty() {
                            chain.push(f);
                        }
                        if chain.last() != Some(&f) {
                            issues.push(Issue::error(
                                "broken_shift_link",
                                format!("{what}: link {:?} does not continue the chain", link.id),
                            ));
                        }
                        chain.push(t);
                    }
                    _ => issues.push(Issue::error(
                        "unknown_linked_shift",
                        format!(
                            "{what}: link {:?} names a shift not of vehicle {:?}",
                            link.id, vehicle.id
                        ),
                    )),
                }
            }
            if chain.len() < 2 {
                issues.push(Issue::error(
                    "linked_shift_too_short",
                    format!("{what} links fewer than two shifts"),
                ));
                continue;
            }
            if chain
                .windows(2)
                .any(|p| shift_facts[p[1]].min_start <= shift_facts[p[0]].min_start)
            {
                issues.push(Issue::error(
                    "linked_shifts_out_of_order",
                    format!("{what}: each linked shift must start after the one before"),
                ));
                continue;
            }
            let first = &shift_facts[chain[0]];
            let last = &shift_facts[*chain.last().expect("two or more")];
            let spans = chain
                .iter()
                .enumerate()
                .map(|(i, &c)| {
                    let part = &shift_facts[c];
                    let end = if i + 1 == chain.len() {
                        i64::MAX
                    } else {
                        part.max_last_departure
                            .or(part.max_end)
                            .unwrap_or(shift_facts[chain[i + 1]].min_start)
                    };
                    (part.min_start, end)
                })
                .collect();
            let mut fact = ShiftFact {
                id: linked.id.clone(),
                label: format!(
                    "{} · {} to {}",
                    vehicle_facts[vi].label,
                    crate::time::day_label(first.min_start),
                    crate::time::day_label(last.min_start)
                ),
                vehicle: vi,
                components: chain.clone(),
                start_location: first.start_location,
                end_location: last.end_location,
                spans,
                min_start: first.min_start,
                min_first_arrival: first.min_first_arrival,
                max_end: last.max_end,
                max_soft_end: last.max_soft_end,
                max_last_departure: last.max_last_departure,
                max_soft_last_departure: last.max_soft_last_departure,
                ..ShiftFact::default()
            };
            offer(
                plan,
                &mut fact,
                &linked.limits,
                &what,
                &mut skills,
                &mut tags,
                issues,
            );
            let l = shift_facts.len();
            for &c in &chain {
                shift_facts[c].linked_into.push(l);
            }
            shift_facts.push(fact);
            wire_shifts.push(WireShift {
                vehicle: vi,
                limits: &linked.limits,
                required_area: vehicle.required_area.as_ref(),
                preferred_area: vehicle.preferred_area.as_ref(),
            });
        }
    }
    let bench = shift_facts.len();
    shift_facts.push(ShiftFact {
        id: "unplanned".into(),
        label: "Unplanned".into(),
        spans: vec![(0, i64::MAX)],
        bench: true,
        travel_multiplier: 1.0,
        ..ShiftFact::default()
    });

    // Planning window.
    let window = match &plan.planning_window {
        Some(w) => {
            let start = at(issues, "planningWindow.startDate", &w.start_date);
            let end = at(issues, "planningWindow.endDate", &w.end_date);
            match (start, end) {
                (Some(s), Some(e)) if s < e => (s, e),
                (Some(_), Some(_)) => {
                    issues.push(Issue::error(
                        "bad_planning_window",
                        "planningWindow ends before it starts",
                    ));
                    (0, 0)
                }
                _ => (0, 0),
            }
        }
        None => {
            let real = &shift_facts[..bench];
            let start = real.iter().map(|s| s.min_start).min().unwrap_or(0);
            let end = real
                .iter()
                .map(|s| s.max_end.unwrap_or(s.min_start + DAY))
                .max()
                .unwrap_or(DAY);
            (
                start.div_euclid(DAY) * DAY,
                (end + DAY - 1).div_euclid(DAY) * DAY,
            )
        }
    };

    // Visits, plain ones first, then the members of each group.
    let risk_weight = priority(
        plan,
        &plan.parameters.visit_completion_risk_minimal_priority,
    )
    .map_or(i64::MAX, |(w, _)| w);
    let mut wire_visits: Vec<(&wire::Visit, Option<usize>)> =
        plan.visits.iter().map(|v| (v, None)).collect();
    let mut groups = Vec::new();
    for (g, group) in plan.visit_groups.iter().enumerate() {
        claim(issues, "visit group", &group.id);
        if group.visits.len() < 2 {
            issues.push(Issue::error(
                "small_visit_group",
                format!("visit group {:?} has fewer than two visits", group.id),
            ));
        }
        let first = wire_visits.len();
        wire_visits.extend(group.visits.iter().map(|v| (v, Some(g))));
        groups.push(GroupFact {
            id: group.id.clone(),
            align_end: group.alignment == Alignment::End,
            visits: (first..wire_visits.len()).collect(),
        });
    }
    let visit_index: HashMap<&str, usize> = wire_visits
        .iter()
        .enumerate()
        .map(|(i, (v, _))| (v.id.as_str(), i))
        .collect();
    let vehicle_index: HashMap<&str, usize> = plan
        .vehicles
        .iter()
        .enumerate()
        .map(|(i, v)| (v.id.as_str(), i))
        .collect();
    let mut consecutive = Names::default();
    let mut visit_facts = Vec::new();
    let mut statics: Vec<Visit> = Vec::new();
    let mut max_mandatory = 0;
    for (index, &(visit, group)) in wire_visits.iter().enumerate() {
        claim(issues, "visit", &visit.id);
        let what = format!("visit {:?}", visit.id);
        if !valid_place(&visit.location) {
            issues.push(Issue::error(
                "bad_location",
                format!(
                    "{what}: {:?} is not a [latitude, longitude]",
                    visit.location
                ),
            ));
        }
        if visit.service_duration < 0 {
            issues.push(Issue::error(
                "negative_duration",
                format!("{what} has a negative service duration"),
            ));
        }
        let parts: Vec<(usize, i64)> = visit
            .service_duration_breakdown
            .iter()
            .map(|p| (skills.of(&p.skill), p.duration * 60))
            .collect();
        if parts.iter().map(|p| p.1).sum::<i64>() > visit.service_duration * 60 {
            issues.push(Issue::error(
                "breakdown_exceeds_service",
                format!("{what}: the service breakdown is longer than the service"),
            ));
        }
        let mut windows_for = |list: &[wire::TimeWindow]| {
            let mut windows = Vec::new();
            for w in list {
                let min_start = at(issues, &what, &w.min_start_time);
                let max_end = at(issues, &what, &w.max_end_time);
                let max_start = at_opt(issues, &what, &w.max_start_time);
                if let (Some(min_start), Some(max_end)) = (min_start, max_end) {
                    if max_end <= min_start || max_start.is_some_and(|m| m < min_start) {
                        issues.push(Issue::error(
                            "bad_time_window",
                            format!("{what}: a time window closes before it opens"),
                        ));
                    }
                    windows.push(Window {
                        min_start,
                        max_start,
                        max_end,
                    });
                }
            }
            windows.sort_by_key(|w| w.min_start);
            windows
        };
        let windows = windows_for(&visit.time_windows);
        let preferred_windows = windows_for(&visit.preferred_time_windows);
        if windows
            .iter()
            .any(|w| w.max_end < w.min_start + visit.service_duration * 60)
        {
            issues.push(Issue::warning(
                "window_too_small",
                format!("{what}: a time window is shorter than the service"),
            ));
        }
        let name = visit
            .priority
            .clone()
            .unwrap_or_else(|| plan.parameters.default_priority.clone());
        let (weight, mandatory) = priority(plan, &name).unwrap_or_else(|| {
            issues.push(Issue::error(
                "unknown_priority",
                format!("{what}: priority {name:?} is neither built in nor in priorityWeights"),
            ));
            (0, true)
        });
        if mandatory {
            max_mandatory = max_mandatory.max(weight);
        }
        for (list, kind) in [
            (&visit.required_vehicles, "required"),
            (&visit.prohibited_vehicles, "prohibited"),
            (&visit.preferred_vehicles, "preferred"),
        ] {
            for id in list {
                if !vehicle_index.contains_key(id.as_str()) {
                    issues.push(Issue::warning(
                        "unknown_vehicle",
                        format!("{what} names unknown {kind} vehicle {id:?}"),
                    ));
                }
            }
        }
        let movable = match (windows.first(), windows.last()) {
            (Some(first), Some(last)) => {
                last.max_end.div_euclid(DAY) > first.min_start.div_euclid(DAY)
                    || last.max_end > window.1
            }
            _ => false,
        };
        let mut exclusions = Vec::new();
        for id in &visit.exclusions {
            match visit_index.get(id.as_str()) {
                Some(&o) if o != index => exclusions.push(o),
                _ => issues.push(Issue::warning(
                    "unknown_excluded_visit",
                    format!("{what} excludes unknown visit {id:?}"),
                )),
            }
        }
        let sla = at_opt(issues, &what, &visit.latest_sla_end_time);
        let mut element = Visit::new(index);
        element.service = visit.service_duration * 60;
        element.group = group;
        statics.push(element);
        visit_facts.push(VisitFact {
            id: visit.id.clone(),
            label: visit.name.clone().unwrap_or_else(|| visit.id.clone()),
            kind: Kind::Visit,
            location: Some(places.add(visit.location)),
            service: visit.service_duration * 60,
            parts,
            adjustment: visit.service_duration_adjustment * 60,
            first_on_location: visit
                .duration_added_for_first_visit_on_location
                .unwrap_or(plan.parameters.duration_added_for_first_visit_on_location)
                * 60,
            earliest_day: movable.then(|| windows[0].min_start.div_euclid(DAY)),
            windows,
            preferred_windows,
            skills: visit
                .required_skills
                .iter()
                .map(|k| (skills.of(&k.name), k.min_level))
                .collect(),
            tags: visit.required_tags.iter().map(|t| tags.of(t)).collect(),
            weight,
            mandatory,
            at_risk: weight >= risk_weight,
            sla,
            consecutive: visit
                .preferred_consecutive_group_id
                .as_deref()
                .map(|c| consecutive.of(c)),
            exclusions,
            profit: visit.profit.map_or(0, |p| (p * 100.0).round() as i64),
            min_start_travel: at_opt(issues, &what, &visit.min_start_travel_time),
            owner: None,
            pin: None,
            original: None,
            movable,
            remote: false,
            chain: None,
            order: 0,
            dependencies: Vec::new(),
            has_dependents: false,
            group,
        });
    }
    // Exclusions are mutual.
    for v in 0..visit_facts.len() {
        for o in visit_facts[v].exclusions.clone() {
            if !visit_facts[o].exclusions.contains(&v) {
                visit_facts[o].exclusions.push(v);
            }
        }
    }
    // Dependencies, now that every visit has its index.
    let holidays: Vec<i64> = plan
        .parameters
        .holidays
        .iter()
        .filter_map(|d| {
            let day = parse_date(d);
            if day.is_none() {
                issues.push(Issue::error(
                    "bad_date",
                    format!("parameters.holidays: {d:?} is not a date YYYY-MM-DD"),
                ));
            }
            day
        })
        .collect();
    let delay_to = |issues: &mut Vec<Issue>, what: &str, d: &wire::DelayTo| -> Option<Delay> {
        let time = parse_date_time(&format!("1970-01-01T{}", d.time));
        if time.is_none() || d.increment < 0 {
            issues.push(Issue::error(
                "bad_delay",
                format!("{what}: a delay needs a time HH:MM and a non-negative increment"),
            ));
        }
        Some(Delay::At {
            days: d.increment,
            working: d.adjuster == DateAdjuster::NextWorkingDay,
            time: time?,
        })
    };
    for (v, &(visit, _)) in wire_visits.iter().enumerate() {
        for dep in &visit.visit_dependencies {
            claim(issues, "visit dependency", &dep.id);
            let what = format!("visit {:?} dependency {:?}", visit.id, dep.id);
            let Some(&preceding) = visit_index.get(dep.preceding_visit.as_str()) else {
                issues.push(Issue::error(
                    "unknown_preceding_visit",
                    format!("{what}: no visit {:?}", dep.preceding_visit),
                ));
                continue;
            };
            if preceding == v {
                issues.push(Issue::error(
                    "self_dependency",
                    format!("{what}: a visit cannot depend on itself"),
                ));
                continue;
            }
            if dep.min_delay.is_some_and(|d| d < 0) || dep.max_delay.is_some_and(|d| d < 0) {
                issues.push(Issue::error(
                    "negative_delay",
                    format!("{what}: delays must not be negative"),
                ));
            }
            let min = match &dep.min_delay_to {
                Some(d) => delay_to(issues, &what, d).unwrap_or(Delay::After(0)),
                None => Delay::After(dep.min_delay.unwrap_or(0) * 60),
            };
            let max = match &dep.max_delay_to {
                Some(d) => delay_to(issues, &what, d),
                None => dep.max_delay.map(|m| Delay::After(m * 60)),
            };
            let pooling = match dep
                .pooling_type
                .unwrap_or(plan.parameters.default_dependency_pooling_type)
            {
                wire::PoolingType::AllowPoolingAll => Pooling::All,
                wire::PoolingType::AllowPoolingVisits => Pooling::Visits,
                wire::PoolingType::AllowPoolingDependencies => Pooling::Dependencies,
                wire::PoolingType::DisallowPooling => Pooling::None,
            };
            visit_facts[v].dependencies.push(Dependency {
                preceding,
                min,
                max,
                coordination: match dep.coordination {
                    wire::Coordination::None => Coordination::None,
                    wire::Coordination::SameVehicle => Coordination::SameVehicle,
                    wire::Coordination::SameShift => Coordination::SameShift,
                },
                pooling,
            });
            visit_facts[preceding].has_dependents = true;
            statics[v].preceding.push(preceding);
            statics[preceding].has_dependents = true;
        }
    }
    for (v, fact) in visit_facts.iter().enumerate() {
        statics[v].exclusions = fact.exclusions.clone();
    }
    // Visits linked by exclusions, and by dependencies, in small connected
    // sets: the joins that score those links match only within a set.
    let exclusion_sets = components(
        statics.len(),
        visit_facts
            .iter()
            .enumerate()
            .flat_map(|(v, f)| f.exclusions.iter().map(move |&o| (v, o))),
    );
    let dependency_sets = components(
        statics.len(),
        visit_facts
            .iter()
            .enumerate()
            .flat_map(|(v, f)| f.dependencies.iter().map(move |d| (v, d.preceding))),
    );
    for (v, element) in statics.iter_mut().enumerate() {
        element.exclusion_set = exclusion_sets[v];
        element.dependency_set = dependency_sets[v];
    }
    let first_break = visit_facts.len();

    // Breaks, after every visit.
    let mut working = vec![0i64; shift_facts.len()];
    for (s, wire_shift) in wire_shifts.iter().enumerate() {
        let fact = &shift_facts[s];
        let from = fact.min_first_arrival.unwrap_or(fact.min_start);
        let until = fact.max_last_departure.or(fact.max_end);
        let mut breaks_time = 0;
        for b in &wire_shift.limits.required_breaks {
            claim(issues, "break", &b.id);
            let what = format!("break {:?}", b.id);
            let fixed = b.kind == BreakKind::Fixed;
            let (start, end, duration) = if fixed {
                match (
                    at_opt(issues, &what, &b.start_time),
                    at_opt(issues, &what, &b.end_time),
                ) {
                    (Some(s), Some(e)) if s < e => (s, Some(e), e - s),
                    _ => {
                        issues.push(Issue::error(
                            "bad_break",
                            format!("{what}: a fixed break needs a startTime before its endTime"),
                        ));
                        (0, None, 0)
                    }
                }
            } else {
                let start = at_opt(issues, &what, &b.min_start_time);
                let end = at_opt(issues, &what, &b.max_end_time);
                match (start, b.duration) {
                    (Some(s), Some(d)) if d > 0 => (s, end, d * 60),
                    _ => {
                        issues.push(Issue::error(
                            "bad_break",
                            format!("{what}: a floating break needs minStartTime and a positive duration"),
                        ));
                        (0, None, 0)
                    }
                }
            };
            breaks_time += match (fixed, until) {
                (true, Some(until)) => crate::time::overlap(start, start + duration, from, until),
                _ => duration,
            };
            if let Some(p) = &b.location {
                if !valid_place(p) {
                    issues.push(Issue::error(
                        "bad_location",
                        format!("{what}: {p:?} is not a [latitude, longitude]"),
                    ));
                }
            }
            let index = visit_facts.len();
            let mut element = Visit::new(index);
            element.is_break = true;
            statics.push(element);
            visit_facts.push(VisitFact {
                id: b.id.clone(),
                label: b.id.clone(),
                kind: Kind::Break(BreakFact {
                    shift: s,
                    fixed,
                    start,
                    end,
                    duration,
                }),
                location: b.location.map(|p| places.add(p)),
                service: 0,
                parts: Vec::new(),
                adjustment: 0,
                first_on_location: 0,
                windows: Vec::new(),
                preferred_windows: Vec::new(),
                skills: Vec::new(),
                tags: Vec::new(),
                weight: 0,
                mandatory: false,
                at_risk: false,
                sla: None,
                consecutive: None,
                exclusions: Vec::new(),
                profit: 0,
                min_start_travel: None,
                owner: Some(s),
                pin: None,
                original: None,
                movable: false,
                remote: false,
                chain: None,
                order: i64::MIN / 2 + index as i64,
                earliest_day: None,
                dependencies: Vec::new(),
                has_dependents: false,
                group: None,
            });
        }
        working[s] = until.map_or(i64::MAX / 4, |u| (u - from - breaks_time).max(0));
    }
    for (s, fact) in shift_facts.iter_mut().enumerate() {
        fact.working_time = match fact.components.is_empty() {
            true => working[s],
            false => fact.components.iter().map(|&c| working[c]).sum(),
        };
    }
    for fact in shift_facts.iter().filter(|f| !f.bench && !f.linked()) {
        if fact.working_time < i64::MAX / 4 {
            vehicle_facts[fact.vehicle].capacity += fact.working_time;
        }
    }

    for (v, visit) in wire_visits.iter().map(|(v, _)| v).enumerate() {
        for tag in &visit.required_tags {
            let offered = shift_facts.iter().any(|s| {
                let t = tags.0.iter().position(|n| n == tag);
                t.is_some_and(|t| {
                    s.tags.contains(&t) || s.temporary_tags.iter().any(|x| x.2.contains(&t))
                })
            });
            if !offered {
                issues.push(Issue::warning(
                    "tag_not_offered",
                    format!(
                        "visit {:?} requires tag {tag:?} no shift offers",
                        visit_facts[v].id
                    ),
                ));
            }
        }
    }
    if issues[errors_before..]
        .iter()
        .any(|i| i.severity == solverforge_workbench_host::Severity::Error)
    {
        return None;
    }
    // Busy periods are laid out from the day before the first shift starts to
    // the day after the last one ends.
    let first = shift_facts.iter().map(|s| s.min_start).min().unwrap_or(0);
    let last = shift_facts
        .iter()
        .map(|s| s.max_end.unwrap_or(s.min_start))
        .max()
        .unwrap_or(first);
    let days = (first.div_euclid(DAY) - 1, last.div_euclid(DAY) + 1);
    let travel = travel(plan, places.points, days, issues)?;
    // Shared tools: which visits need which, and the drive between sites.
    if !plan.tools.is_empty() {
        let mut tool_ids: HashMap<&str, usize> = HashMap::new();
        for (t, tool) in plan.tools.iter().enumerate() {
            if tool.count == Some(0) {
                issues.push(Issue::error(
                    "bad_tool",
                    format!("tool {:?} has a count of zero", tool.id),
                ));
            }
            if tool_ids.insert(tool.id.as_str(), t).is_some() {
                issues.push(Issue::error(
                    "duplicate_id",
                    format!("tool id {:?} is used twice", tool.id),
                ));
            }
        }
        let places = travel.places.len();
        let shared = Arc::new(Tools {
            counts: plan.tools.iter().map(|t| t.count.unwrap_or(1)).collect(),
            places,
            seconds: (0..places * places)
                .map(|i| travel.leg(i / places, i % places).map_or(0, |l| l.0))
                .collect(),
        });
        let wire_visits: HashMap<&str, &crate::wire::Visit> = plan
            .visits
            .iter()
            .chain(plan.visit_groups.iter().flat_map(|g| g.visits.iter()))
            .map(|v| (v.id.as_str(), v))
            .collect();
        for (v, fact) in visit_facts.iter().enumerate() {
            let Some(visit) = wire_visits.get(fact.id.as_str()) else {
                continue;
            };
            let mut tools = Vec::new();
            for id in &visit.required_tools {
                match tool_ids.get(id.as_str()) {
                    Some(&t) => tools.push(t),
                    None => issues.push(Issue::error(
                        "unknown_tool",
                        format!(
                            "visit {:?} needs tool {id:?}, which the plan does not list",
                            visit.id
                        ),
                    )),
                }
            }
            if let (false, Some(place)) = (tools.is_empty(), fact.location) {
                statics[v].tools = Some(Arc::new(ToolUse {
                    tools,
                    place,
                    shared: Arc::clone(&shared),
                }));
            }
        }
    } else if plan
        .visits
        .iter()
        .chain(plan.visit_groups.iter().flat_map(|g| g.visits.iter()))
        .any(|v| !v.required_tools.is_empty())
    {
        issues.push(Issue::error(
            "unknown_tool",
            "visits need tools, but the plan lists none",
        ));
    }

    // How each visit fits each shift.
    let mut fits = vec![Fit::default(); first_break * shift_facts.len()];
    for (v, (visit, _)) in wire_visits.iter().enumerate() {
        for (s, fact) in shift_facts.iter().enumerate() {
            if fact.bench {
                continue;
            }
            let wire_shift = &wire_shifts[s];
            let vehicle = &plan.vehicles[wire_shift.vehicle].id;
            let preferred = visit
                .preferred_vehicles
                .iter()
                .filter(|p| *p == vehicle)
                .count() as i64
                + visit
                    .preferred_vehicles_weights
                    .iter()
                    .filter(|p| &p.id == vehicle)
                    .map(|p| p.weight)
                    .sum::<i64>();
            let place = visit.location;
            let round_trip = travel
                .leg_of(
                    fact.vehicle,
                    fact.start_location,
                    visit_facts[v].location.unwrap_or(0),
                )
                .zip(travel.leg_of(
                    fact.vehicle,
                    visit_facts[v].location.unwrap_or(0),
                    fact.end_location,
                ))
                .map(|(a, b)| fact.drive(a.0) + visit_facts[v].service + fact.drive(b.0));
            fits[v * shift_facts.len() + s] = Fit {
                not_required: !visit.required_vehicles.is_empty()
                    && !visit.required_vehicles.contains(vehicle),
                prohibited: visit.prohibited_vehicles.contains(vehicle),
                preferred,
                outside_required: wire_shift
                    .required_area
                    .is_some_and(|area| !inside(area, place)),
                outside_preferred: wire_shift
                    .preferred_area
                    .is_some_and(|area| !inside(area, place)),
                far: round_trip.is_some_and(|t| t > fact.working_time),
            };
        }
    }

    let shift_count = shift_facts.len();
    for (v, fact) in visit_facts[..first_break].iter_mut().enumerate() {
        let fit = |s: usize| &fits[v * shift_count + s];
        let single = |s: &usize| !shift_facts[*s].bench && !shift_facts[*s].linked();
        fact.remote = (0..shift_count).filter(single).all(|s| fit(s).far)
            && (0..shift_count).any(|s| shift_facts[s].linked() && !fit(s).far);
        if fact.earliest_day.is_some() {
            let (windows, skills, tags) = (&fact.windows, &fact.skills, &fact.tags);
            let may_serve = |s: &usize| {
                let (shift, fit) = (&shift_facts[*s], fit(*s));
                !shift.bench
                    && !fit.not_required
                    && !fit.prohibited
                    && !fit.outside_required
                    && skills
                        .iter()
                        .all(|&(skill, min)| offers_skill(shift, skill, min))
                    && tags.iter().all(|&tag| offers_tag(shift, tag))
            };
            let first = (0..shift_count)
                .filter(may_serve)
                .flat_map(|s| {
                    let shift = &shift_facts[s];
                    let end = shift.max_end.unwrap_or(i64::MAX);
                    windows.iter().filter_map(move |w| {
                        let start = w.min_start.max(shift.min_start);
                        let latest = w.max_start.unwrap_or(w.max_end);
                        (start < end && start < w.max_end && start <= latest).then_some(start)
                    })
                })
                .min();
            if let Some(first) = first {
                fact.earliest_day = Some(first.div_euclid(DAY));
            }
        }
    }
    let chains = chain(&mut visit_facts, first_break);
    order(&mut visit_facts, &groups, &chains, first_break);
    let overnight_cost = 10 * max_mandatory;
    let shifts = (0..shift_facts.len())
        .map(|s| {
            Shift::new(
                s,
                s == bench,
                shift_facts[s].vehicle,
                shift_facts[s].components.clone(),
                first_break,
            )
        })
        .collect();
    let bench_skills = skills.0;
    let bench_tags = tags.0;
    // Parts: what each shift loads, what each visit uses up.
    let mut part_names = PartNames::default();
    let wire_visits: HashMap<&str, &crate::wire::Visit> = plan
        .visits
        .iter()
        .chain(plan.visit_groups.iter().flat_map(|g| g.visits.iter()))
        .map(|v| (v.id.as_str(), v))
        .collect();
    let mut visit_parts = Vec::with_capacity(visit_facts.len());
    for fact in &visit_facts {
        let needs = match wire_visits.get(fact.id.as_str()) {
            Some(visit) => part_names.read(&visit.parts, &format!("visit {:?}", visit.id)),
            None => Ok(Vec::new()),
        };
        match needs {
            Ok(needs) => visit_parts.push(needs),
            Err(message) => {
                issues.push(Issue::error("bad_parts", message));
                return None;
            }
        }
    }
    let wire_shifts_by_id: HashMap<&str, &crate::wire::VehicleShift> = plan
        .vehicles
        .iter()
        .flat_map(|v| v.shifts.iter())
        .map(|s| (s.id.as_str(), s))
        .collect();
    let mut loads = Vec::with_capacity(shift_facts.len());
    for fact in &shift_facts {
        // A linked shift sets off with what its first day's shift loads.
        let id = match fact.components.first() {
            Some(&first) => shift_facts[first].id.as_str(),
            None => fact.id.as_str(),
        };
        let stock = match wire_shifts_by_id.get(id) {
            Some(shift) => part_names.read(&shift.stock, &format!("shift {:?}", shift.id)),
            None => Ok(Vec::new()),
        };
        match stock {
            Ok(stock) => loads.push(stock),
            Err(message) => {
                issues.push(Issue::error("bad_parts", message));
                return None;
            }
        }
    }
    let shift_stock = loads
        .into_iter()
        .map(|load| {
            let mut units = vec![0; part_names.names.len()];
            for (part, quantity) in load {
                units[part] += quantity;
            }
            units
        })
        .collect();
    Some((
        Facts {
            window,
            travel,
            skills: bench_skills,
            tags: bench_tags,
            vehicles: vehicle_facts,
            visits: visit_facts,
            first_break,
            groups,
            chains,
            consecutive_groups: consecutive.0,
            shifts: shift_facts,
            fits,
            overnight_cost,
            pin_allowed_delay: plan.parameters.pinned_start_allowed_delay * 60,
            risk_margin: plan
                .parameters
                .visit_completion_risk_minimal_time_to_shift_end
                * 60,
            holidays,
            parts: part_names.names,
            visit_parts,
            shift_stock,
        },
        statics,
        shifts,
    ))
}
