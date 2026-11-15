//! The workshop timeline: fleet readiness on top, then unplanned work, then
//! one lane per bay with its tasks and its closures.
//!
//! A task is red when it breaks a hard rule of its own or one it shares with
//! the other tasks of a bay, crew, unit or part (a crowded bay, a short crew),
//! and rose when it is mandatory and unplanned. Readiness is judged per day on
//! the top lane, where a red day is below a minimum, rather than on every task
//! that keeps a unit down that day.

use std::collections::BTreeMap;

use serde_json::{json, Value};
use solverforge_workbench_host::view::{blame, Broken, Timeline, View, HARD_TONE, MEDIUM_TONE};
use solverforge_workbench_host::{Analysis, Level};

use crate::derived::Derived;
use crate::domain::{Plan, Task};
use crate::status::Fleet;
use crate::time::{short_date, span_label, weekday, weekday_label, DAY};

/// Tones of the fleet types, in order of first appearance.
const FLEET_TONES: [&str; 5] = ["blue", "violet", "cyan", "amber", "slate"];
/// The constraints the readiness lane shows instead of the tasks.
const READINESS_RULES: [&str; 1] = ["Readiness below minimum"];

/// Closed bays are grey, so red only ever means a broken rule.
fn closed() -> Value {
    json!({"background": "rgba(75, 85, 99, 0.16)", "border": "#6b7280",
           "overlay": "rgba(75, 85, 99, 0.16)", "text": "#374151"})
}

fn tone_of(broken: &[Broken]) -> Option<&'static str> {
    let counted = broken
        .iter()
        .filter(|b| !READINESS_RULES.contains(&b.constraint.as_str()));
    let mut tone = None;
    for b in counted {
        match b.level {
            Level::Hard => return Some(HARD_TONE),
            Level::Medium => tone = Some(MEDIUM_TONE),
            Level::Soft => {}
        }
    }
    tone
}

fn names_of(broken: &[Broken]) -> Vec<&str> {
    broken.iter().map(|b| b.constraint.as_str()).collect()
}

struct Items<'a> {
    plan: &'a Plan,
    d: &'a Derived,
    blamed: BTreeMap<(String, String), Vec<Broken>>,
}

impl Items<'_> {
    fn item(&self, task: &Task, (start, end): (i64, i64)) -> Value {
        let d = self.d;
        let info = &d.tasks[task.index];
        let unit = &self.plan.units[info.unit];
        let broken = self
            .blamed
            .get(&("Task".to_string(), task.id.clone()))
            .map_or(&[][..], Vec::as_slice);
        let mut notes = vec![span_label(d.origin + start, d.origin + end)];
        notes.push(format!(
            "{} day{}",
            info.days,
            if info.days == 1 { "" } else { "s" }
        ));
        if !task.labour.is_empty() {
            let crews: Vec<String> = task
                .labour
                .iter()
                .map(|l| format!("{} {}", l.technicians, l.crew))
                .collect();
            notes.push(crews.join(", "));
        }
        if !task.parts.is_empty() {
            let parts: Vec<String> = task
                .parts
                .iter()
                .map(|p| format!("{}× {}", p.quantity, p.part))
                .collect();
            notes.push(parts.join(", "));
        }
        if let Some(due) = &task.due_date {
            notes.push(format!("due {due}"));
        }
        if task.pinned {
            notes.push("pinned".into());
        }
        if info.deferrable {
            notes.push("deferrable".into());
        }
        if !broken.is_empty() {
            notes.push(format!("breaks: {}", names_of(broken).join("; ")));
        }
        let fleet_tone = FLEET_TONES[d.units[info.unit].fleet % FLEET_TONES.len()];
        let tone = tone_of(broken).unwrap_or(if task.slot_idx.is_some() {
            fleet_tone
        } else {
            "slate"
        });
        json!({
            "id": task.id,
            "startMinute": start * DAY,
            "endMinute": end * DAY,
            "label": format!("{}{} · {}", if task.pinned { "📌 " } else { "" },
                             unit.name.as_deref().unwrap_or(&unit.id), task.name),
            "meta": notes.join(" · "),
            "tone": tone,
        })
    }
}

