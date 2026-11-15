//! The heat treatment shop the demos share: furnaces, recipes, crews and an
//! order generator.

use serde_json::{json, Value};

use super::rng::Rng;
use crate::time::{format_date, format_date_time, parse_date, DAY};

/// `(id, name, kind, max °C, max kg, processes, heat, cool, kW)`.
type FurnaceSpec = (
    &'static str,
    &'static str,
    &'static str,
    i64,
    i64,
    &'static [&'static str],
    i64,
    i64,
    i64,
);

const CHAMBER: &[&str] = &[
    "quenching",
    "tempering",
    "annealing",
    "normalizing",
    "stressRelieving",
];

pub const FURNACES: [FurnaceSpec; 11] = [
    (
        "CH1",
        "Chamber furnace 1",
        "chamber",
        1050,
        1600,
        CHAMBER,
        10,
        4,
        180,
    ),
    (
        "CH2",
        "Chamber furnace 2",
        "chamber",
        1050,
        1200,
        CHAMBER,
        9,
        4,
        150,
    ),
    (
        "CH3",
        "Chamber furnace 3",
        "chamber",
        1100,
        1800,
        CHAMBER,
        11,
        5,
        210,
    ),
    (
        "CH4",
        "Chamber furnace 4",
        "chamber",
        1050,
        1000,
        CHAMBER,
        8,
        3,
        130,
    ),
    (
        "CH5",
        "Chamber furnace 5",
        "chamber",
        1050,
        900,
        CHAMBER,
        8,
        3,
        120,
    ),
    (
        "CP1",
        "Carburizing pit 1",
        "pit",
        950,
        3000,
        &["carburizing", "quenching"],
        6,
        2,
        260,
    ),
    (
        "CP2",
        "Carburizing pit 2",
        "pit",
        960,
        2600,
        &["carburizing", "quenching"],
        6,
        2,
        240,
    ),
    (
        "NP1",
        "Nitriding pit 1",
        "pit",
        580,
        3000,
        &["nitriding"],
        5,
        2,
        150,
    ),
    (
        "NP2",
        "Nitriding pit 2",
        "pit",
        580,
        2400,
        &["nitriding"],
        5,
        2,
        140,
    ),
    (
        "CHF",
        "Car hearth furnace",
        "carHearth",
        1050,
        5000,
        CHAMBER,
        5,
        2,
        400,
    ),
    (
        "ALU",
        "Aluminium furnace",
        "aluminum",
        600,
        1000,
        &["solutionTreating", "aging"],
        10,
        5,
        110,
    ),
];

/// A furnace record by its spec index, with optional downtime and start temperature.
pub fn furnace(index: usize, downtime: Value, initial: Option<i64>) -> Value {
    let (id, name, kind, max_t, max_kg, processes, heat, cool, kw) = FURNACES[index];
    let mut record = json!({
        "id": id, "name": name, "kind": kind, "maxTemperature": max_t, "maxLoadKg": max_kg,
        "processes": processes, "heatingRate": heat, "coolingRate": cool, "powerKw": kw,
    });
    if downtime.as_array().is_some_and(|d| !d.is_empty()) {
        record["downtime"] = downtime;
    }
    if let Some(t) = initial {
        record["initialTemperature"] = json!(t);
    }
    record
}

/// `(id, name, process, °C, cycle minutes)`.
pub const RECIPES: [(&str, &str, &str, i64, i64); 27] = [
    ("HRD-840-60", "Harden 42CrMo4", "quenching", 840, 60),
    ("HRD-860-90", "Harden 39NiCrMo3", "quenching", 860, 90),
    ("HRD-880-150", "Harden heavy section", "quenching", 880, 150),
    ("HRD-830-45", "Harden 100Cr6 rings", "quenching", 830, 45),
    ("TMP-180-120", "Low temper bearings", "tempering", 180, 120),
    ("TMP-450-90", "Temper springs", "tempering", 450, 90),
    ("TMP-600-180", "Temper shafts", "tempering", 600, 180),
    ("TMP-650-240", "High temper forgings", "tempering", 650, 240),
    ("ANN-780-300", "Soft anneal castings", "annealing", 780, 300),
    ("ANN-860-480", "Full anneal GS-52", "annealing", 860, 480),
    ("CRB-920-480", "Carburize 0.8 mm", "carburizing", 920, 480),
    ("CRB-930-720", "Carburize 1.2 mm", "carburizing", 930, 720),
    ("CRB-940-900", "Carburize 1.6 mm", "carburizing", 940, 900),
    ("NIT-520-1440", "Gas nitride 24 h", "nitriding", 520, 1440),
    ("NIT-540-2160", "Gas nitride 36 h", "nitriding", 540, 2160),
    (
        "SR-580-180",
        "Stress relieve welds",
        "stressRelieving",
        580,
        180,
    ),
    (
        "SR-620-240",
        "Stress relieve frames",
        "stressRelieving",
        620,
        240,
    ),
    (
        "SR-650-120",
        "Stress relieve housings",
        "stressRelieving",
        650,
        120,
    ),
    ("NRM-880-120", "Normalize bars", "normalizing", 880, 120),
    ("NRM-900-180", "Normalize flanges", "normalizing", 900, 180),
    ("NRM-920-90", "Normalize discs", "normalizing", 920, 90),
    (
        "SOL-530-180",
        "Solution treat 6061",
        "solutionTreating",
        530,
        180,
    ),
    (
        "SOL-480-240",
        "Solution treat 7075",
        "solutionTreating",
        480,
        240,
    ),
    ("AGE-175-480", "Age 6061-T6", "aging", 175, 480),
    ("AGE-120-720", "Age 7075-T6", "aging", 120, 720),
    ("HRD-850-120", "Harden gears", "quenching", 850, 120),
    ("TMP-200-150", "Temper gears", "tempering", 200, 150),
];

