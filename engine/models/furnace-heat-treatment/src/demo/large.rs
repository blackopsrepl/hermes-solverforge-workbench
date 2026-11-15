//! A full week of a heat treatment shop: eleven furnaces, 39 operators and
//! 150 work orders, with maintenance, leave and parts arriving mid-week.

use serde_json::{json, Value};

use super::rng::Rng;
use super::shop::{coverage, furnace, operators, orders, recipes, Work, OPERATORS, RECIPES};

const GEARS: &[&str] = &[
    "Planetary ring gear",
    "Bevel pinion",
    "Driven gear",
    "Epicyclic pinion",
];
const SHAFTS: &[&str] = &[
    "Transmission shaft",
    "Gearbox output shaft",
    "Differential flange",
];
const RINGS: &[&str] = &["Outer bearing ring", "Inner bearing ring"];
const CASTINGS: &[&str] = &["Engine crankcase", "Cylinder head", "Gear pump housing"];
const FORGINGS: &[&str] = &[
    "Forged round bar",
    "Forged flange",
    "Forged disc",
    "Forged billet",
];
const RODS: &[&str] = &[
    "Hydraulic cylinder rod",
    "Steering knuckle pin",
    "Transmission input shaft",
];
const WELDS: &[&str] = &[
    "Welded differential housing",
    "Welded axle frame",
    "Welded beam",
];
const ALU: &[&str] = &[
    "Aluminium pump body",
    "Racing brake caliper",
    "Transmission housing",
];
const STEELS: &[&str] = &["42CrMo4", "39NiCrMo3", "18NiCrMo5", "C45"];
const CASE: &[&str] = &["16MnCr5", "20MnCr5", "18NiCrMo5"];
const ALLOYS: &[&str] = &["6061-T6", "7075-T6", "A356-T6"];

pub fn dataset() -> Value {
    let mut rng = Rng::new(2027);
    let w = |recipe, parts, materials, kg| Work {
        recipe,
        parts,
        materials,
        kg,
    };
    let work = [
        (w(0, SHAFTS, STEELS, (100, 900)), 10),
        (w(1, SHAFTS, STEELS, (150, 1100)), 8),
        (w(2, FORGINGS, STEELS, (400, 1400)), 4),
        (w(3, RINGS, &["100Cr6"], (50, 400)), 6),
        (w(25, GEARS, STEELS, (100, 800)), 6),
        (w(4, RINGS, &["100Cr6"], (50, 400)), 6),
        (w(5, RODS, STEELS, (100, 600)), 4),
        (w(6, SHAFTS, STEELS, (150, 1100)), 8),
        (w(7, FORGINGS, STEELS, (300, 1400)), 4),
        (w(26, GEARS, STEELS, (100, 800)), 6),
        (w(8, CASTINGS, &["GG25", "GS-52"], (200, 1600)), 6),
        (w(9, CASTINGS, &["GS-60", "GS-52"], (300, 1800)), 4),
        (w(10, GEARS, CASE, (300, 1500)), 6),
        (w(11, SHAFTS, CASE, (300, 1800)), 5),
        (w(12, GEARS, CASE, (400, 2200)), 3),
        (w(13, RODS, &["31CrMoV9", "Nitralloy 135M"], (300, 2000)), 2),
        (w(14, RODS, &["31CrMoV9", "42CrMo4"], (300, 2000)), 2),
        (w(15, WELDS, &["S355", "P265GH"], (200, 1200)), 7),
        (w(16, WELDS, &["S355"], (600, 1900)), 5),
        (w(17, CASTINGS, &["GS-52"], (150, 900)), 5),
        (w(18, FORGINGS, STEELS, (200, 1500)), 7),
        (w(19, FORGINGS, STEELS, (200, 1500)), 6),
        (w(20, FORGINGS, STEELS, (200, 1200)), 5),
        (w(21, ALU, ALLOYS, (50, 600)), 4),
        (w(22, ALU, ALLOYS, (50, 600)), 3),
        (w(23, ALU, ALLOYS, (50, 600)), 4),
        (w(24, ALU, ALLOYS, (50, 600)), 4),
    ];
    let all_recipes: Vec<usize> = (0..RECIPES.len()).collect();
    let all_operators: Vec<usize> = (0..OPERATORS.len()).collect();
    json!({
        "schedule": {"start": "2027-03-01T00:00", "end": "2027-03-08T00:00"},
        "recipes": recipes(&all_recipes),
        "furnaces": [
            furnace(0, json!([]), Some(840)),
            furnace(1, json!([]), None),
            furnace(2, json!([{"from": "2027-03-03T06:00", "to": "2027-03-03T14:00",
                               "reason": "Burner service"}]), None),
            furnace(3, json!([]), None),
            furnace(4, json!([]), None),
            furnace(5, json!([]), Some(920)),
            furnace(6, json!([]), Some(930)),
            furnace(7, json!([]), Some(520)),
            furnace(8, json!([{"from": "2027-03-04T08:00", "to": "2027-03-04T16:00",
                               "reason": "Thermocouple calibration"}]), None),
            furnace(9, json!([]), None),
            furnace(10, json!([]), None),
        ],
        "operators": operators(&all_operators, &[
            (5, &["2027-03-03", "2027-03-04"]),
            (22, &["2027-03-06", "2027-03-07"]),
        ]),
        "coverage": coverage("2027-03-01", 7, 2),
        "workOrders": orders(&mut rng, "2027-03-01", 7, &work, 20, 6),
    })
}
