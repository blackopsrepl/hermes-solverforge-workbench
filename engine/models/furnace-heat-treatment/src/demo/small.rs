//! A three-day job shop with four furnaces and eleven operators.

use serde_json::{json, Value};

use super::rng::Rng;
use super::shop::{coverage, furnace, operators, orders, recipes, Work};

const GEARS: &[&str] = &["Planetary ring gear", "Bevel pinion", "Driven gear"];
const SHAFTS: &[&str] = &["Transmission shaft", "Differential flange"];
const ALU: &[&str] = &["Aluminium pump body", "Transmission housing"];
const WELDS: &[&str] = &["Welded axle frame", "Welded engine base"];
const STEELS: &[&str] = &["42CrMo4", "39NiCrMo3", "18NiCrMo5"];
const ALLOYS: &[&str] = &["6061-T6", "A356-T6"];

pub fn dataset() -> Value {
    let mut rng = Rng::new(11);
    let work = [
        (
            Work {
                recipe: 0,
                parts: GEARS,
                materials: STEELS,
                kg: (150, 450),
            },
            3,
        ),
        (
            Work {
                recipe: 1,
                parts: SHAFTS,
                materials: STEELS,
                kg: (200, 700),
            },
            2,
        ),
        (
            Work {
                recipe: 6,
                parts: SHAFTS,
                materials: STEELS,
                kg: (200, 600),
            },
            2,
        ),
        (
            Work {
                recipe: 4,
                parts: GEARS,
                materials: STEELS,
                kg: (100, 300),
            },
            1,
        ),
        (
            Work {
                recipe: 10,
                parts: GEARS,
                materials: &["16MnCr5", "20MnCr5"],
                kg: (400, 1200),
            },
            2,
        ),
        (
            Work {
                recipe: 21,
                parts: ALU,
                materials: ALLOYS,
                kg: (80, 300),
            },
            2,
        ),
        (
            Work {
                recipe: 23,
                parts: ALU,
                materials: ALLOYS,
                kg: (80, 300),
            },
            1,
        ),
        (
            Work {
                recipe: 15,
                parts: WELDS,
                materials: &["S355"],
                kg: (300, 800),
            },
            1,
        ),
        (
            Work {
                recipe: 18,
                parts: &["Forged round bar"],
                materials: &["C45"],
                kg: (200, 600),
            },
            2,
        ),
    ];
    json!({
        "schedule": {"start": "2027-03-01T00:00", "end": "2027-03-04T00:00"},
        "recipes": recipes(&[0, 1, 4, 6, 10, 15, 18, 21, 23]),
        "furnaces": [
            furnace(0, json!([]), None),
            furnace(3, json!([{"from": "2027-03-02T06:00", "to": "2027-03-02T12:00",
                               "reason": "Door seal replacement"}]), None),
            furnace(5, json!([]), Some(920)),
            furnace(10, json!([]), None),
        ],
        "operators": operators(&[0, 1, 2, 12, 3, 4, 5, 13, 17, 6, 10], &[(4, &["2027-03-02"])]),
        "coverage": coverage("2027-03-01", 3, 1),
        "workOrders": orders(&mut rng, "2027-03-01", 3, &work, 8, 5),
    })
}
