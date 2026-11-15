//! The schedule as a timeline: one lane per machine and employee, open tasks
//! in an Unscheduled lane on top, changeovers as their own items and
//! unavailable time as grey overlays.

use serde_json::{json, Value};
use solverforge_workbench_host::view::{blame, Broken, Timeline, View, HARD_TONE, MEDIUM_TONE};
use solverforge_workbench_host::{Analysis, Level};

use crate::domain::Plan;
use crate::schedule::Timing;
use crate::time::{axis_labels, clock, duration_label, span_label, DAY};
use crate::wire::ResourceKind;

/// Task tones by task type; slate is the changeovers'.
const TYPE_TONES: [&str; 5] = ["amber", "blue", "violet", "cyan", "emerald"];
const CHANGEOVER_TONE: &str = "slate";
const MAX_LANE_BADGES: usize = 4;

/// Unavailable time is grey, so red and rose only ever mean a broken rule.
fn unavailable() -> Value {
    json!({"background": "rgba(75, 85, 99, 0.16)", "border": "#6b7280",
           "overlay": "rgba(75, 85, 99, 0.16)", "text": "#374151"})
}

fn broken_badge(level: Level) -> Value {
    match level {
        Level::Hard => {
            json!({"bg": "rgba(220, 38, 38, 0.12)", "border": "1px solid #dc2626", "color": "#b91c1c"})
        }
        _ => {
            json!({"bg": "rgba(225, 29, 72, 0.08)", "border": "1px solid #fb7185", "color": "#be123c"})
        }
    }
}

/// Red for any hard rule the item takes part in breaking, rose for medium.
fn tone(broken: &[Broken]) -> Option<&'static str> {
    if broken.iter().any(|b| b.level == Level::Hard) {
        Some(HARD_TONE)
    } else if broken.iter().any(|b| b.level == Level::Medium) {
        Some(MEDIUM_TONE)
    } else {
        None
    }
}

fn rules(broken: &[Broken]) -> String {
    let mut names: Vec<&str> = broken.iter().map(|b| b.constraint.as_str()).collect();
    names.dedup();
    names.join("; ")
}

