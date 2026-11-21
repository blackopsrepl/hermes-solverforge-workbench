//! FACTORY: a sheet-metal and machining plant over one working week. Sixteen
//! machines in two shifts and eight people on day shift run seventy-six orders
//! of up to seven operations each: cut, bend or turn, mill (sometimes in two
//! setups on one machine), weld, paint and inspect. Each coat uses a box of
//! its colour's powder from stock.

use super::build::{
    after, at, holds, holds_between, off, on, outside_hours, placed, task, Builder,
};
use super::rng::Rng;
use crate::domain::Plan;
use crate::wire::StockArrival;
use solverforge_workbench_host::stock::Part;

const ORDERS: usize = 76;
const DAYS: i64 = 5;

const MATERIALS: [&str; 3] = ["aluminium", "steel", "stainless"];
const COLOURS: [&str; 4] = [
    "RAL 9005 black",
    "RAL 7035 grey",
    "RAL 5010 blue",
    "RAL 3020 red",
];
const PARTS: [&str; 10] = [
    "Control cabinet",
    "Machine guard",
    "Conveyor frame",
    "Pump skid",
    "Battery tray",
    "Hydraulic block",
    "Motor bracket",
    "Panel door",
    "Axle hub",
    "Cable duct",
];
const CUSTOMERS: [&str; 7] = [
    "Vandermolen",
    "Kessler AG",
    "Brightline",
    "Norrland",
    "Oostwerk",
    "Tecnomar",
    "Halden",
];

fn machines(b: &mut Builder) {
    let shifts = || outside_hours(DAYS, 6, 22, "No shift");
    for (id, name, initial) in [
        ("LASER-1", "Fibre laser 1", "steel"),
        ("LASER-2", "Fibre laser 2", "aluminium"),
        ("LASER-3", "Fibre laser 3 (12 kW)", "steel"),
    ] {
        let m = b.machine(id, name, "Laser", at(0, 6, 0));
        m.initial_task_type = Some(initial.into());
        m.unavailable = shifts();
        if id == "LASER-3" {
            m.tags = vec![holds("Thick plate")];
        }
    }
    for (id, name) in [
        ("PRESS-1", "Press brake 135 t"),
        ("PRESS-2", "Press brake 320 t"),
    ] {
        b.machine(id, name, "Press brake", at(0, 6, 0)).unavailable = shifts();
    }
    for (id, name) in [
        ("MILL-1", "Vertical mill 1"),
        ("MILL-2", "Vertical mill 2"),
        ("MILL-3", "5-axis mill 3"),
        ("MILL-4", "5-axis mill 4 (new)"),
    ] {
        let m = b.machine(id, name, "Mill", at(0, 6, 0));
        m.initial_task_type = Some("aluminium".into());
        m.unavailable = shifts();
        if id == "MILL-3" || id == "MILL-4" {
            m.tags = vec![holds("5-axis")];
        }
        if id == "MILL-2" {
            m.unavailable
                .push(off(at(2, 6, 0), at(2, 14, 0), "Ball screw replacement"));
        }
    }
    for (id, name) in [
        ("LATHE-1", "CNC lathe 1 (old)"),
        ("LATHE-2", "CNC lathe 2"),
        ("LATHE-3", "CNC lathe 3 (bar feeder)"),
    ] {
        let m = b.machine(id, name, "Lathe", at(0, 6, 0));
        m.initial_task_type = Some("steel".into());
        m.unavailable = shifts();
    }
    for (id, name, colour) in [
        ("PAINT-1", "Powder line 1", COLOURS[1]),
        ("PAINT-2", "Powder line 2", COLOURS[0]),
    ] {
        let m = b.machine(id, name, "Paint line", at(0, 6, 0));
        m.initial_task_type = Some(colour.into());
        m.unavailable = shifts();
        if id == "PAINT-2" {
            m.unavailable
                .push(off(at(3, 14, 0), at(3, 22, 0), "Oven cleaning"));
        }
    }
    for (id, name) in [("WELD-1", "Welding robot 1"), ("WELD-2", "Welding robot 2")] {
        let m = b.machine(id, name, "Weld cell", at(0, 6, 0));
        m.unavailable = shifts();
    }
}

