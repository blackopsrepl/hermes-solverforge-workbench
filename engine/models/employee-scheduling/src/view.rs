//! The roster: one lane per employee, open shifts on top, time off as overlays.
//!
//! A hard or medium match that names an employee comes from a rule over that
//! employee's whole roster, so it badges the employee's row rather than every
//! shift they work; a match without an employee marks the shifts it names.

use std::collections::{BTreeMap, HashMap};

use serde_json::{json, Value};
use solverforge_workbench_host::view::{level_cost, Timeline, View, HARD_TONE, MEDIUM_TONE};
use solverforge_workbench_host::{Analysis, Level};

use crate::domain::{Plan, Shift};
use crate::time::{civil_from_days, parse_date_time, weekday, DAY};
use crate::wire::{SkillRef, TimeSpan, Weekday};

const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
/// Tones for the first tag of a shift; other tags take the spare tones in
/// order of appearance.
const TAG_TONES: [(&str, &str); 6] = [
    ("early", "amber"),
    ("day", "amber"),
    ("morning", "amber"),
    ("late", "blue"),
    ("evening", "blue"),
    ("night", "violet"),
];
const SPARE_TONES: [&str; 6] = ["cyan", "emerald", "slate", "blue", "amber", "violet"];
const MAX_LANE_RULES: usize = 2;
const MAX_LANE_BADGES: usize = 4;

/// Time off is grey, so red and rose on the roster only ever mean a broken rule.
fn unavailable() -> Value {
    json!({"background": "rgba(75, 85, 99, 0.16)", "border": "#6b7280",
           "overlay": "rgba(75, 85, 99, 0.16)", "text": "#374151"})
}