pub fn view(plan: &Plan, analysis: &Analysis) -> Option<View> {
    let facts = &*plan.facts;
    if facts.resources.is_empty() {
        return None;
    }
    let timing = Timing::of(plan);
    let broken = blame(analysis);
    let none: Vec<Broken> = Vec::new();
    let broken_of = |type_name: &str, id: &str| {
        broken
            .get(&(type_name.to_string(), id.to_string()))
            .unwrap_or(&none)
    };
    let first = facts.resources.iter().map(|r| r.start).min()?;
    let day0 = first.div_euclid(DAY) * DAY;
    let last = (0..plan.tasks.len())
        .filter(|&t| timing.assigned(t))
        .map(|t| timing.end[t])
        .chain(facts.tasks.iter().filter_map(|t| t.max_end.or(t.due)))
        .max()
        .unwrap_or(first)
        .max(first + 1);
    let span_days = ((last - day0 + DAY - 1) / DAY).max(1);

    let type_tone = |t: usize| {
        facts.tasks[t]
            .task_type
            .map_or(TYPE_TONES[0], |k| TYPE_TONES[k % TYPE_TONES.len()])
    };
    let task_item = |t: usize, start: i64, end: i64, place: String| -> Value {
        let task = &plan.tasks[t];
        let broken = broken_of("Task", &task.id);
        let mut notes = vec![place];
        if let Some(kind) = &task.task_type {
            notes.push(kind.clone());
        }
        notes.push(format!("priority {}", facts.tasks[t].priority));
        if task.pinned {
            notes.push("pinned".into());
        }
        if !broken.is_empty() {
            notes.push(format!("breaks: {}", rules(broken)));
        }
        json!({
            "id": task.id,
            "startMinute": start - day0,
            "endMinute": end - day0,
            "label": format!("{}{}", if task.pinned { "📌 " } else { "" },
                             task.name.as_deref().unwrap_or(&task.id)),
            "meta": notes.join(" · "),
            "tone": tone(broken).unwrap_or_else(|| type_tone(t)),
        })
    };

    let mut lanes = Vec::new();
    let open: Vec<usize> = (0..plan.tasks.len())
        .filter(|&t| !timing.assigned(t))
        .collect();
    if !open.is_empty() {
        let items: Vec<Value> = open
            .iter()
            .map(|&t| {
                let tf = &facts.tasks[t];
                let start = tf.min_start.unwrap_or(first).max(day0);
                let minutes = tf.duration.iter().copied().min().unwrap_or(0).max(1);
                task_item(t, start, start + minutes, "not planned".into())
            })
            .collect();
        lanes.push(json!({
            "id": "unscheduled", "label": "Unscheduled", "mode": "detailed",
            "badges": [{"label": format!("{} open", open.len())}],
            "stats": [{"label": "Tasks", "value": open.len()}],
            "overlays": [],
            "items": items,
        }));
    }
    for (r, order) in timing.order.iter().enumerate() {
        let resource = &plan.resources[r];
        let rf = &facts.resources[r];
        let mut items = Vec::new();
        let mut busy = 0;
        let mut idle = 0;
        let mut changeovers = 0;
        for &t in order {
            if let Some(changeover) = timing.changeover[t] {
                changeovers += 1;
                let from = plan.task_types[changeover.from_type].id.clone();
                let to = facts.tasks[t]
                    .task_type
                    .map_or(String::new(), |k| plan.task_types[k].id.clone());
                items.push(json!({
                    "id": format!("changeover:{}", plan.tasks[t].id),
                    "startMinute": changeover.start - day0,
                    "endMinute": changeover.end.max(changeover.start + 1) - day0,
                    "label": format!("Setup {from} → {to}"),
                    "meta": format!("{} · {}", span_label(changeover.start, changeover.end),
                                    duration_label(changeover.minutes)),
                    "tone": CHANGEOVER_TONE,
                }));
            }
            busy += facts.tasks[t].duration[r];
            idle += timing.idle[t];
            let place = span_label(timing.start[t], timing.end[t]);
            items.push(task_item(t, timing.start[t], timing.end[t], place));
        }
        let mut badges: Vec<Value> = Vec::new();
        let own = broken_of("Resource", &resource.id);
        let mut seen: Vec<&str> = Vec::new();
        for b in own.iter().filter(|b| b.level != Level::Soft) {
            if !seen.contains(&b.constraint.as_str()) {
                seen.push(&b.constraint);
                badges.push(json!({"label": b.constraint, "style": broken_badge(b.level)}));
            }
        }
        badges.push(
            json!({"label": match (resource.kind, &resource.machine_type) {
                (ResourceKind::Machine, Some(kind)) => kind.clone(),
                (ResourceKind::Machine, None) => "Machine".to_string(),
                (ResourceKind::Employee, _) => "Employee".to_string(),
            }}),
        );
        for tag in &resource.tags {
            badges.push(json!({"label": tag.tag}));
        }
        badges.truncate(MAX_LANE_BADGES);
        let mut stats = vec![
            json!({"label": "Tasks", "value": order.len()}),
            json!({"label": "Busy h", "value": (busy as f64 / 6.0).round() / 10.0}),
        ];
        if rf.machine {
            stats.push(json!({"label": "Setups", "value": changeovers}));
            stats.push(json!({"label": "Idle h", "value": (idle as f64 / 6.0).round() / 10.0}));
        }
        let mut overlays: Vec<Value> = resource
            .unavailable
            .iter()
            .filter_map(|span| {
                let from = crate::time::parse_date_time(&span.start)?;
                let to = crate::time::parse_date_time(&span.end)?;
                Some(json!({"startMinute": from - day0, "endMinute": to - day0,
                            "label": span.label.clone().unwrap_or_else(|| "Unavailable".into()),
                            "tone": unavailable()}))
            })
            .collect();
        if rf.start > day0 {
            overlays.push(json!({"startMinute": 0, "endMinute": rf.start - day0,
                                 "label": format!("Starts {}", clock(rf.start)),
                                 "tone": unavailable()}));
        }
        lanes.push(json!({
            "id": format!("resource:{}", resource.id),
            "label": resource.name.clone().unwrap_or_else(|| resource.id.clone()),
            "mode": "detailed",
            "badges": badges,
            "stats": stats,
            "overlays": overlays,
            "items": items,
        }));
    }

    let days: Vec<Value> = (0..span_days)
        .map(|n| {
            let date = day0.div_euclid(DAY) + n;
            let (weekday, sub) = axis_labels(date);
            json!({"label": weekday, "subLabel": sub,
                   "isWeekend": matches!(weekday, "Sat" | "Sun")})
        })
        .collect();
    let ticks: Vec<Value> = (0..=span_days * DAY)
        .step_by(360)
        .map(|m| json!({"minute": m, "label": format!("{:02}:00", (m % DAY) / 60)}))
        .collect();
    let shown = span_days.min(7);
    let planned = plan.tasks.len() - open.len();
    Some(View::Timeline(Timeline {
        title: "Schedule".into(),
        subtitle: format!(
            "{planned} tasks planned · {} open · {} resources · colours are task types, slate \
             is a changeover · red: breaks a hard rule · rose: a medium one (left open) · grey: \
             unavailable",
            open.len(),
            facts.resources.len()
        ),
        label: "Resource".into(),
        model: json!({
            "axis": {"startMinute": 0, "endMinute": span_days * DAY, "days": days, "ticks": ticks,
                     "initialViewport": {"startMinute": 0, "endMinute": shown * DAY}},
            "lanes": lanes,
        }),
    }))
}