pub fn recipes(indexes: &[usize]) -> Value {
    Value::Array(
        indexes
            .iter()
            .map(|&i| {
                let (id, name, process, t, cycle) = RECIPES[i];
                json!({"id": id, "name": name, "process": process, "temperature": t, "cycleMinutes": cycle})
            })
            .collect(),
    )
}

/// `(id, name, role, day only, extra skills)`.
pub const OPERATORS: [(&str, &str, &str, bool, &[&str]); 39] = [
    ("op-01", "Marco Bellini", "shiftLead", false, &[]),
    ("op-02", "Fabio Rinaldi", "shiftLead", false, &[]),
    (
        "op-03",
        "Davide Greco",
        "shiftLead",
        false,
        &["craneForklift"],
    ),
    ("op-04", "Luca Mariani", "furnaceOperator", false, &[]),
    ("op-05", "Stefano Moretti", "furnaceOperator", false, &[]),
    (
        "op-06",
        "Paolo Conti",
        "furnaceOperator",
        false,
        &["furnaceProgram"],
    ),
    ("op-07", "Andrea Bruno", "maintenanceTechnician", false, &[]),
    (
        "op-08",
        "Giorgio Esposito",
        "maintenanceTechnician",
        false,
        &["loadBuild"],
    ),
    (
        "op-09",
        "Matteo Ferri",
        "maintenanceTechnician",
        false,
        &["craneForklift"],
    ),
    ("op-10", "Simone Neri", "materialHandler", true, &[]),
    (
        "op-11",
        "Riccardo Sala",
        "materialHandler",
        true,
        &["loadBuild"],
    ),
    (
        "op-12",
        "Elena Villa",
        "qualityControl",
        true,
        &["shipStore"],
    ),
    (
        "op-13",
        "Michele Riva",
        "shiftLead",
        false,
        &["craneForklift"],
    ),
    (
        "op-14",
        "Giulio De Luca",
        "furnaceOperator",
        false,
        &["furnaceProgram"],
    ),
    (
        "op-15",
        "Roberto Leone",
        "maintenanceTechnician",
        false,
        &["loadBuild", "quenchOperate"],
    ),
    ("op-16", "Alessio Guerra", "shiftLead", false, &[]),
    (
        "op-17",
        "Tommaso Villa",
        "shiftLead",
        false,
        &["craneForklift"],
    ),
    (
        "op-18",
        "Sergio Bassi",
        "furnaceOperator",
        false,
        &["furnaceProgram"],
    ),
    ("op-19", "Ivan Serra", "furnaceOperator", false, &[]),
    (
        "op-20",
        "Enrico Nardi",
        "maintenanceTechnician",
        false,
        &["loadBuild"],
    ),
    (
        "op-21",
        "Dario Bellotti",
        "maintenanceTechnician",
        false,
        &["craneForklift"],
    ),
    (
        "op-22",
        "Gabriele Rossi",
        "shiftLead",
        false,
        &["craneForklift"],
    ),
    ("op-23", "Nicola Santini", "shiftLead", false, &[]),
    (
        "op-24",
        "Massimo Orsi",
        "furnaceOperator",
        false,
        &["furnaceProgram"],
    ),
    (
        "op-25",
        "Fabiano Grechi",
        "maintenanceTechnician",
        false,
        &["loadBuild"],
    ),
    (
        "op-26",
        "Leonardo Piva",
        "maintenanceTechnician",
        false,
        &["craneForklift"],
    ),
    (
        "op-27",
        "Federico Galli",
        "shiftLead",
        false,
        &["craneForklift"],
    ),
    (
        "op-28",
        "Samuele Vannini",
        "furnaceOperator",
        false,
        &["furnaceProgram"],
    ),
    (
        "op-29",
        "Matteo Alberti",
        "furnaceOperator",
        false,
        &["furnaceProgram"],
    ),
    (
        "op-30",
        "Claudio Martini",
        "shiftLead",
        false,
        &["craneForklift"],
    ),
    ("op-31", "Ettore Romano", "shiftLead", false, &[]),
    (
        "op-32",
        "Francesco Serra",
        "furnaceOperator",
        false,
        &["furnaceProgram"],
    ),
    (
        "op-33",
        "Alberto Costa",
        "furnaceOperator",
        false,
        &["furnaceProgram"],
    ),
    (
        "op-34",
        "Mauro Benedetti",
        "furnaceOperator",
        false,
        &["furnaceProgram"],
    ),
    (
        "op-35",
        "Valerio Fontana",
        "shiftLead",
        false,
        &["craneForklift"],
    ),
    (
        "op-36",
        "Marta Fabbri",
        "furnaceOperator",
        false,
        &["furnaceProgram"],
    ),
    (
        "op-37",
        "Pietro Ricci",
        "furnaceOperator",
        false,
        &["furnaceProgram"],
    ),
    (
        "op-38",
        "Andrea Bianchi",
        "shiftLead",
        false,
        &["craneForklift"],
    ),
    (
        "op-39",
        "Silvia Rota",
        "furnaceOperator",
        false,
        &["furnaceProgram"],
    ),
];

