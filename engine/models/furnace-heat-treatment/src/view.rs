//! The furnace schedule: one lane per furnace with its loads, changeovers
//! and downtime, a crew lane for monitoring and staffing gaps, and the
//! orders left unscheduled on top.
//!
//! A hard or medium match marks the orders it names, except for rules over
//! many loads at once (monitoring, staffing, a furnace's changeovers): those
//! are drawn where they break, as red stretches on the crew lane and red
//! changeovers on the furnace lane.

use std::collections::{BTreeMap, HashMap};

use serde_json::{json, Value};
use solverforge::stream::Projection;
use solverforge::stream::ProjectionSink;
use solverforge_workbench_host::view::{level_cost, Timeline, View, HARD_TONE, MEDIUM_TONE};
use solverforge_workbench_host::{Analysis, Level};

use crate::derived::{Derived, ShiftId, BUCKET};
use crate::domain::Plan;
use crate::eval::{
    Monitor, MonitorAccumulator, MonitorCollector, OrderWatch, RosterWatch, WatchRow,
};
use crate::labels::{clock, date, furnace as furnace_name, operator, MONTHS, WEEKDAYS};
use crate::schedule::{furnace_changeovers, loads, Load};
use crate::time::{civil_from_days, weekday, DAY};
use crate::wire::{Process, ShiftKind};

const MONITORING: &str = "Too few operators monitoring";
const STAFFING: &str = "Shift short of a role";
const CHANGEOVER: &str = "Changeover time too short";

/// Downtime is grey, so red and rose only ever mean a broken rule.
fn downtime_tone() -> Value {
    json!({"background": "rgba(75, 85, 99, 0.16)", "border": "#6b7280",
           "overlay": "rgba(75, 85, 99, 0.16)", "text": "#374151"})
}

fn process_tone(process: Process) -> &'static str {
    match process {
        Process::Quenching | Process::Normalizing => "amber",
        Process::Tempering | Process::StressRelieving => "blue",
        Process::Carburizing => "violet",
        Process::Nitriding => "emerald",
        Process::Annealing => "cyan",
        Process::SolutionTreating | Process::Aging => "slate",
    }
}

/// Rules broken by each order id, and the cost of each rule over all its matches.
struct Flags {
    orders: HashMap<String, Vec<(Level, String)>>,
    rules: HashMap<String, (Level, i64)>,
}

fn flags(analysis: &Analysis) -> Flags {
    let mut out = Flags {
        orders: HashMap::new(),
        rules: HashMap::new(),
    };
    for constraint in &analysis.constraints {
        if constraint.level == Level::Soft {
            continue;
        }
        for found in &constraint.matches {
            let cost = level_cost(&found.score, constraint.level);
            if cost <= 0 {
                continue;
            }
            let rule = out
                .rules
                .entry(constraint.name.clone())
                .or_insert((constraint.level, 0));
            rule.1 += cost;
            if [MONITORING, STAFFING, CHANGEOVER].contains(&constraint.name.as_str()) {
                continue;
            }
            for entity in found.entities.iter().filter(|e| e.type_name == "WorkOrder") {
                if let Some(id) = &entity.name.id {
                    out.orders
                        .entry(id.clone())
                        .or_default()
                        .push((constraint.level, constraint.name.clone()));
                }
            }
        }
    }
    out
}

