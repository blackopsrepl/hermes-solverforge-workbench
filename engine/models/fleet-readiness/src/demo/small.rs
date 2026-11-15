//! `SMALL_FORTNIGHT`: a regional operator's six aircraft over two weeks, two
//! hangars and a line stand. Small enough to read on one screen, with every
//! kind of record: usage-limited checks, a C-check followed by a gear
//! inspection, parts that arrive mid-fortnight, a charter, an aircraft back
//! from lease, a closed hangar, a crew on a course and deferrable upgrades.

use super::build::{bay, change, crew, date, needs, part, plan, rule, span, task, unit};
use crate::domain::Plan;
use crate::wire::Parameters;

const FIRST: &str = "2027-03-01";

pub fn small_fortnight() -> Plan {
    let mut plan = plan(
        FIRST,
        14,
        Parameters {
            return_to_service_days: 0,
            ..Parameters::default()
        },
    );

    let mut fna = unit("EI-FNA", "A320", 11_840.0, 10.0, 18_000);
    fna.priority = 2;
    let mut fnb = unit("EI-FNB", "A320", 9_410.0, 10.5, 18_000);
    fnb.priority = 2;
    fnb.commitments = vec![span(FIRST, 5, 6, "Charter: Six Nations, Rome")];
    let fnc = unit("EI-FNC", "A320", 12_205.0, 9.5, 18_000);
    let fat = unit("EI-FAT", "ATR72", 21_300.0, 7.0, 7_000);
    let fau = unit("EI-FAU", "ATR72", 18_950.0, 7.5, 7_000);
    let mut fav = unit("EI-FAV", "ATR72", 20_110.0, 6.5, 7_000);
    fav.available_from = Some(date(FIRST, 2));
    plan.units = vec![fna, fnb, fnc, fat, fau, fav];

    let h1 = bay("H1", "Hangar 1", "hangar", 1, 1_200);
    let mut h2 = bay("H2", "Hangar 2", "hangar", 1, 900);
    h2.fleets = vec!["ATR72".into()];
    h2.closures = vec![span(FIRST, 10, 11, "Fire suppression test")];
    let line = bay("L1", "Line stands", "line", 2, 300);
    plan.bays = vec![h1, h2, line];

    let mut b1 = crew("b1", "B1 mechanics", 8, 2);
    b1.changes = vec![change(FIRST, 3, 4, 5, "Type course")];
    plan.crews = vec![
        b1,
        crew("b2", "B2 avionics", 3, 1),
        crew("st", "Structures", 3, 1),
        crew("ndt", "NDT inspectors", 1, 1),
    ];
    plan.parts = vec![
        part(FIRST, "brake-a320", "A320 carbon brake set", 1, &[(4, 2)]),
        part(FIRST, "prop-atr", "ATR72 propeller blade set", 0, &[(7, 1)]),
    ];
    plan.readiness = vec![
        rule("fleet", "Whole fleet", None, 4, 5),
        rule("a320", "A320 schedule", Some("A320"), 2, 3),
        rule("atr72", "ATR72 schedule", Some("ATR72"), 1, 2),
    ];

    let line_or_hangar = ["line", "hangar"];
    let mut tasks = Vec::new();
    let mut fna_a = task(
        "fna-a",
        "EI-FNA",
        "A-check",
        1,
        &line_or_hangar,
        &[("b1", 3), ("b2", 1)],
    );
    fna_a.due_usage = Some(11_890);
    tasks.push(fna_a);
    let mut brakes = task(
        "fnb-brakes",
        "EI-FNB",
        "Brake change",
        1,
        &line_or_hangar,
        &[("b1", 2)],
    );
    brakes.due_date = Some(date(FIRST, 8));
    tasks.push(needs(brakes, "brake-a320", 1));
    let mut fnc_a = task(
        "fnc-a",
        "EI-FNC",
        "A-check",
        1,
        &line_or_hangar,
        &[("b1", 3), ("b2", 1)],
    );
    fnc_a.due_usage = Some(12_300);
    tasks.push(fnc_a);
    let mut bore = task(
        "fnc-bore",
        "EI-FNC",
        "Engine borescope",
        1,
        &line_or_hangar,
        &[("ndt", 1), ("b1", 1)],
    );
    bore.due_usage = Some(12_290);
    tasks.push(bore);
    let mut c_check = task(
        "fat-c",
        "EI-FAT",
        "C-check",
        7,
        &["hangar"],
        &[("b1", 4), ("b2", 1), ("st", 2), ("ndt", 1)],
    );
    c_check.due_date = Some(date(FIRST, 11));
    c_check.return_to_service = Some(1);
    tasks.push(c_check);
    let mut gear = task(
        "fat-gear",
        "EI-FAT",
        "Landing gear inspection",
        1,
        &["hangar"],
        &[("st", 1), ("b1", 1)],
    );
    gear.after = vec!["fat-c".into()];
    gear.due_date = Some(date(FIRST, 13));
    tasks.push(gear);
    let mut prop = task(
        "fau-prop",
        "EI-FAU",
        "Propeller overhaul",
        2,
        &["hangar"],
        &[("b1", 2)],
    );
    prop.due_date = Some(date(FIRST, 12));
    tasks.push(needs(prop, "prop-atr", 1));
    let mut fav_a = task(
        "fav-a",
        "EI-FAV",
        "A-check",
        1,
        &line_or_hangar,
        &[("b1", 3), ("b2", 1)],
    );
    fav_a.due_usage = Some(20_160);
    tasks.push(fav_a);
    let mut cabin = task(
        "fna-cabin",
        "EI-FNA",
        "Cabin refurbishment",
        3,
        &["hangar"],
        &[("st", 1), ("b1", 1)],
    );
    cabin.deferrable = true;
    tasks.push(cabin);
    let mut adsb = task(
        "fnb-adsb",
        "EI-FNB",
        "ADS-B Out upgrade",
        2,
        &["hangar"],
        &[("b2", 2)],
    );
    adsb.deferrable = true;
    tasks.push(adsb);
    plan.tasks = tasks;
    plan
}