/// Operator records; `leave` gives `(operator index, dates)` of unavailability.
pub fn operators(indexes: &[usize], leave: &[(usize, &[&str])]) -> Value {
    Value::Array(
        indexes
            .iter()
            .map(|&i| {
                let (id, name, role, day_only, skills) = OPERATORS[i];
                let mut record = json!({"id": id, "name": name, "role": role});
                if day_only {
                    record["dayOnly"] = json!(true);
                }
                if !skills.is_empty() {
                    record["skills"] = json!(skills);
                }
                if let Some((_, dates)) = leave.iter().find(|(op, _)| *op == i) {
                    record["unavailable"] = json!(dates);
                }
                record
            })
            .collect(),
    )
}

/// One shift lead per shift and `day_crew` furnace operators on day shifts,
/// one on nights, for the carry-in night and every day from `first`.
pub fn coverage(first: &str, days: i64, day_crew: i64) -> Value {
    let start = parse_date(first).expect("demo date");
    let mut out = vec![
        json!({"day": format_date(start - 1), "shift": "night", "role": "shiftLead", "count": 1}),
        json!({"day": format_date(start - 1), "shift": "night", "role": "furnaceOperator", "count": 1}),
    ];
    for day in 0..days {
        let date = format_date(start + day);
        for shift in ["morning", "afternoon", "night"] {
            out.push(json!({"day": date, "shift": shift, "role": "shiftLead", "count": 1}));
            let crew = if shift == "night" { 1 } else { day_crew };
            out.push(
                json!({"day": date, "shift": shift, "role": "furnaceOperator", "count": crew}),
            );
        }
    }
    Value::Array(out)
}

const CUSTOMERS: [&str; 10] = [
    "Brembo",
    "Dana",
    "SKF",
    "Interpump",
    "Carraro",
    "Bondioli & Pavesi",
    "Comer Industries",
    "Lombardini",
    "Bonfiglioli",
    "Dana Graziano",
];

/// `(recipe index, parts, materials, kg range)` the order generator draws from.
pub struct Work {
    pub recipe: usize,
    pub parts: &'static [&'static str],
    pub materials: &'static [&'static str],
    pub kg: (i64, i64),
}

/// Generates `count` orders for `work`, due within `days` days of `first`.
/// Every `express_every`-th order is express and the next two urgent; every
/// `late_release_every`-th order arrives during the week.
pub fn orders(
    rng: &mut Rng,
    first: &str,
    days: i64,
    work: &[(Work, usize)],
    express_every: usize,
    late_release_every: usize,
) -> Value {
    let origin = parse_date(first).expect("demo date") * DAY;
    let mut out = Vec::new();
    let mut seq = 1400;
    for (spec, count) in work {
        let (_, _, _, _, cycle) = RECIPES[spec.recipe];
        for _ in 0..*count {
            let n = out.len();
            let kg = spec.kg.0 + rng.below((spec.kg.1 - spec.kg.0 + 1) as u64) as i64;
            // Due at 14:00 or 22:00 on a day of the schedule, plus some slack.
            let due_day = 1 + rng.below((days - 1).max(1) as u64) as i64;
            let due_hour = if rng.below(2) == 0 { 14 * 60 } else { 22 * 60 };
            let slack = 120 * rng.below(3) as i64;
            let mut due = (due_day * DAY + due_hour + slack).min(days * DAY - 60);
            let priority = match n % express_every {
                0 => "express",
                1 | 2 => "urgent",
                _ => "standard",
            };
            let mut record = json!({
                "id": format!("WO-{seq}"),
                "customer": CUSTOMERS[rng.below(CUSTOMERS.len() as u64) as usize],
                "part": spec.parts[rng.below(spec.parts.len() as u64) as usize],
                "material": spec.materials[rng.below(spec.materials.len() as u64) as usize],
                "recipe": RECIPES[spec.recipe].0,
                "loadKg": kg,
                "priority": priority,
            });
            let release = (DAY / 2 + rng.below((days * DAY / 2) as u64) as i64) / 60 * 60;
            let arrives_late = late_release_every > 0
                && n % late_release_every == late_release_every - 1
                && release + cycle + 8 * 60 <= days * DAY - 60;
            if arrives_late {
                record["release"] = json!(format_date_time(origin + release));
                due = due.max(release + cycle + 8 * 60);
            }
            record["due"] = json!(format_date_time(origin + due));
            out.push(record);
            seq += 1;
        }
    }
    Value::Array(out)
}