fn tone_of(broken: &[(Level, String)], fallback: &'static str) -> &'static str {
    if broken.iter().any(|(level, _)| *level == Level::Hard) {
        HARD_TONE
    } else if !broken.is_empty() {
        MEDIUM_TONE
    } else {
        fallback
    }
}

fn breaks_note(broken: &[(Level, String)]) -> Option<String> {
    let mut names: Vec<&str> = broken.iter().map(|(_, n)| n.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    (!names.is_empty()).then(|| format!("breaks: {}", names.join("; ")))
}

fn load_item(plan: &Plan, d: &Derived, load: &Load, flags: &Flags) -> Value {
    let recipe = &plan.recipes[load.recipe];
    let first = &plan.work_orders[load.orders[0]];
    let mut broken = Vec::new();
    let mut orders = Vec::new();
    let mut pinned = false;
    for &index in &load.orders {
        let order = &plan.work_orders[index];
        pinned |= order.pinned;
        broken.extend(flags.orders.get(&order.id).cloned().unwrap_or_default());
        let customer = order
            .customer
            .as_deref()
            .map(|c| format!("{c}, "))
            .unwrap_or_default();
        orders.push(format!("{} ({customer}{} kg)", order.id, order.load_kg));
    }
    let crew: Vec<String> = {
        let mut names: Vec<String> = load
            .orders
            .iter()
            .flat_map(|&i| {
                let o = &plan.work_orders[i];
                o.tasks().filter_map(|t| o.op(t)).collect::<Vec<_>>()
            })
            .map(|op| operator(plan, op))
            .collect();
        names.sort();
        names.dedup();
        names
    };
    let info = &d.recipes[load.recipe];
    let mut notes = vec![
        format!(
            "{}–{}",
            clock(d.origin + load.start),
            clock(d.origin + load.end)
        ),
        format!(
            "{} {} °C, {} min",
            info.process.label(),
            info.temperature,
            info.cycle
        ),
        orders.join(", "),
        format!("{} / {} kg", load.kg, d.furnaces[load.furnace].max_load_kg),
    ];
    if !crew.is_empty() {
        notes.push(format!("crew: {}", crew.join(", ")));
    }
    if pinned {
        notes.push("pinned".into());
    }
    notes.extend(breaks_note(&broken));
    let name = recipe.name.clone().unwrap_or_else(|| recipe.id.clone());
    let count = if load.orders.len() > 1 {
        format!(" ×{}", load.orders.len())
    } else {
        String::new()
    };
    json!({
        "id": first.id,
        "startMinute": load.start,
        "endMinute": load.end,
        "label": format!("{}{name} · {} °C{count}", if pinned { "📌 " } else { "" }, info.temperature),
        "meta": notes.join(" · "),
        "tone": tone_of(&broken, process_tone(info.process)),
    })
}

/// Collects projected rows into a vector.
struct Rows(Vec<WatchRow>);

impl ProjectionSink<WatchRow> for Rows {
    fn emit(&mut self, row: WatchRow) {
        self.0.push(row);
    }
}

/// Stretches where the monitoring crew is short: `(start, end, operators missing)`.
fn monitoring_gaps(plan: &Plan) -> Vec<(i64, i64, i64)> {
    use solverforge::stream::collector::{Accumulator, Collector};
    let mut rows = Rows(Vec::new());
    for order in &plan.work_orders {
        OrderWatch.project(order, &mut rows);
    }
    for row in &plan.roster {
        RosterWatch.project(row, &mut rows);
    }
    let mut buckets: BTreeMap<i64, MonitorAccumulator> = BTreeMap::new();
    for row in &rows.0 {
        let acc = buckets
            .entry(row.bucket)
            .or_insert_with(|| MonitorCollector.create_accumulator());
        acc.accumulate(MonitorCollector.extract(row));
    }
    let mut gaps: Vec<(i64, i64, i64)> = Vec::new();
    for (bucket, acc) in &buckets {
        let short = acc.with_result(Monitor::shortage);
        if short == 0 {
            continue;
        }
        let (from, to) = (bucket * BUCKET, (bucket + 1) * BUCKET);
        match gaps.last_mut() {
            Some(last) if last.1 == from && last.2 == short => last.1 = to,
            _ => gaps.push((from, to, short)),
        }
    }
    gaps
}

/// Shifts short of a role: `(shift, role label, operators missing)`.
fn staffing_gaps(plan: &Plan) -> Vec<(ShiftId, &'static str, i64)> {
    let mut missing: BTreeMap<(i64, usize, &'static str), i64> = BTreeMap::new();
    for demand in &plan.coverage {
        let day = (demand.shift_number / 3) as i64 - 1;
        *missing
            .entry((day, demand.shift.value(), demand.role.label()))
            .or_default() += demand.count;
    }
    for row in &plan.roster {
        if let Some(shift) = row.shift_id() {
            let role = row.ctx.operators[row.operator_idx()].role.label();
            if let Some(n) = missing.get_mut(&(shift.day, shift.kind.value(), role)) {
                *n -= 1;
            }
        }
    }
    missing
        .into_iter()
        .filter(|(_, n)| *n > 0)
        .filter_map(|((day, kind, role), n)| {
            Some((
                ShiftId {
                    day,
                    kind: ShiftKind::from_value(kind)?,
                },
                role,
                n,
            ))
        })
        .collect()
}

pub fn view(plan: &Plan, analysis: &Analysis) -> Option<View> {
    let d = plan.work_orders.first().map(|o| o.ctx.clone())?;
    let flags = flags(analysis);
    let all_loads = loads(plan);
    let span_days = d.days;
    let mut lanes = Vec::new();

    // Crew lane: monitoring and staffing gaps, only while those rules count.
    let mut crew_items = Vec::new();
    if let Some((level, _)) = flags.rules.get(MONITORING) {
        for (n, (from, to, short)) in monitoring_gaps(plan).into_iter().enumerate() {
            crew_items.push(json!({
                "id": format!("monitoring-{n}"), "startMinute": from, "endMinute": to,
                "label": format!("{short} monitor{} short", if short == 1 { "" } else { "s" }),
                "meta": format!("{}–{} · breaks: {MONITORING}", clock(d.origin + from), clock(d.origin + to)),
                "tone": if *level == Level::Hard { HARD_TONE } else { MEDIUM_TONE },
            }));
        }
    }
    if let Some((level, _)) = flags.rules.get(STAFFING) {
        for (n, (shift, role, short)) in staffing_gaps(plan).into_iter().enumerate() {
            let (from, to) = d.shift_bounds(shift);
            crew_items.push(json!({
                "id": format!("staffing-{n}"), "startMinute": from.max(0), "endMinute": to,
                "label": format!("{short} × {} short", role.to_lowercase()),
                "meta": format!("{} {} shift · breaks: {STAFFING}", date(d.origin / DAY + shift.day), shift.kind.label()),
                "tone": if *level == Level::Hard { HARD_TONE } else { MEDIUM_TONE },
            }));
        }
    }
    let rostered = plan.roster.iter().filter(|r| r.works_in_schedule()).count();
    lanes.push(json!({
        "id": "crew", "label": "Crew", "mode": "detailed",
        "badges": [{"label": format!("{} operators", plan.operators.len())}],
        "stats": [{"label": "Shifts", "value": rostered}],
        "items": crew_items,
    }));

    // Unscheduled orders, drawn ending at their due time.
    let open: Vec<_> = plan
        .work_orders
        .iter()
        .filter(|o| o.slot.is_none())
        .collect();
    if !open.is_empty() {
        let items: Vec<Value> = open
            .iter()
            .map(|order| {
                let info = d.recipe_of(order.index);
                let due = d.orders[order.index].due;
                let broken = flags.orders.get(&order.id).cloned().unwrap_or_default();
                let mut notes = vec![
                    format!("due {} {}", date((d.origin + due).div_euclid(DAY)), clock(d.origin + due)),
                    format!("{} {} °C, {} min, {} kg", info.process.label(), info.temperature, info.cycle, order.load_kg),
                    d.orders[order.index].priority.label().to_string(),
                ];
                notes.extend(breaks_note(&broken));
                json!({
                    "id": order.id, "startMinute": (due - info.cycle).max(0), "endMinute": due.max(info.cycle),
                    "label": order.id, "meta": notes.join(" · "),
                    "tone": tone_of(&broken, "slate"),
                })
            })
            .collect();
        lanes.push(json!({
            "id": "unscheduled", "label": "Not scheduled", "mode": "detailed",
            "badges": [], "stats": [{"label": "Orders", "value": items.len()}],
            "items": items,
        }));
    }

    let short_changeovers = flags.rules.get(CHANGEOVER).map(|(level, _)| *level);
    for (index, furnace) in plan.furnaces.iter().enumerate() {
        let mine: Vec<&Load> = all_loads.iter().filter(|l| l.furnace == index).collect();
        let mut items: Vec<Value> = mine
            .iter()
            .map(|l| load_item(plan, &d, l, &flags))
            .collect();
        for (n, c) in furnace_changeovers(plan, index).into_iter().enumerate() {
            if c.required == 0 {
                continue;
            }
            let broken = c.shortage > 0 && short_changeovers.is_some();
            let mut meta = format!(
                "{} → {} °C · needs {} min, has {} min",
                c.temperatures.0,
                c.temperatures.1,
                c.required,
                c.to - c.from
            );
            if broken {
                meta.push_str(&format!(" · breaks: {CHANGEOVER}"));
            }
            items.push(json!({
                "id": format!("changeover:{}:{n}", furnace.id),
                "startMinute": c.from, "endMinute": c.from + c.required,
                "label": format!("{}→{} °C", c.temperatures.0, c.temperatures.1),
                "meta": meta,
                "tone": match (broken, short_changeovers) {
                    (true, Some(Level::Hard)) => HARD_TONE,
                    (true, _) => MEDIUM_TONE,
                    _ => "slate",
                },
            }));
        }
        let busy: i64 = mine.iter().map(|l| l.end - l.start).sum();
        let overlays: Vec<Value> = furnace
            .downtime
            .iter()
            .zip(&d.furnaces[index].downtime)
            .map(|(window, &(from, to))| {
                json!({"startMinute": from, "endMinute": to,
                       "label": window.reason.clone().unwrap_or_else(|| "Downtime".into()),
                       "tone": downtime_tone()})
            })
            .collect();
        lanes.push(json!({
            "id": format!("furnace:{}", furnace.id),
            "label": furnace_name(plan, index),
            "mode": "detailed",
            "badges": [{"label": furnace.kind.label()},
                       {"label": format!("≤ {} °C", furnace.max_temperature)},
                       {"label": format!("≤ {} kg", furnace.max_load_kg)}],
            "stats": [{"label": "Loads", "value": mine.len()},
                      {"label": "Busy", "value": format!("{}%", (100 * busy / d.horizon.max(1)))}],
            "overlays": overlays,
            "items": items,
        }));
    }

    let day0 = d.origin / DAY;
    let days: Vec<Value> = (0..span_days)
        .map(|n| {
            let date = day0 + n;
            let (_, month, day) = civil_from_days(date);
            let wd = weekday(date) as usize;
            json!({"label": WEEKDAYS[wd], "subLabel": format!("{day} {}", MONTHS[(month - 1) as usize]),
                   "isWeekend": wd >= 5})
        })
        .collect();
    let ticks: Vec<Value> = (0..=span_days * DAY)
        .step_by(360)
        .map(|m| json!({"minute": m, "label": format!("{:02}:00", (m % DAY) / 60)}))
        .collect();
    let shown = span_days.min(7);
    let shared = all_loads.iter().filter(|l| l.orders.len() > 1).count();
    Some(View::Timeline(Timeline {
        title: "Furnace schedule".into(),
        subtitle: format!(
            "{} loads ({shared} shared) · {} furnaces · {} not scheduled · red: breaks a hard rule, \
             on a load, a changeover or the crew lane · rose: a medium one · slate between loads: \
             changeovers · grey: downtime",
            all_loads.len(),
            plan.furnaces.len(),
            open.len()
        ),
        label: "Furnace".into(),
        model: json!({
            "axis": {"startMinute": 0, "endMinute": span_days * DAY, "days": days, "ticks": ticks,
                     "initialViewport": {"startMinute": 0, "endMinute": shown * DAY}},
            "lanes": lanes,
        }),
    }))
}