pub fn view(plan: &Plan, analysis: &Analysis) -> Option<View> {
    let fleet = Fleet::of(plan)?;
    let d = fleet.d;
    let days = d.days;
    let items = Items {
        plan,
        d,
        blamed: blame(analysis),
    };
    let mut lanes = vec![readiness_lane(plan, &fleet)];

    let mut open: Vec<&Task> = plan.tasks.iter().filter(|t| t.slot_idx.is_none()).collect();
    if !open.is_empty() {
        open.sort_by_key(|t| (d.tasks[t.index].earliest, t.index));
        let mandatory = open.iter().filter(|t| !d.tasks[t.index].deferrable).count();
        let badges: Vec<Value> = if mandatory > 0 {
            vec![json!({"label": format!("{mandatory} mandatory")})]
        } else {
            Vec::new()
        };
        let drawn: Vec<Value> = open
            .iter()
            .map(|t| {
                let start = d.tasks[t.index].earliest.min(days - 1);
                items.item(t, (start, start + d.tasks[t.index].days))
            })
            .collect();
        lanes.push(json!({
            "id": "unplanned", "label": "Unplanned", "mode": "detailed",
            "badges": badges,
            "stats": [{"label": "Tasks", "value": open.len()}],
            "items": drawn,
        }));
    }

    for (b, bay) in plan.bays.iter().enumerate() {
        let mut mine: Vec<&Task> = plan
            .tasks
            .iter()
            .filter(|t| t.slot_idx.is_some_and(|slot| d.slot_bay(slot) == b))
            .collect();
        mine.sort_by_key(|t| (t.slot_idx, t.index));
        let drawn: Vec<Value> = mine
            .iter()
            .filter_map(|t| {
                t.slot_idx
                    .map(|slot| items.item(t, d.occupied(t.index, slot)))
            })
            .collect();
        let used: i64 = mine.iter().map(|t| d.tasks[t.index].days).sum();
        let info = &d.bays[b];
        let open_days: i64 = days
            - info
                .closures
                .iter()
                .map(|&(from, to)| crate::time::overlap(from, to, 0, days))
                .sum::<i64>();
        let room = (open_days * info.capacity).max(1);
        let mut badges = vec![json!({"label": bay.class})];
        if info.capacity > 1 {
            badges.push(json!({"label": format!("holds {}", info.capacity)}));
        }
        if !bay.fleets.is_empty() {
            badges.push(json!({"label": bay.fleets.join(", ")}));
        }
        let overlays: Vec<Value> = bay
            .closures
            .iter()
            .zip(&info.closures)
            .map(|(span, &(from, to))| {
                json!({"startMinute": from.max(0) * DAY, "endMinute": to.min(days) * DAY,
                       "label": span.label.clone().unwrap_or_else(|| "Closed".into()),
                       "tone": closed()})
            })
            .filter(|o| o["endMinute"].as_i64() > o["startMinute"].as_i64())
            .collect();
        lanes.push(json!({
            "id": format!("bay:{}", bay.id),
            "label": bay.name.clone().unwrap_or_else(|| bay.id.clone()),
            "mode": "detailed",
            "badges": badges,
            "stats": [{"label": "Tasks", "value": mine.len()},
                      {"label": "Use", "value": format!("{}%", 100 * used.min(room) / room)}],
            "overlays": overlays,
            "items": drawn,
        }));
    }

    let axis_days: Vec<Value> = (0..days)
        .map(|n| {
            let date = d.origin + n;
            json!({"label": weekday_label(date), "subLabel": short_date(date),
                   "isWeekend": weekday(date) >= 5})
        })
        .collect();
    let ticks: Vec<Value> = (0..=days)
        .map(|n| json!({"minute": n * DAY, "label": short_date(d.origin + n)}))
        .collect();
    let shown = if days > 21 { 14 } else { days };
    let planned = plan.tasks.len() - open.len();
    Some(View::Timeline(Timeline {
        title: "Workshop plan".into(),
        subtitle: format!(
            "{planned} of {} tasks planned · {} bays · {} units · top lane: ready units per day, \
             red below a minimum, amber below a target · red task: breaks a hard rule · rose: \
             mandatory and unplanned · grey: bay closed",
            plan.tasks.len(),
            plan.bays.len(),
            plan.units.len()
        ),
        label: "Bay".into(),
        model: json!({
            "axis": {"startMinute": 0, "endMinute": days * DAY, "days": axis_days, "ticks": ticks,
                     "initialViewport": {"startMinute": 0, "endMinute": shown * DAY}},
            "lanes": lanes,
        }),
    }))
}

