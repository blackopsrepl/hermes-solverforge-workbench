//! Derives the facts, the visits and the shifts from the wire records.

use std::collections::{BTreeSet, HashMap};

use solverforge_workbench_host::roads::TravelMode;
use solverforge_workbench_host::stock::PartNames;
use solverforge_workbench_host::traffic;
use solverforge_workbench_host::Issue;

use crate::derived::{BreakFact, Facts, Fit, JobFact, Kind, ShiftFact, Travel, VisitFact, Window};
use crate::domain::{Plan, Shift, Visit};
use crate::time::{parse_date_time, DAY};
use crate::wire::{Assignment, BreakKind, BreakPolicy, Interruption};

/// Earliest start assumed for a shift that only sets its first arrival.
const LEAD_TIME: i64 = 6 * 3600;

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
    let mut vehicle_modes = Vec::with_capacity(plan.drivers.len());
    for vehicle in &plan.drivers {
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

pub type Built = (Facts, Vec<Visit>, Vec<Shift>);

/// Builds the facts, visits and shifts, or reports why it cannot.
pub fn build(plan: &Plan, issues: &mut Vec<Issue>) -> Option<Built> {
    let errors_before = issues.len();
    let mut places = Places::default();

    // Ids are unique across drivers, shifts, jobs, stops and breaks.
    let mut seen: HashMap<String, &'static str> = HashMap::new();
    let mut claim = |issues: &mut Vec<Issue>, kind: &'static str, id: &str| {
        if let Some(other) = seen.insert(id.to_string(), kind) {
            issues.push(Issue::error(
                "duplicate_id",
                format!("{kind} id {id:?} is already used by a {other}"),
            ));
        }
    };
    if plan.drivers.is_empty() {
        issues.push(Issue::error("no_drivers", "the dataset has no drivers"));
    }
    if plan.jobs.is_empty() {
        issues.push(Issue::error("no_jobs", "the dataset has no jobs"));
    }

    // Capacity types, skills and tags by name.
    let capacity_types: Vec<String> = BTreeSet::<String>::from_iter(
        plan.drivers
            .iter()
            .flat_map(|d| &d.shifts)
            .flat_map(|s| s.capacities.iter().map(|c| c.kind.clone()))
            .chain(plan.jobs.iter().flat_map(|j| {
                j.demands.iter().map(|d| d.kind.clone()).chain(
                    j.stops
                        .iter()
                        .flat_map(|s| s.demands.iter().map(|d| d.kind.clone())),
                )
            })),
    )
    .into_iter()
    .collect();
    if capacity_types.len() > crate::route::MAX_LOAD_TYPES {
        issues.push(Issue::error(
            "too_many_capacity_types",
            format!(
                "the dataset declares {} capacity types; at most {} are supported",
                capacity_types.len(),
                crate::route::MAX_LOAD_TYPES
            ),
        ));
        return None;
    }
    let type_of = |name: &str| capacity_types.iter().position(|t| t == name);

    // Shifts.
    let mut shift_facts = Vec::new();
    let mut driver_ids = Vec::new();
    let mut breaks: Vec<(usize, &crate::wire::Break)> = Vec::new();
    for (d, driver) in plan.drivers.iter().enumerate() {
        claim(issues, "driver", &driver.id);
        driver_ids.push(driver.id.clone());
        if driver.shifts.is_empty() {
            issues.push(Issue::warning(
                "driver_without_shifts",
                format!("driver {:?} has no shifts", driver.id),
            ));
        }
        for shift in &driver.shifts {
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
            let min_first_arrival = at_opt(issues, &what, &shift.min_first_stop_arrival_time);
            let min_start = match (min_start, min_first_arrival) {
                (Some(s), _) => s,
                (None, Some(f)) => f - LEAD_TIME,
                (None, None) => {
                    issues.push(Issue::error(
                        "shift_without_start",
                        format!("{what} sets neither minStartTime nor minFirstStopArrivalTime"),
                    ));
                    0
                }
            };
            let max_end = at_opt(issues, &what, &shift.max_end_time);
            let max_soft_end = at_opt(issues, &what, &shift.max_soft_end_time);
            let max_last_departure = at_opt(issues, &what, &shift.max_last_stop_departure_time);
            let max_soft_last_departure =
                at_opt(issues, &what, &shift.max_soft_last_stop_departure_time);
            if max_end.is_some_and(|e| e <= min_start) {
                issues.push(Issue::error(
                    "shift_ends_before_start",
                    format!("{what} ends before it starts"),
                ));
            }
            if max_soft_end.is_some() && max_end.is_none() {
                issues.push(Issue::warning(
                    "soft_end_without_hard_end",
                    format!("{what} sets maxSoftEndTime without maxEndTime"),
                ));
            }
            let mut capacity = vec![0; capacity_types.len()];
            for c in &shift.capacities {
                if c.quantity < 0 {
                    issues.push(Issue::error(
                        "negative_capacity",
                        format!("{what}: capacity {:?} is negative", c.kind),
                    ));
                }
                if let Some(k) = type_of(&c.kind) {
                    capacity[k] = c.quantity;
                }
            }
            for b in &shift.required_breaks {
                breaks.push((shift_facts.len(), b));
            }
            let start_location = places.add(shift.start_location);
            let end_location = places.add(shift.end_location.unwrap_or(shift.start_location));
            let driving_rest = match shift.driving_rest.map(|r| r.seconds(&what)) {
                Some(Ok(rest)) => Some(rest),
                Some(Err(message)) => {
                    issues.push(Issue::error("bad_driving_rest", message));
                    None
                }
                None => None,
            };
            shift_facts.push(ShiftFact {
                id: shift.id.clone(),
                label: format!(
                    "{} · {}",
                    driver.name.as_deref().unwrap_or(&driver.id),
                    crate::time::day_label(min_start)
                ),
                driver: d,
                start_location,
                end_location,
                min_start,
                min_first_arrival,
                max_end,
                max_soft_end,
                max_last_departure,
                max_soft_last_departure,
                capacity,
                driving_rest,
                bench: false,
            });
        }
    }
    let bench = shift_facts.len();
    shift_facts.push(ShiftFact {
        id: "unplanned".into(),
        label: "Unplanned".into(),
        capacity: vec![0; capacity_types.len()],
        bench: true,
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

    // Jobs and their stops.
    let mut job_facts = Vec::new();
    let mut visit_facts = Vec::new();
    let mut statics: Vec<Visit> = Vec::new();
    let job_index: HashMap<&str, usize> = plan
        .jobs
        .iter()
        .enumerate()
        .map(|(i, j)| (j.id.as_str(), i))
        .collect();
    let driver_index: HashMap<&str, usize> = plan
        .drivers
        .iter()
        .enumerate()
        .map(|(i, d)| (d.id.as_str(), i))
        .collect();
    for (j, job) in plan.jobs.iter().enumerate() {
        claim(issues, "job", &job.id);
        let what = format!("job {:?}", job.id);
        if job.stops.is_empty() {
            issues.push(Issue::error(
                "job_without_stops",
                format!("{what} has no stops"),
            ));
        }
        let name = job
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
        if !job.demands.is_empty() && job.stops.len() != 2 {
            issues.push(Issue::error(
                "job_demand_needs_two_stops",
                format!("{what} sets job demands but has {} stops", job.stops.len()),
            ));
        }
        let mut prohibited_jobs = Vec::new();
        for other in &job.prohibited_jobs {
            match job_index.get(other.as_str()) {
                Some(&o) => prohibited_jobs.push(o),
                None => issues.push(Issue::warning(
                    "unknown_prohibited_job",
                    format!("{what} prohibits unknown job {other:?}"),
                )),
            }
        }
        for (list, kind) in [
            (&job.required_drivers, "required"),
            (&job.preferred_drivers, "preferred"),
            (&job.prohibited_drivers, "prohibited"),
        ] {
            for id in list {
                if !driver_index.contains_key(id.as_str()) {
                    issues.push(Issue::warning(
                        "unknown_driver",
                        format!("{what} names unknown {kind} driver {id:?}"),
                    ));
                }
            }
        }
        let mut stops = Vec::new();
        let mut totals = vec![0i64; capacity_types.len()];
        for (seq, stop) in job.stops.iter().enumerate() {
            claim(issues, "stop", &stop.id);
            let swhat = format!("stop {:?}", stop.id);
            if !valid_place(&stop.location) {
                issues.push(Issue::error(
                    "bad_location",
                    format!(
                        "{swhat}: {:?} is not a [latitude, longitude]",
                        stop.location
                    ),
                ));
            }
            if stop.duration < 0 {
                issues.push(Issue::error(
                    "negative_duration",
                    format!("{swhat} has a negative duration"),
                ));
            }
            let mut windows = Vec::new();
            for w in &stop.time_windows {
                let min_start = at(issues, &swhat, &w.min_start_time);
                let max_end = at(issues, &swhat, &w.max_end_time);
                let max_start = at_opt(issues, &swhat, &w.max_start_time);
                if let (Some(min_start), Some(max_end)) = (min_start, max_end) {
                    if max_end < min_start + stop.duration * 60 {
                        issues.push(Issue::warning(
                            "window_too_small",
                            format!("{swhat}: a time window is shorter than the service"),
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
            let mut load = vec![0i64; capacity_types.len()];
            for d in &stop.demands {
                if d.pickup < 0 || d.deliver < 0 {
                    issues.push(Issue::error(
                        "negative_demand",
                        format!("{swhat}: demand {:?} is negative", d.kind),
                    ));
                }
                if let Some(k) = type_of(&d.kind) {
                    load[k] += d.pickup - d.deliver;
                }
            }
            for d in &job.demands {
                if let Some(k) = type_of(&d.kind) {
                    load[k] += if seq == 0 {
                        d.demand
                    } else if seq == 1 {
                        -d.demand
                    } else {
                        0
                    };
                }
            }
            for (k, l) in load.iter().enumerate() {
                totals[k] += l;
            }
            let sla = at_opt(issues, &swhat, &stop.latest_sla_end_time);
            let earliest_day = match (windows.first(), windows.last()) {
                (Some(first), Some(last))
                    if last.max_end.div_euclid(DAY) > first.min_start.div_euclid(DAY) =>
                {
                    Some(first.min_start.div_euclid(DAY))
                }
                _ => None,
            };
            let index = visit_facts.len();
            let mut visit = Visit::new(index);
            visit.job = Some(j);
            visit.previous = (seq > 0).then(|| index - 1);
            visit.duration = stop.duration * 60;
            if mandatory {
                visit.unplanned_medium = weight;
            } else {
                visit.unplanned_soft = weight;
            }
            visit.sla_unplanned = sla
                .filter(|&s| s >= window.0 && s < window.1)
                .map_or(0, |s| window.1 - s);
            statics.push(visit);
            visit_facts.push(VisitFact {
                id: stop.id.clone(),
                label: stop.name.clone().unwrap_or_else(|| stop.id.clone()),
                kind: Kind::Stop { job: j, seq },
                location: Some(places.add(stop.location)),
                duration: stop.duration * 60,
                windows,
                load,
                sla,
                min_start_travel: at_opt(issues, &swhat, &stop.min_start_travel_time),
                earliest_day,
                owner: None,
                pin: None,
            });
            stops.push(index);
        }
        if let Some(k) = totals.iter().position(|&t| t != 0) {
            issues.push(Issue::warning(
                "unbalanced_demand",
                format!(
                    "{what} picks up and delivers different amounts of {:?}",
                    capacity_types[k]
                ),
            ));
        }
        let max_burden = job
            .maximum_time_burden
            .or(plan.parameters.maximum_time_burden);
        if max_burden.is_some_and(|b| b < 0) {
            issues.push(Issue::error(
                "negative_time_burden",
                format!("{what} has a negative maximum time burden"),
            ));
        }
        job_facts.push(JobFact {
            id: job.id.clone(),
            label: job.name.clone().unwrap_or_else(|| job.id.clone()),
            stops,
            prohibited_jobs,
            no_pooling: job.prohibit_job_pooling,
            interruptible: job.allow_interruption == Interruption::Breaks,
            max_burden: max_burden.map(|m| m * 60),
            mandatory,
            weight,
        });
    }

    // Breaks, after every stop.
    for (s, b) in breaks {
        claim(issues, "break", &b.id);
        let what = format!("break {:?}", b.id);
        let fixed = b.kind == BreakKind::Fixed;
        let (start, end, duration) = if fixed {
            let start = b.start_time.as_deref().and_then(|t| at(issues, &what, t));
            let end = b.end_time.as_deref().and_then(|t| at(issues, &what, t));
            match (start, end) {
                (Some(s), Some(e)) if s < e => (s, Some(e), e - s),
                (Some(_), Some(_)) => {
                    issues.push(Issue::error(
                        "bad_break",
                        format!("{what} ends before it starts"),
                    ));
                    (0, None, 0)
                }
                _ => {
                    issues.push(Issue::error(
                        "bad_break",
                        format!("{what}: a fixed break needs startTime and endTime"),
                    ));
                    (0, None, 0)
                }
            }
        } else {
            let start = b
                .min_start_time
                .as_deref()
                .and_then(|t| at(issues, &what, t));
            let end = at_opt(issues, &what, &b.max_end_time);
            match (start, b.duration) {
                (Some(s), Some(d)) if d > 0 => {
                    if end.is_some_and(|e| e < s + d * 60) {
                        issues.push(Issue::warning(
                            "break_window_too_small",
                            format!("{what} is longer than its window"),
                        ));
                    }
                    (s, end, d * 60)
                }
                _ => {
                    issues.push(Issue::error(
                        "bad_break",
                        format!(
                            "{what}: a floating break needs minStartTime and a positive duration"
                        ),
                    ));
                    (0, None, 0)
                }
            }
        };
        let index = visit_facts.len();
        let mut visit = Visit::new(index);
        visit.fixed_break = fixed;
        visit.floating_break = !fixed;
        visit.located = fixed && b.location.is_some();
        visit.duration = duration;
        statics.push(visit);
        if let Some(p) = &b.location {
            if !valid_place(p) {
                issues.push(Issue::error(
                    "bad_location",
                    format!("{what}: {p:?} is not a [latitude, longitude]"),
                ));
            }
        }
        visit_facts.push(VisitFact {
            id: b.id.clone(),
            label: b.id.clone(),
            kind: Kind::Break(BreakFact {
                shift: s,
                fixed,
                start,
                end,
                duration,
                reject_jobs: b.policy == BreakPolicy::RejectJobs,
            }),
            location: b.location.map(|p| places.add(p)),
            duration,
            windows: Vec::new(),
            load: vec![0; capacity_types.len()],
            sla: None,
            min_start_travel: None,
            earliest_day: None,
            owner: Some(s),
            pin: None,
        });
    }

    // How each shift fits each job.
    let mut fits = Vec::with_capacity(plan.jobs.len() * shift_facts.len());
    let shifts_wire: Vec<&crate::wire::DriverShift> =
        plan.drivers.iter().flat_map(|d| &d.shifts).collect();
    for job in &plan.jobs {
        for (s, fact) in shift_facts.iter().enumerate() {
            if fact.bench {
                fits.push(Fit {
                    multiplier: 1.0,
                    ..Fit::default()
                });
                continue;
            }
            let shift = shifts_wire[s];
            let driver = &plan.drivers[fact.driver].id;
            let mut fit = Fit {
                not_required: !job.required_drivers.is_empty()
                    && !job.required_drivers.contains(driver),
                prohibited: job.prohibited_drivers.contains(driver),
                not_preferred: !job.preferred_drivers.is_empty()
                    && !job.preferred_drivers.contains(driver),
                multiplier: 1.0,
                ..Fit::default()
            };
            for need in &job.required_skills {
                match shift.skills.iter().find(|k| k.name == need.name) {
                    None => fit.skills_missing += 1 + need.min_level.unwrap_or(0),
                    Some(have) => {
                        let level = have.level.unwrap_or(1);
                        if let Some(min) = need.min_level {
                            fit.skills_missing += (min - level).max(0);
                            fit.overqualified += (level - min).max(0);
                        }
                        fit.multiplier *= have.multiplier.unwrap_or(1.0);
                    }
                }
            }
            fit.tags_missing = job
                .required_tags
                .iter()
                .filter(|t| !shift.tags.contains(t))
                .count() as i64;
            fits.push(fit);
        }
    }
    let shift_count = shift_facts.len();
    for fact in visit_facts.iter_mut() {
        let (Kind::Stop { job, .. }, Some(_)) = (&fact.kind, fact.earliest_day) else {
            continue;
        };
        let job = *job;
        let windows = &fact.windows;
        let first = (0..shift_count)
            .filter(|&s| {
                let fit = &fits[job * shift_count + s];
                !shift_facts[s].bench
                    && !fit.not_required
                    && !fit.prohibited
                    && fit.skills_missing == 0
                    && fit.tags_missing == 0
            })
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
    for job in &plan.jobs {
        for tag in &job.required_tags {
            if !shifts_wire.iter().any(|s| s.tags.contains(tag)) {
                issues.push(Issue::warning(
                    "tag_not_offered",
                    format!("job {:?} requires tag {tag:?} no shift offers", job.id),
                ));
            }
        }
        for need in &job.required_skills {
            if !shifts_wire
                .iter()
                .any(|s| s.skills.iter().any(|k| k.name == need.name))
            {
                issues.push(Issue::warning(
                    "skill_not_offered",
                    format!(
                        "job {:?} requires skill {:?} no shift offers",
                        job.id, need.name
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
    // Parts: what each shift loads, what each stop uses up.
    let mut part_names = PartNames::default();
    let wire_stops: HashMap<&str, &crate::wire::Stop> = plan
        .jobs
        .iter()
        .flat_map(|j| j.stops.iter())
        .map(|s| (s.id.as_str(), s))
        .collect();
    let mut visit_parts = Vec::with_capacity(visit_facts.len());
    for fact in &visit_facts {
        let needs = match (&fact.kind, wire_stops.get(fact.id.as_str())) {
            (Kind::Stop { .. }, Some(stop)) => {
                part_names.read(&stop.parts, &format!("stop {:?}", stop.id))
            }
            _ => Ok(Vec::new()),
        };
        match needs {
            Ok(needs) => visit_parts.push(needs),
            Err(message) => {
                issues.push(Issue::error("bad_parts", message));
                return None;
            }
        }
    }
    let wire_shifts: HashMap<&str, &crate::wire::DriverShift> = plan
        .drivers
        .iter()
        .flat_map(|d| d.shifts.iter())
        .map(|s| (s.id.as_str(), s))
        .collect();
    let mut loads = Vec::with_capacity(shift_facts.len());
    for fact in &shift_facts {
        let stock = match wire_shifts.get(fact.id.as_str()).filter(|_| !fact.bench) {
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
    let shifts = (0..shift_facts.len())
        .map(|s| Shift::new(s, s == bench))
        .collect();
    Some((
        Facts {
            window,
            travel,
            capacity_types,
            drivers: driver_ids,
            visits: visit_facts,
            jobs: job_facts,
            shifts: shift_facts,
            fits,
            parts: part_names.names,
            visit_parts,
            shift_stock,
        },
        statics,
        shifts,
    ))
}