fn people(b: &mut Builder) {
    let day_shift = |b: &mut Builder, id: &str, name: &str, tags: Vec<_>| {
        let e = b.employee(id, name, at(0, 7, 0));
        e.tags = tags;
        e.unavailable = outside_hours(DAYS, 7, 16, "Off shift");
        for day in 0..DAYS {
            e.unavailable
                .push(off(at(day, 11, 30), at(day, 12, 0), "Lunch"));
        }
    };
    day_shift(
        b,
        "W-PIETER",
        "Pieter de Vries (welder)",
        vec![holds("Welding cert")],
    );
    day_shift(
        b,
        "W-SANNE",
        "Sanne Bakker (welder)",
        vec![holds("Welding cert")],
    );
    // Joris's coded welding certificate lapses on Wednesday evening.
    day_shift(
        b,
        "W-JORIS",
        "Joris Smit (welder)",
        vec![holds_between("Welding cert", at(0, 0, 0), at(2, 18, 0))],
    );
    day_shift(
        b,
        "A-FATIMA",
        "Fatima El Amrani (assembler)",
        vec![holds("Assembly")],
    );
    day_shift(
        b,
        "A-LUCAS",
        "Lucas Janssens (assembler)",
        vec![holds("Assembly")],
    );
    day_shift(
        b,
        "A-EMMA",
        "Emma Peeters (assembler)",
        vec![holds("Assembly")],
    );
    day_shift(b, "Q-NOOR", "Noor Hendriks (inspector)", vec![holds("QA")]);
    day_shift(b, "Q-DAAN", "Daan Mertens (inspector)", vec![holds("QA")]);
}

fn changeovers(b: &mut Builder) {
    for (from, to, laser, mill, lathe) in [
        ("aluminium", "steel", 15, 30, 20),
        ("steel", "aluminium", 20, 45, 25),
        ("steel", "stainless", 15, 35, 20),
        ("stainless", "steel", 10, 30, 15),
        ("aluminium", "stainless", 20, 40, 25),
        ("stainless", "aluminium", 20, 45, 25),
    ] {
        b.changeover("Laser", from, to, laser);
        b.changeover("Mill", from, to, mill);
        b.changeover("Lathe", from, to, lathe);
    }
    for material in MATERIALS {
        b.changeover("Mill", material, material, 5);
    }
    // Powder colour changes: towards darker is a quick purge, towards
    // lighter a full booth clean.
    for (i, from) in COLOURS.iter().enumerate() {
        for (j, to) in COLOURS.iter().enumerate() {
            if i != j {
                let minutes = if j == 0 {
                    20
                } else if j > i {
                    45
                } else {
                    30
                };
                b.changeover("Paint line", from, to, minutes);
            }
        }
    }
}