/// One item per day: ready units of the fleet, red below a minimum, amber
/// below a target, emerald otherwise.
fn readiness_lane(plan: &Plan, fleet: &Fleet<'_>) -> Value {
    let d = fleet.d;
    let units = d.units.len();
    let mut below = 0;
    let items: Vec<Value> = (0..d.days as usize)
        .map(|day| {
            let ready = fleet.ready_count(day);
            let rules: Vec<String> = fleet
                .by_rule(day)
                .into_iter()
                .map(|(r, ready)| {
                    let rule = &plan.readiness[r];
                    let info = &d.rules[r];
                    format!(
                        "{}: {ready} ready (minimum {}, target {})",
                        rule.name.as_deref().unwrap_or(&rule.id),
                        info.minimum,
                        info.target
                    )
                })
                .collect();
            let tone = if fleet.below_minimum(day) {
                below += 1;
                HARD_TONE
            } else if fleet.below_target(day) {
                "amber"
            } else {
                "emerald"
            };
            let mut meta = vec![format!(
                "{ready} of {units} units ready ({}%)",
                100 * ready / units.max(1)
            )];
            meta.extend(rules);
            json!({
                "id": format!("ready:{day}"),
                "startMinute": day as i64 * DAY,
                "endMinute": (day as i64 + 1) * DAY,
                "label": format!("{ready}/{units}"),
                "meta": meta.join(" · "),
                "tone": tone,
            })
        })
        .collect();
    let average = (0..d.days as usize)
        .map(|day| fleet.ready_count(day))
        .sum::<usize>() as f64
        / (d.days as f64 * units.max(1) as f64);
    let badges: Vec<Value> = if below > 0 {
        vec![
            json!({"label": format!("{below} day{} below minimum", if below == 1 { "" } else { "s" }),
                    "style": {"bg": "rgba(220, 38, 38, 0.12)", "border": "1px solid #dc2626",
                              "color": "#b91c1c"}}),
        ]
    } else {
        Vec::new()
    };
    json!({
        "id": "readiness", "label": "Fleet readiness", "mode": "detailed",
        "badges": badges,
        "stats": [{"label": "Average", "value": format!("{:.0}%", 100.0 * average)}],
        "items": items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn broken(level: Level, constraint: &str, shared: bool) -> Broken {
        Broken {
            level,
            constraint: constraint.into(),
            cost: 1,
            shared,
        }
    }

    #[test]
    fn readiness_alone_does_not_colour_a_task() {
        let day = broken(Level::Hard, "Readiness below minimum", true);
        assert_eq!(tone_of(std::slice::from_ref(&day)), None);
        let crowded = broken(Level::Hard, "Bay over capacity", true);
        assert_eq!(tone_of(&[day, crowded]), Some(HARD_TONE));
        let open = broken(Level::Medium, "Mandatory task unplanned", false);
        assert_eq!(tone_of(&[open]), Some(MEDIUM_TONE));
    }
}
