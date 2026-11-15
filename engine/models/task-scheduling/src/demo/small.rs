//! WORKSHOP: a job shop with a saw, two mills, a lathe and a fitter, ten
//! customer orders of three operations over three days.

use super::build::{after, at, holds, holds_between, off, on, outside_hours, task, Builder};
use crate::domain::Plan;

/// One customer order: what is made, its material, how long each operation
/// takes and which machine family turns or mills it.
struct Order {
    id: &'static str,
    part: &'static str,
    material: &'static str,
    cut: i64,
    machine: i64,
    finish: i64,
    lathe: bool,
}

const ORDERS: [Order; 10] = [
    Order {
        id: "1041",
        part: "Pump housing",
        material: "aluminium",
        cut: 25,
        machine: 140,
        finish: 45,
        lathe: false,
    },
    Order {
        id: "1042",
        part: "Drive shaft",
        material: "steel",
        cut: 20,
        machine: 90,
        finish: 30,
        lathe: true,
    },
    Order {
        id: "1043",
        part: "Sensor bracket",
        material: "aluminium",
        cut: 15,
        machine: 60,
        finish: 30,
        lathe: false,
    },
    Order {
        id: "1044",
        part: "Flange",
        material: "steel",
        cut: 30,
        machine: 75,
        finish: 40,
        lathe: true,
    },
    Order {
        id: "1045",
        part: "Gearbox cover",
        material: "aluminium",
        cut: 35,
        machine: 150,
        finish: 50,
        lathe: false,
    },
    Order {
        id: "1046",
        part: "Bearing sleeve",
        material: "steel",
        cut: 20,
        machine: 70,
        finish: 25,
        lathe: true,
    },
    Order {
        id: "1047",
        part: "Manifold block",
        material: "aluminium",
        cut: 30,
        machine: 120,
        finish: 60,
        lathe: false,
    },
    Order {
        id: "1048",
        part: "Welded frame",
        material: "steel",
        cut: 40,
        machine: 80,
        finish: 90,
        lathe: false,
    },
    Order {
        id: "1049",
        part: "Spindle nose",
        material: "steel",
        cut: 20,
        machine: 110,
        finish: 30,
        lathe: true,
    },
    Order {
        id: "1050",
        part: "Heat sink",
        material: "aluminium",
        cut: 20,
        machine: 100,
        finish: 35,
        lathe: false,
    },
];

pub fn workshop() -> Plan {
    let mut b = Builder::new("Workshop · three days");
    for tag in ["5-axis", "Welding"] {
        b.tag(tag);
    }
    for kind in ["aluminium", "steel"] {
        b.task_type(kind);
    }
    for kind in ["Saw", "Mill", "Lathe"] {
        b.machine_type(kind);
    }
    // Mills switch material with a coolant and tooling change; the same
    // material twice still needs a tool check.
    b.changeover("Mill", "aluminium", "steel", 30);
    b.changeover("Mill", "steel", "aluminium", 45);
    b.changeover("Mill", "aluminium", "aluminium", 5);
    b.changeover("Mill", "steel", "steel", 10);
    b.changeover("Lathe", "aluminium", "steel", 20);
    b.changeover("Lathe", "steel", "aluminium", 25);

    let days = 3;
    b.machine("SAW", "Band saw", "Saw", at(0, 7, 0)).unavailable =
        outside_hours(days, 7, 17, "Closed");
    let mill = b.machine("MILL-5X", "5-axis mill", "Mill", at(0, 7, 0));
    mill.tags = vec![holds("5-axis")];
    mill.initial_task_type = Some("aluminium".into());
    mill.unavailable = outside_hours(days, 7, 17, "Closed");
    let mill = b.machine("MILL-3X", "3-axis mill", "Mill", at(0, 7, 0));
    mill.initial_task_type = Some("steel".into());
    mill.unavailable = outside_hours(days, 7, 17, "Closed");
    mill.unavailable
        .push(off(at(1, 7, 0), at(1, 9, 0), "Spindle service"));
    let lathe = b.machine("LATHE", "CNC lathe", "Lathe", at(0, 7, 0));
    lathe.unavailable = outside_hours(days, 7, 17, "Closed");
    let fitter = b.employee("ANNA", "Anna Visser (fitter)", at(0, 7, 0));
    // Anna's welding certificate is renewed on Wednesday at noon.
    fitter.tags = vec![holds_between("Welding", at(0, 0, 0), at(2, 12, 0))];
    fitter.unavailable = outside_hours(days, 7, 16, "Off shift");
    for day in 0..days {
        fitter
            .unavailable
            .push(off(at(day, 12, 0), at(day, 12, 30), "Lunch"));
    }
    // The saw stands next to the lathe: bars go straight across.
    b.hand_over("SAW", "LATHE", Some(15));

    for (n, order) in ORDERS.iter().enumerate() {
        let cut = format!("{}-cut", order.id);
        let machine = format!("{}-{}", order.id, if order.lathe { "turn" } else { "mill" });
        let finish = format!("{}-finish", order.id);
        let t = b.add(task(
            &cut,
            &format!("Cut stock · {}", order.part),
            order.cut,
        ));
        t.task_type = Some(order.material.into());
        t.required_resources = vec!["SAW".into()];
        if order.material == "steel" && n % 3 == 0 {
            // Steel bar from the stockist arrives Monday 10:00.
            t.min_start = Some(at(0, 10, 0));
        }

        let verb = if order.lathe { "Turn" } else { "Mill" };
        let t = b.add(task(
            &machine,
            &format!("{verb} · {}", order.part),
            order.machine,
        ));
        t.task_type = Some(order.material.into());
        t.dependencies = vec![after(&cut)];
        if order.lathe {
            t.required_resources = vec!["LATHE".into()];
        } else {
            t.required_resources = vec!["MILL-5X".into(), "MILL-3X".into()];
            // The old mill is slower.
            t.duration_on = vec![on("MILL-3X", order.machine * 5 / 4)];
            if order.machine >= 140 {
                t.required_tags = vec!["5-axis".into()];
            }
        }

        let welded = order.part.contains("Welded");
        let t = b.add(task(
            &finish,
            &format!(
                "{} · {}",
                if welded { "Weld" } else { "Deburr and fit" },
                order.part
            ),
            order.finish,
        ));
        let mut dep = after(&machine);
        if order.part.contains("Manifold") {
            // Leak test needs the coolant drained: 30 minutes to 4 hours.
            dep.min_offset = Some(30);
            dep.max_offset = Some(240);
        }
        t.dependencies = vec![dep];
        t.required_resources = vec!["ANNA".into()];
        if welded {
            t.required_tags = vec!["Welding".into()];
        }
        t.due_date = Some(at(1 + (n as i64 % 2), 16, 0));
        t.priority = Some(if n == 2 { 5 } else { 1 + n as i64 % 2 });
        if n == 2 {
            // A rush order the courier collects on Monday evening.
            t.max_end = Some(at(0, 17, 0));
        }
        if n == 6 {
            // The customer collects on Tuesday at 15:00.
            t.ideal_end = Some(at(1, 15, 0));
        }
    }
    b.finish()
}