pub fn factory() -> Plan {
    let mut b = Builder::new("Factory · one week");
    for tag in ["Thick plate", "5-axis", "Welding cert", "Assembly", "QA"] {
        b.tag(tag);
    }
    for kind in MATERIALS.iter().chain(COLOURS.iter()) {
        b.task_type(kind);
    }
    for kind in [
        "Laser",
        "Press brake",
        "Mill",
        "Lathe",
        "Paint line",
        "Weld cell",
    ] {
        b.machine_type(kind);
    }
    changeovers(&mut b);
    machines(&mut b);
    people(&mut b);
    // Lasers 1 and 2 feed the brakes next to them over a roller table.
    b.hand_over("LASER-1", "PRESS-1", Some(30));
    b.hand_over("LASER-2", "PRESS-2", Some(30));
    // 12 kW parts are too thick for the 135 t brake.
    b.hand_over("LASER-3", "PRESS-1", None);
    // Robot 2 has no fume extraction link to line 1's pre-treatment.
    b.hand_over("WELD-2", "PAINT-1", None);

    let mut rng = Rng::new(20_270_301);
    let mut pick = |n: usize| rng.below(n as u64) as usize;
    for o in 0..ORDERS {
        let id = format!("O{:03}", o + 1);
        let part = PARTS[pick(PARTS.len())];
        let customer = CUSTOMERS[pick(CUSTOMERS.len())];
        let material = MATERIALS[pick(MATERIALS.len())];
        let colour = COLOURS[pick(COLOURS.len())];
        let thick = pick(5) == 0;
        let turned = matches!(part, "Axle hub" | "Hydraulic block") || pick(6) == 0;
        let two_setups = pick(5) == 0;
        let five_axis = pick(4) == 0;
        let welded = !turned && pick(3) != 0;
        let painted = pick(5) != 0;
        let rush = o % 13 == 5;
        let priority = if rush {
            10
        } else if o % 4 == 0 {
            3
        } else {
            1
        };
        let label = |what: &str| format!("{what} · {part} for {customer}");
        let mut previous: Option<String> = None;
        let mut step = |b: &mut Builder, suffix: &str, mut t: crate::domain::Task| -> String {
            let tid = format!("{id}-{suffix}");
            t.id = tid.clone();
            if let Some(before) = &previous {
                if t.dependencies.is_empty() {
                    t.dependencies = vec![after(before)];
                }
            }
            b.add(t);
            previous = Some(tid.clone());
            tid
        };

        // 1. Cut on a laser, or turn bar on a lathe.
        let mut t = task("", &label(if turned { "Turn" } else { "Laser cut" }), 0);
        t.task_type = Some(material.into());
        if turned {
            t.duration = Some(40 + 10 * pick(6) as i64);
            t.required_resources = vec!["LATHE-1".into(), "LATHE-2".into(), "LATHE-3".into()];
            t.duration_on = vec![on("LATHE-1", t.duration.unwrap() * 5 / 4)];
            if part == "Axle hub" {
                t.required_resources = vec!["LATHE-3".into()];
            }
        } else {
            t.duration = Some(20 + 5 * pick(8) as i64);
            t.required_resources = vec!["LASER-1".into(), "LASER-2".into(), "LASER-3".into()];
            if thick {
                t.required_tags = vec!["Thick plate".into()];
            }
        }
        if o % 6 == 1 {
            // Material from the mill arrives later in the week.
            t.min_start = Some(at(pick(3) as i64, 10, 0));
        }
        step(&mut b, "cut", t);

        // 2. Bend sheet parts.
        if !turned {
            let mut t = task("", &label("Bend"), 15 + 5 * pick(7) as i64);
            t.required_resources = vec!["PRESS-1".into(), "PRESS-2".into()];
            if thick {
                t.required_resources = vec!["PRESS-2".into()];
            }
            step(&mut b, "bend", t);
        }

        // 3. Mill, in one or two setups on the same machine.
        let mut t = task(
            "",
            &label(if two_setups { "Mill, rough" } else { "Mill" }),
            60 + 15 * pick(7) as i64,
        );
        t.task_type = Some(material.into());
        t.required_resources = (1..=4).map(|m| format!("MILL-{m}")).collect();
        t.duration_on = vec![on("MILL-4", t.duration.unwrap() * 3 / 4)];
        if five_axis {
            t.required_tags = vec!["5-axis".into()];
        }
        if t.duration.unwrap() >= 120 {
            // Long unattended runs must not straddle a shift change.
            t.no_interruptions = Some(true);
        }
        let rough = step(&mut b, "mill", t);
        if two_setups {
            let mut t = task("", &label("Mill, finish"), 45 + 15 * pick(3) as i64);
            t.task_type = Some(material.into());
            t.required_resources = (1..=4).map(|m| format!("MILL-{m}")).collect();
            if five_axis {
                t.required_tags = vec!["5-axis".into()];
            }
            let mut dep = after(&rough);
            dep.same_resource = true;
            dep.no_intermediate_tasks = true;
            t.dependencies = vec![dep];
            step(&mut b, "finish", t);
        }

        // 4. Weld: by robot for long seams, by hand otherwise.
        if welded {
            let mut t = task("", &label("Weld"), 45 + 15 * pick(5) as i64);
            if pick(3) == 0 {
                t.required_resources = vec!["WELD-1".into(), "WELD-2".into()];
            } else {
                t.required_tags = vec!["Welding cert".into()];
                t.required_resources = vec!["W-PIETER".into(), "W-SANNE".into(), "W-JORIS".into()];
            }
            step(&mut b, "weld", t);
        }

        // 5. Powder coat; the next step waits for the parts to cool.
        let mut cure = None;
        if painted {
            let mut t = task(
                "",
                &label(&format!("Powder coat {colour}")),
                60 + 10 * pick(4) as i64,
            );
            t.task_type = Some(colour.into());
            t.required_resources = vec!["PAINT-1".into(), "PAINT-2".into()];
            t.no_interruptions = Some(true);
            t.consumes = vec![Part {
                part: format!("{colour} powder"),
                quantity: 1,
            }];
            cure = Some(step(&mut b, "paint", t));
        }

        // 6. Assemble the larger products.
        if matches!(part, "Control cabinet" | "Pump skid" | "Conveyor frame") {
            let mut t = task("", &label("Assemble"), 60 + 15 * pick(5) as i64);
            t.required_tags = vec!["Assembly".into()];
            t.required_resources = vec!["A-FATIMA".into(), "A-LUCAS".into(), "A-EMMA".into()];
            if let Some(paint) = &cure {
                let mut dep = after(paint);
                dep.min_offset = Some(60);
                t.dependencies = vec![dep];
                cure = None;
            }
            step(&mut b, "assemble", t);
        }

        // 7. Inspect and pack against the delivery date.
        let mut t = task("", &label("Inspect and pack"), 20 + 5 * pick(5) as i64);
        t.required_tags = vec!["QA".into()];
        t.required_resources = vec!["Q-NOOR".into(), "Q-DAAN".into()];
        if let Some(paint) = &cure {
            let mut dep = after(paint);
            dep.min_offset = Some(60);
            dep.max_offset = Some(24 * 60);
            t.dependencies = vec![dep];
        }
        let due_day = 1 + (o as i64 * 3 / 2) % (DAYS - 1) + i64::from(o % 3 == 0);
        t.due_date = Some(at(due_day.min(DAYS - 1), 15, 30));
        t.priority = Some(priority);
        if rush {
            t.max_end = Some(at((due_day + 1).min(DAYS - 1), 15, 30));
        }
        if o % 9 == 4 {
            // The customer's truck is booked for a fixed slot.
            t.ideal_end = Some(at(due_day.min(DAYS - 1), 13, 0));
        }
        step(&mut b, "inspect", t);
    }

    let mut plan = b.finish();
    // Powder for every coat but two of each colour is in stock; the other
    // boxes come in on Tuesday's first delivery.
    for colour in COLOURS {
        let part = format!("{colour} powder");
        let coats = plan
            .tasks
            .iter()
            .flat_map(|t| &t.consumes)
            .filter(|c| c.part == part)
            .count() as i64;
        plan.stock.push(StockArrival {
            part: part.clone(),
            quantity: (coats - 2).max(0),
            at: None,
        });
        plan.stock.push(StockArrival {
            part,
            quantity: coats.min(2),
            at: Some(at(1, 6, 0)),
        });
    }
    // Monday's first laser job has been running since the shift started.
    let running = plan
        .tasks
        .iter()
        .find(|t| {
            t.id.ends_with("-cut")
                && t.required_resources.iter().any(|r| r == "LASER-1")
                && t.required_tags.is_empty()
                && t.min_start.is_none()
        })
        .map(|t| t.id.clone());
    if let (Some(running), Some(laser)) = (
        running,
        plan.resources.iter_mut().find(|r| r.id == "LASER-1"),
    ) {
        laser.tasks = vec![placed(&running, Some(at(0, 6, 0)))];
        laser.freeze_tasks_until = Some(at(0, 6, 30));
    }
    plan
}