fn rather_not() -> Value {
    json!({"background": "rgba(156, 163, 175, 0.12)", "border": "#9ca3af",
           "overlay": "rgba(156, 163, 175, 0.12)", "text": "#4b5563"})
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

/// Rules broken, by shift id and by employee id: `(level, constraint, cost)`.
#[derive(Default)]
struct Flags {
    shifts: HashMap<String, Vec<(Level, String)>>,
    employees: HashMap<String, Vec<(Level, String, i64)>>,
}

fn flags(analysis: &Analysis) -> Flags {
    let mut out = Flags::default();
    for constraint in &analysis.constraints {
        if constraint.level == Level::Soft {
            continue;
        }
        for found in &constraint.matches {
            let cost = level_cost(&found.score, constraint.level);
            if cost <= 0 {
                continue;
            }
            let ids = |type_name: &str| -> Vec<String> {
                found
                    .entities
                    .iter()
                    .filter(|e| e.type_name == type_name)
                    .filter_map(|e| e.name.id.clone())
                    .collect()
            };
            let employees = ids("Employee");
            if !employees.is_empty() {
                for id in employees {
                    out.employees.entry(id).or_default().push((
                        constraint.level,
                        constraint.name.clone(),
                        cost,
                    ));
                }
                continue;
            }
            for id in ids("Shift") {
                out.shifts
                    .entry(id)
                    .or_default()
                    .push((constraint.level, constraint.name.clone()));
            }
        }
    }
    out
}

/// Badges naming the rules an employee's roster breaks: hard first, then costliest.
fn rule_badges(broken: &[(Level, String, i64)]) -> Vec<Value> {
    let mut by_rule: BTreeMap<&str, (Level, i64)> = BTreeMap::new();
    for (level, name, cost) in broken {
        let entry = by_rule.entry(name.as_str()).or_insert((*level, 0));
        entry.0 = *level;
        entry.1 += cost;
    }
    let mut names: Vec<(&str, Level, i64)> = by_rule
        .into_iter()
        .map(|(name, (level, cost))| (name, level, cost))
        .collect();
    names.sort_by_key(|(name, level, cost)| (*level != Level::Hard, -cost, *name));
    let mut badges: Vec<Value> = names
        .iter()
        .take(MAX_LANE_RULES)
        .map(|(name, level, _)| json!({"label": name, "style": broken_badge(*level)}))
        .collect();
    if names.len() > MAX_LANE_RULES {
        let rest = &names[MAX_LANE_RULES..];
        let worst = if rest.iter().any(|(_, level, _)| *level == Level::Hard) {
            Level::Hard
        } else {
            Level::Medium
        };
        badges
            .push(json!({"label": format!("+{} more", rest.len()), "style": broken_badge(worst)}));
    }
    badges
}

fn clock(minute: i64) -> String {
    let rest = minute.rem_euclid(DAY);
    format!("{:02}:{:02}", rest / 60, rest % 60)
}

struct Tones(Vec<(String, &'static str)>);

impl Tones {
    fn of(&mut self, tags: &[String]) -> &'static str {
        let key = tags.first().map_or("shift", String::as_str);
        if let Some((_, tone)) = self.0.iter().find(|(tag, _)| tag == key) {
            return tone;
        }
        let tone = SPARE_TONES[(self.0.len() - TAG_TONES.len()) % SPARE_TONES.len()];
        self.0.push((key.to_string(), tone));
        tone
    }
}

pub fn view(plan: &Plan, analysis: &Analysis) -> Option<View> {
    let start = parse_date_time(&plan.schedule.start)?;
    let end = parse_date_time(&plan.schedule.end)?;
    let day0 = start.div_euclid(DAY) * DAY;
    let span_days = ((end - day0 + DAY - 1) / DAY).max(1);
    let weekend: Vec<Weekday> = plan.schedule.weekend.clone();
    let flags = flags(analysis);
    let mut tones = Tones(
        TAG_TONES
            .iter()
            .map(|(tag, tone)| (tag.to_string(), *tone))
            .collect(),
    );

    let mut item = |shift: &Shift| -> Option<Value> {
        let begin = parse_date_time(&shift.start)? - day0;
        let finish = parse_date_time(&shift.end)? - day0;
        let broken = flags.shifts.get(&shift.id).map_or(&[][..], Vec::as_slice);
        let mut notes = vec![format!("{}–{}", clock(begin), clock(finish))];
        if !shift.tags.is_empty() {
            notes.push(shift.tags.join(", "));
        }
        if shift.pinned {
            notes.push("pinned".into());
        }
        if shift.optional {
            notes.push("optional".into());
        }
        if !broken.is_empty() {
            let mut names: Vec<&str> = broken.iter().map(|(_, n)| n.as_str()).collect();
            names.sort_unstable();
            names.dedup();
            notes.push(format!("breaks: {}", names.join("; ")));
        }
        let tone = if broken.iter().any(|(level, _)| *level == Level::Hard) {
            HARD_TONE
        } else if !broken.is_empty() {
            MEDIUM_TONE
        } else {
            tones.of(&shift.tags)
        };
        Some(json!({
            "id": shift.id,
            "startMinute": begin,
            "endMinute": finish,
            "label": format!("{}{}", if shift.pinned { "📌 " } else { "" },
                             shift.tags.first().map_or("shift", String::as_str)),
            "meta": notes.join(" · "),
            "tone": tone,
        }))
    };

    let mut by_employee: HashMap<usize, Vec<&Shift>> = HashMap::new();
    let mut open: Vec<&Shift> = Vec::new();
    for shift in &plan.shifts {
        match shift.employee_idx {
            Some(employee) => by_employee.entry(employee).or_default().push(shift),
            // An unchosen alternative is not an open shift.
            None if shift.alternative_of.is_none() => open.push(shift),
            None => {}
        }
    }
    let by_start = |shifts: &mut Vec<&Shift>| shifts.sort_by(|a, b| a.start.cmp(&b.start));

    let mut lanes = Vec::new();
    if !open.is_empty() {
        by_start(&mut open);
        let mandatory = open.iter().filter(|s| !s.optional).count();
        let badges: Vec<Value> = if mandatory > 0 {
            vec![json!({"label": format!("{mandatory} mandatory")})]
        } else {
            Vec::new()
        };
        lanes.push(json!({
            "id": "open", "label": "Open shifts", "mode": "detailed",
            "badges": badges,
            "stats": [{"label": "Shifts", "value": open.len()}],
            "items": open.iter().filter_map(|s| item(s)).collect::<Vec<_>>(),
        }));
    }
    for (index, employee) in plan.employees.iter().enumerate() {
        let mut mine = by_employee.remove(&index).unwrap_or_default();
        by_start(&mut mine);
        let items: Vec<Value> = mine.iter().filter_map(|s| item(s)).collect();
        let minutes: i64 = items
            .iter()
            .map(|i| i["endMinute"].as_i64().unwrap_or(0) - i["startMinute"].as_i64().unwrap_or(0))
            .sum();
        let mut badges = rule_badges(
            flags
                .employees
                .get(&employee.id)
                .map_or(&[][..], Vec::as_slice),
        );
        badges.extend(employee.skills.iter().map(|skill| {
            json!({"label": match skill { SkillRef::Id(id) => id.clone(), SkillRef::Detailed(d) => d.id.clone() }})
        }));
        badges.truncate(MAX_LANE_BADGES);
        lanes.push(json!({
            "id": format!("employee:{}", employee.id),
            "label": employee.name.clone().unwrap_or_else(|| employee.id.clone()),
            "mode": "detailed",
            "badges": badges,
            "stats": [{"label": "Shifts", "value": items.len()},
                      {"label": "Hours", "value": (minutes as f64 / 6.0).round() / 10.0}],
            "overlays": overlays(&employee.unavailable, &employee.unwanted_times,
                                 &employee.preferred_times, day0),
            "items": items,
        }));
    }

    let days: Vec<Value> = (0..span_days)
        .map(|n| {
            let date = day0.div_euclid(DAY) + n;
            let (_, month, day) = civil_from_days(date);
            let wd = weekday(date) as usize;
            json!({"label": WEEKDAYS[wd], "subLabel": format!("{day} {}", MONTHS[(month - 1) as usize]),
                   "isWeekend": weekend.iter().any(|w| w.number() as usize == wd)})
        })
        .collect();
    let ticks: Vec<Value> = (0..=span_days * DAY)
        .step_by(360)
        .map(|m| json!({"minute": m, "label": format!("{:02}:00", (m % DAY) / 60)}))
        .collect();
    let shown = if span_days > 14 { 7 } else { span_days };
    let assigned = plan.shifts.len();
    Some(View::Timeline(Timeline {
        title: "Roster".into(),
        subtitle: format!(
            "{assigned} shifts · {} employees · {} open · red: breaks a hard rule, on a shift or an \
             employee's row · rose: a medium one · grey: time off",
            plan.employees.len(),
            open.len()
        ),
        label: "Employee".into(),
        model: json!({
            "axis": {"startMinute": 0, "endMinute": span_days * DAY, "days": days, "ticks": ticks,
                     "initialViewport": {"startMinute": 0, "endMinute": shown * DAY}},
            "lanes": lanes,
        }),
    }))
}

fn overlays(
    unavailable_spans: &[TimeSpan],
    unwanted: &[TimeSpan],
    preferred: &[TimeSpan],
    day0: i64,
) -> Vec<Value> {
    let mut out = Vec::new();
    for (spans, label, tone) in [
        (unavailable_spans, "Unavailable", unavailable()),
        (unwanted, "Rather not", rather_not()),
        (preferred, "Preferred", json!("emerald")),
    ] {
        for span in spans {
            let (Some(from), Some(to)) = (parse_date_time(&span.from), parse_date_time(&span.to))
            else {
                continue;
            };
            if to > from {
                out.push(json!({"startMinute": from - day0, "endMinute": to - day0,
                                "label": label, "tone": tone}));
            }
        }
    }
    out
}
