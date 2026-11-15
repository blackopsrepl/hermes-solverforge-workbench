//! `LARGE_SIX_WEEKS`: a regional airline's thirty aircraft of four types over
//! six weeks around Easter, with six bays (one at a contractor), six crews,
//! six kinds of part and a readiness rule per type plus an Easter peak. The
//! first week is published from an earlier plan, and work inducted on the
//! first two days is pinned.

use super::build::{bay, change, crew, date, needs, part, plan, rule, span, task, unit};
use super::published::publish;
use super::rng::Rng;
use crate::domain::{Plan, Task};
use crate::wire::Parameters;

const FIRST: &str = "2027-03-01";
const DAYS: i64 = 42;

/// `(fleet, registrations, usage per day, downtime cost, priority)`.
const FLEETS: [(&str, &[&str], f64, i64, i64); 4] = [
    (
        "A320",
        &[
            "EI-DEA", "EI-DEB", "EI-DEC", "EI-DED", "EI-DEE", "EI-DEF", "EI-DEG", "EI-DEH",
            "EI-DEI", "EI-DEJ", "EI-DEK", "EI-DEL",
        ],
        10.0,
        18_000,
        1,
    ),
    (
        "A321",
        &["EI-LRA", "EI-LRB", "EI-LRC", "EI-LRD", "EI-LRE", "EI-LRF"],
        11.0,
        22_000,
        2,
    ),
    (
        "ATR72",
        &[
            "EI-FAA", "EI-FAB", "EI-FAC", "EI-FAD", "EI-FAE", "EI-FAF", "EI-FAG", "EI-FAH",
        ],
        7.0,
        7_000,
        1,
    ),
    (
        "E190",
        &["EI-RMA", "EI-RMB", "EI-RMC", "EI-RMD"],
        8.5,
        12_000,
        1,
    ),
];

const A_CHECK_INTERVAL: f64 = 600.0;
const ANY_HANGAR: [&str; 1] = ["hangar"];
const LINE_OR_HANGAR: [&str; 2] = ["line", "hangar"];

pub fn large_six_weeks() -> Plan {
    let mut rng = Rng::new(2027);
    let mut plan = plan(
        FIRST,
        DAYS,
        Parameters {
            return_to_service_days: 0,
            ..Parameters::default()
        },
    );

    // Aircraft, and each one's hours since its last A-check.
    let mut since_check = Vec::new();
    for (fleet, registrations, per_day, downtime, priority) in FLEETS {
        for registration in registrations {
            let current = rng.between(8_000, 30_000) as f64;
            let mut aircraft = unit(registration, fleet, current, per_day, downtime);
            aircraft.priority = priority;
            plan.units.push(aircraft);
            since_check.push(rng.between(120, 560) as f64);
        }
    }
    let charters = [
        ("EI-DEB", 12, 13, "Charter: Six Nations, Rome"),
        ("EI-DEC", 12, 13, "Charter: Six Nations, Rome"),
        ("EI-LRB", 19, 21, "Charter: pilgrimage, Lourdes"),
        ("EI-RMA", 25, 28, "Wet lease to partner"),
    ];
    for (registration, from, to, label) in charters {
        unit_mut(&mut plan, registration)
            .commitments
            .push(span(FIRST, from, to, label));
    }
    unit_mut(&mut plan, "EI-DEL").available_from = Some(date(FIRST, 10));
    unit_mut(&mut plan, "EI-FAH").available_from = Some(date(FIRST, 7));

    let mut bay_a = bay("H1A", "Hangar 1 · Bay A", "hangar", 1, 1_500);
    bay_a.closures = vec![span(FIRST, 33, 34, "Door actuator overhaul")];
    let bay_b = bay("H1B", "Hangar 1 · Bay B", "hangar", 1, 1_500);
    let mut h2 = bay("H2", "Hangar 2", "hangar", 2, 1_100);
    h2.fleets = vec!["ATR72".into(), "E190".into()];
    h2.closures = vec![span(FIRST, 21, 23, "Roof repair")];
    let paint = bay("PB", "Paint hangar", "paint", 1, 2_000);
    let line = bay("LS", "Line stands", "line", 3, 300);
    let contractor = bay("SNN", "Contractor hangar, Shannon", "hangar", 1, 4_200);
    plan.bays = vec![bay_a, bay_b, h2, paint, line, contractor];

    let easter = |technicians| change(FIRST, 25, 28, technicians, "Easter");
    let mut b1 = crew("b1", "B1 mechanics", 26, 6);
    b1.changes = vec![change(FIRST, 14, 16, 20, "Recurrent training"), easter(16)];
    let mut b2 = crew("b2", "B2 avionics", 10, 3);
    b2.changes = vec![easter(6)];
    let mut structures = crew("st", "Structures", 8, 2);
    structures.changes = vec![easter(4)];
    let mut cabin = crew("cab", "Cabin technicians", 6, 2);
    cabin.changes = vec![easter(3)];
    plan.crews = vec![
        b1,
        b2,
        structures,
        crew("ndt", "NDT inspectors", 3, 1),
        crew("pnt", "Painters", 5, 1),
        cabin,
    ];
    plan.parts = vec![
        part(
            FIRST,
            "brake",
            "A320-family carbon brake set",
            3,
            &[(9, 4), (23, 4)],
        ),
        part(FIRST, "prop", "ATR72 propeller blade set", 1, &[(14, 2)]),
        part(
            FIRST,
            "gear",
            "Main landing gear overhaul kit",
            0,
            &[(11, 1), (29, 1)],
        ),
        part(FIRST, "idg", "Integrated drive generator", 2, &[(19, 1)]),
        part(FIRST, "adsb", "ADS-B Out kit", 2, &[(7, 3), (21, 3)]),
        part(FIRST, "seats", "Cabin seat cover set", 2, &[(17, 2)]),
    ];
    let mut peak = rule("easter", "Easter peak", None, 25, 27);
    peak.from = Some(date(FIRST, 24));
    peak.to = Some(date(FIRST, 35));
    plan.readiness = vec![
        rule("fleet", "Whole fleet", None, 23, 26),
        rule("a320", "A320 schedule", Some("A320"), 8, 10),
        rule("a321", "A321 schedule", Some("A321"), 4, 5),
        rule("atr72", "ATR72 schedule", Some("ATR72"), 5, 6),
        rule("e190", "E190 schedule", Some("E190"), 2, 3),
        peak,
    ];

    plan.tasks = tasks(&plan, &since_check, &mut rng);
    // The first week as published by last week's plan; what was inducted
    // on the first two days is under way and pinned.
    let week_one: Vec<usize> = plan
        .tasks
        .iter()
        .enumerate()
        .filter(|(_, t)| !t.deferrable && due_within(t, &plan, 9))
        .map(|(i, _)| i)
        .collect();
    publish(plan, &week_one, 2)
}

