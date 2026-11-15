//! Short constructors for demo records.

use crate::domain::{Plan, Task};
use crate::time::{format_date, parse_date};
use crate::wire::{
    Bay, Crew, CrewChange, Delivery, Horizon, Labour, Parameters, Part, PartNeed, ReadinessRule,
    Span, Unit, Usage,
};

/// The date `day` days after `first` (`YYYY-MM-DD`).
pub fn date(first: &str, day: i64) -> String {
    format_date(parse_date(first).expect("demo date") + day)
}

pub fn plan(first: &str, days: i64, parameters: Parameters) -> Plan {
    Plan {
        horizon: Horizon {
            start: first.into(),
            days,
        },
        parameters,
        units: Vec::new(),
        bays: Vec::new(),
        crews: Vec::new(),
        parts: Vec::new(),
        readiness: Vec::new(),
        tasks: Vec::new(),
        slots: Vec::new(),
        days: Vec::new(),
        weights: Default::default(),
        score: None,
    }
}

/// A unit with a usage counter, its daily growth and its downtime cost.
pub fn unit(id: &str, fleet: &str, current: f64, per_day: f64, downtime_cost: i64) -> Unit {
    let mut unit = Unit::new(id, fleet);
    unit.usage = Some(Usage { current, per_day });
    unit.downtime_cost = downtime_cost;
    unit
}

pub fn bay(id: &str, name: &str, class: &str, capacity: i64, cost_per_day: i64) -> Bay {
    let mut bay = Bay::new(id, class);
    bay.name = Some(name.into());
    bay.capacity = capacity;
    bay.cost_per_day = cost_per_day;
    bay
}

pub fn crew(id: &str, name: &str, technicians: i64, overtime: i64) -> Crew {
    Crew {
        id: id.into(),
        name: Some(name.into()),
        technicians,
        overtime,
        changes: Vec::new(),
    }
}

pub fn change(first: &str, from: i64, to: i64, technicians: i64, label: &str) -> CrewChange {
    CrewChange {
        from: date(first, from),
        to: date(first, to),
        technicians,
        label: Some(label.into()),
    }
}

pub fn span(first: &str, from: i64, to: i64, label: &str) -> Span {
    Span::new(date(first, from), date(first, to), label)
}

/// A part with its opening stock and `(day, quantity)` deliveries.
pub fn part(first: &str, id: &str, name: &str, on_hand: i64, deliveries: &[(i64, i64)]) -> Part {
    Part {
        id: id.into(),
        name: Some(name.into()),
        on_hand,
        deliveries: deliveries
            .iter()
            .map(|&(day, quantity)| Delivery {
                date: date(first, day),
                quantity,
            })
            .collect(),
    }
}

pub fn rule(id: &str, name: &str, fleet: Option<&str>, minimum: i64, target: i64) -> ReadinessRule {
    ReadinessRule {
        id: id.into(),
        name: Some(name.into()),
        fleet: fleet.map(str::to_string),
        minimum,
        target,
        from: None,
        to: None,
    }
}

/// A task with its bay classes and `(crew, technicians)` labour.
pub fn task(
    id: &str,
    unit: &str,
    name: &str,
    days: i64,
    classes: &[&str],
    labour: &[(&str, i64)],
) -> Task {
    let mut task = Task::new(id, unit, name, days);
    task.bay_classes = classes.iter().map(|c| c.to_string()).collect();
    task.labour = labour
        .iter()
        .map(|&(crew, technicians)| Labour {
            crew: crew.into(),
            technicians,
        })
        .collect();
    task
}

/// The task consumes `quantity` of `part`.
pub fn needs(mut task: Task, part: &str, quantity: i64) -> Task {
    task.parts.push(PartNeed {
        part: part.into(),
        quantity,
    });
    task
}