fn unit_mut<'a>(plan: &'a mut Plan, id: &str) -> &'a mut crate::wire::Unit {
    plan.units
        .iter_mut()
        .find(|u| u.id == id)
        .expect("demo unit")
}

/// Whether the task falls due within `days` days of the start.
fn due_within(task: &Task, plan: &Plan, days: i64) -> bool {
    let unit = plan.units.iter().find(|u| u.id == task.unit).expect("unit");
    let by_date = task
        .due_date
        .as_deref()
        .is_some_and(|d| d <= date(FIRST, days).as_str());
    let by_usage = match (task.due_usage, &unit.usage) {
        (Some(limit), Some(usage)) => (limit as f64 - usage.current) / usage.per_day < days as f64,
        _ => false,
    };
    by_date || by_usage
}

fn tasks(plan: &Plan, since_check: &[f64], rng: &mut Rng) -> Vec<Task> {
    let mut out = Vec::new();
    // An A-check for every aircraft, due when it reaches the interval.
    for (n, aircraft) in plan.units.iter().enumerate() {
        let usage = aircraft.usage.as_ref().expect("demo usage");
        let reg = &aircraft.id;
        let mut check = task(
            &format!("{reg}-a"),
            reg,
            "A-check",
            1,
            &LINE_OR_HANGAR,
            &[("b1", 4), ("b2", 2)],
        );
        check.due_usage = Some((usage.current + A_CHECK_INTERVAL - since_check[n]) as i64);
        out.push(check);
    }
    // C-checks with their follow-on work.
    let c_checks: [(&str, i64, i64, &[&str]); 6] = [
        ("EI-DEA", 14, 20, &["paint"]),
        ("EI-DEG", 14, 38, &["gear"]),
        ("EI-LRD", 16, 30, &["paint"]),
        ("EI-FAB", 9, 16, &[]),
        ("EI-FAF", 9, 34, &["gear"]),
        ("EI-RMC", 12, 40, &[]),
    ];
    for (reg, days, due, follow_on) in c_checks {
        let c_id = format!("{reg}-c");
        let mut check = task(
            &c_id,
            reg,
            "C-check",
            days,
            &ANY_HANGAR,
            &[("b1", 7), ("b2", 2), ("st", 2), ("ndt", 1)],
        );
        check.due_date = Some(date(FIRST, due));
        check.return_to_service = Some(1);
        out.push(check);
        for kind in follow_on {
            let mut next = match *kind {
                "paint" => {
                    let mut repaint = task(
                        &format!("{reg}-paint"),
                        reg,
                        "Repaint",
                        5,
                        &["paint"],
                        &[("pnt", 4)],
                    );
                    repaint.deferrable = true;
                    repaint
                }
                _ => needs(
                    task(
                        &format!("{reg}-gear"),
                        reg,
                        "Landing gear overhaul",
                        3,
                        &ANY_HANGAR,
                        &[("b1", 3), ("st", 1)],
                    ),
                    "gear",
                    1,
                ),
            };
            next.after = vec![c_id.clone()];
            next.due_date = Some(date(FIRST, DAYS - 1));
            out.push(next);
        }
    }
    let a320_family: Vec<&str> = plan
        .units
        .iter()
        .filter(|u| u.fleet == "A320" || u.fleet == "A321")
        .map(|u| u.id.as_str())
        .collect();
    let atr: Vec<&str> = plan
        .units
        .iter()
        .filter(|u| u.fleet == "ATR72")
        .map(|u| u.id.as_str())
        .collect();
    for (n, reg) in pick(&a320_family, 6, rng).into_iter().enumerate() {
        let mut brakes = task(
            &format!("{reg}-brakes"),
            reg,
            "Brake change",
            1,
            &LINE_OR_HANGAR,
            &[("b1", 2)],
        );
        brakes.due_date = Some(date(FIRST, 6 + 6 * n as i64));
        out.push(needs(brakes, "brake", 1));
    }
    for (n, reg) in pick(&atr, 3, rng).into_iter().enumerate() {
        let mut prop = task(
            &format!("{reg}-prop"),
            reg,
            "Propeller overhaul",
            2,
            &ANY_HANGAR,
            &[("b1", 3)],
        );
        prop.due_date = Some(date(FIRST, 18 + 8 * n as i64));
        out.push(needs(prop, "prop", 1));
    }
    let all: Vec<&str> = plan.units.iter().map(|u| u.id.as_str()).collect();
    for reg in pick(&all, 6, rng) {
        let usage = plan
            .units
            .iter()
            .find(|u| u.id == reg)
            .and_then(|u| u.usage.as_ref())
            .expect("demo usage");
        let mut bore = task(
            &format!("{reg}-bore"),
            reg,
            "Engine borescope",
            1,
            &LINE_OR_HANGAR,
            &[("ndt", 1), ("b1", 1)],
        );
        bore.due_usage = Some((usage.current + usage.per_day * rng.between(8, 36) as f64) as i64);
        out.push(bore);
    }
    for (n, reg) in pick(&a320_family, 3, rng).into_iter().enumerate() {
        let mut idg = task(
            &format!("{reg}-idg"),
            reg,
            "IDG replacement",
            1,
            &LINE_OR_HANGAR,
            &[("b1", 2), ("b2", 1)],
        );
        idg.due_date = Some(date(FIRST, 12 + 10 * n as i64));
        out.push(needs(idg, "idg", 1));
    }
    for reg in pick(&a320_family, 8, rng) {
        let mut adsb = task(
            &format!("{reg}-adsb"),
            reg,
            "ADS-B Out upgrade",
            3,
            &ANY_HANGAR,
            &[("b2", 3)],
        );
        adsb.deferrable = true;
        out.push(needs(adsb, "adsb", 1));
    }
    for reg in pick(&a320_family, 4, rng) {
        let mut cabin = task(
            &format!("{reg}-cabin"),
            reg,
            "Cabin refurbishment",
            4,
            &ANY_HANGAR,
            &[("cab", 4)],
        );
        cabin.deferrable = true;
        cabin.earliest_start = Some(date(FIRST, 7));
        out.push(needs(cabin, "seats", 1));
    }
    for (n, reg) in pick(&all, 4, rng).into_iter().enumerate() {
        let mut corrosion = task(
            &format!("{reg}-corrosion"),
            reg,
            "Corrosion inspection",
            3,
            &ANY_HANGAR,
            &[("st", 2), ("ndt", 1)],
        );
        corrosion.due_date = Some(date(FIRST, 15 + 7 * n as i64));
        out.push(corrosion);
    }
    out
}

/// `count` distinct entries of `from`, in a seeded order.
fn pick<'a>(from: &[&'a str], count: usize, rng: &mut Rng) -> Vec<&'a str> {
    let mut pool: Vec<&str> = from.to_vec();
    let mut out = Vec::new();
    while out.len() < count && !pool.is_empty() {
        out.push(pool.remove(rng.below(pool.len() as u64) as usize));
    }
    out
}
