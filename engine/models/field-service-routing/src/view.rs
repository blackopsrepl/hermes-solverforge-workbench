//! The plan as routes on a map, with the same plan as a timeline: one lane
//! per shift, unplanned visits in a lane on top.

use serde_json::{json, Value};
use solverforge_workbench_host::view::{
    blame, Broken, Point, Route as MapRoute, Routes, Stop as MapStop, Timeline, View, HARD_TONE,
    MEDIUM_TONE,
};
use solverforge_workbench_host::{Analysis, Level};

use crate::domain::Plan;
use crate::names::label;
use crate::present::timed;
use crate::route::routes;
use crate::time::{axis_labels, clock, duration_label, format_date_time, DAY};

/// Route colours on the map, one per shift in turn.
const COLORS: [&str; 10] = [
    "#2563eb", "#059669", "#d97706", "#7c3aed", "#0891b2", "#db2777", "#65a30d", "#ea580c",
    "#4f46e5", "#0d9488",
];
const VISIT_TONE: &str = "blue";
const JOINT_TONE: &str = "violet";
const BREAK_TONE: &str = "slate";
const MAX_LANE_BADGES: usize = 4;

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

/// Outside the shift's hours is grey, so red and rose only mean a broken rule.
fn off_hours() -> Value {
    json!({"background": "rgba(75, 85, 99, 0.16)", "border": "#6b7280",
           "overlay": "rgba(75, 85, 99, 0.16)", "text": "#374151"})
}

pub fn view(plan: &Plan, analysis: &Analysis) -> Option<View> {
    if plan.facts.shifts.is_empty() {
        return None;
    }
    let timed = timed(plan);
    let plan = &timed;
    let facts = &*plan.facts;
    let routes = routes(plan);
    let broken = blame(analysis);
    let none: Vec<Broken> = Vec::new();
    let broken_of = |type_name: &str, id: &str| {
        broken
            .get(&(type_name.to_string(), id.to_string()))
            .unwrap_or(&none)
    };
    let real = &facts.shifts[..facts.bench()];
    let first = real.iter().map(|s| s.min_start).min()?;
    let day0 = first.div_euclid(DAY) * DAY;
    let last = routes
        .iter()
        .map(|r| r.end_arrival)
        .chain(real.iter().filter_map(|s| s.max_end))
        .max()
        .unwrap_or(first)
        .max(first + 1);
    let span_days = ((last - day0 + DAY - 1) / DAY).max(1);
    let minute = |t: i64| (t - day0).div_euclid(60);
    let place = |p: usize| facts.travel.places[p];

    let stop_role = |v: usize| {
        let fact = &facts.visits[v];
        match (fact.is_break(), fact.group) {
            (true, _) => BREAK_TONE,
            (false, Some(_)) => JOINT_TONE,
            (false, None) => VISIT_TONE,
        }
    };

    let mut vehicles = Vec::new();
    let mut lanes = Vec::new();
    let unplanned: Vec<usize> = (0..facts.first_break)
        .filter(|&v| !plan.elements[v].planned())
        .collect();
    let mut unassigned = Vec::new();
    if !unplanned.is_empty() {
        let mut items = Vec::new();
        for &v in &unplanned {
            let vf = &facts.visits[v];
            let b = broken_of("Visit", &vf.id);
            let start = vf.windows.first().map_or(first, |w| w.min_start).max(day0);
            let mut notes = vec!["not planned".to_string()];
            if !b.is_empty() {
                notes.push(format!("breaks: {}", rules(b)));
            }
            items.push(json!({
                "id": vf.id,
                "startMinute": minute(start),
                "endMinute": minute(start + vf.service.max(60)),
                "label": label(plan, "Visit", v),
                "meta": notes.join(" · "),
                "tone": tone(b).unwrap_or_else(|| stop_role(v)),
            }));
            if let Some(p) = vf.location {
                let [lat, lon] = place(p);
                unassigned.push(MapStop {
                    id: vf.id.clone(),
                    label: label(plan, "Visit", v),
                    lat,
                    lon,
                    arrival: None,
                    departure: None,
                    broken: b.clone(),
                });
            }
        }
        lanes.push(json!({
            "id": "unplanned", "label": "Unplanned", "mode": "detailed",
            "badges": [{"label": format!("{} visits", unplanned.len())}],
            "stats": [{"label": "Visits", "value": unplanned.len()}],
            "overlays": [],
            "items": items,
        }));
    }

    for (s, fact) in real.iter().enumerate() {
        let route = &routes[s];
        let list = &plan.shifts[s].visits;
        let color = COLORS[s % COLORS.len()].to_string();
        let mut items = Vec::new();
        let mut stops = Vec::new();
        let mut served = 0usize;
        for (position, &v) in list.iter().enumerate() {
            let vf = &facts.visits[v];
            let drawn = &route.elements[position];
            let planned = &drawn.at;
            let b = broken_of("Visit", &vf.id);
            let mut notes = vec![if drawn.untimed {
                "cannot be timed".to_string()
            } else {
                format!("{}–{}", clock(planned.start), clock(planned.end))
            }];
            if planned.wait > 0 {
                notes.push(format!("waits {}", duration_label(planned.wait)));
            }
            if planned.travel > 0 {
                notes.push(format!(
                    "{} drive, {:.1} km",
                    duration_label(planned.travel),
                    planned.distance as f64 / 1000.0
                ));
            }
            if vf.pin.is_some() {
                notes.push("pinned".into());
            }
            if !b.is_empty() {
                notes.push(format!("breaks: {}", rules(b)));
            }
            items.push(json!({
                "id": vf.id,
                "startMinute": minute(planned.start),
                "endMinute": minute(planned.end.max(planned.start + 60)),
                "label": format!("{}{}", if vf.pin.is_some() { "📌 " } else { "" }, label(plan, "Visit", v)),
                "meta": notes.join(" · "),
                "tone": tone(b).unwrap_or_else(|| stop_role(v)),
            }));
            if !vf.is_break() {
                served += 1;
            }
            if let Some(p) = vf.location {
                let [lat, lon] = place(p);
                stops.push(MapStop {
                    id: vf.id.clone(),
                    label: label(plan, "Visit", v),
                    lat,
                    lon,
                    arrival: Some(format_date_time(planned.arrival)),
                    departure: Some(format_date_time(planned.end)),
                    broken: b.clone(),
                });
            }
        }
        let own = broken_of("Shift", &fact.id);
        let mut badges: Vec<Value> = Vec::new();
        let mut seen: Vec<&str> = Vec::new();
        for b in own.iter().filter(|b| b.level != Level::Soft) {
            if !seen.contains(&b.constraint.as_str()) {
                seen.push(&b.constraint);
                badges.push(json!({"label": b.constraint, "style": broken_badge(b.level)}));
            }
        }
        if fact.linked() {
            badges.push(json!({"label": format!("{} days linked", fact.components.len())}));
        }
        for tag in fact.tags.iter().take(MAX_LANE_BADGES) {
            badges.push(json!({"label": facts.tags[*tag]}));
        }
        badges.truncate(MAX_LANE_BADGES);
        let mut overlays = vec![json!({
            "startMinute": minute(day0.max(fact.min_start - 12 * 3600)).max(0),
            "endMinute": minute(fact.min_start),
            "label": format!("Starts {}", clock(fact.min_start)),
            "tone": off_hours(),
        })];
        if let Some(end) = fact.max_end {
            overlays.push(json!({
                "startMinute": minute(end),
                "endMinute": minute(end + 12 * 3600).min(span_days * DAY / 60),
                "label": format!("Ends {}", clock(end)),
                "tone": off_hours(),
            }));
        }
        lanes.push(json!({
            "id": format!("shift:{}", fact.id),
            "label": fact.label,
            "mode": "detailed",
            "badges": badges,
            "stats": [
                {"label": "Visits", "value": served},
                {"label": "Drive h", "value": (route.travel as f64 / 360.0).round() / 10.0},
                {"label": "km", "value": (route.distance as f64 / 100.0).round() / 10.0},
            ],
            "overlays": overlays,
            "items": items,
        }));
        if route.active {
            let [slat, slon] = place(fact.start_location);
            let [elat, elon] = place(fact.end_location);
            vehicles.push(MapRoute {
                id: fact.id.clone(),
                label: fact.label.clone(),
                color,
                start: Point {
                    label: format!("Start · {}", clock(route.start)),
                    lat: slat,
                    lon: slon,
                },
                end: Point {
                    label: format!("End · {}", clock(route.end_arrival)),
                    lat: elat,
                    lon: elon,
                },
                stops,
                path: String::new(),
                stats: vec![
                    format!("{served} visits"),
                    format!("{:.1} km", route.distance as f64 / 1000.0),
                    format!("{} driving", duration_label(route.travel)),
                ],
            });
        }
    }

    let days: Vec<Value> = (0..span_days)
        .map(|n| {
            let date = day0.div_euclid(DAY) + n;
            let (weekday, sub) = axis_labels(date);
            json!({"label": weekday, "subLabel": sub,
                   "isWeekend": matches!(weekday, "Sat" | "Sun")})
        })
        .collect();
    let day_minutes = DAY / 60;
    let ticks: Vec<Value> = (0..=span_days * day_minutes)
        .step_by(120)
        .map(|m| json!({"minute": m, "label": format!("{:02}:00", (m % day_minutes) / 60)}))
        .collect();
    let start_view = minute(first).max(0);
    let planned_visits = facts.first_break - unplanned.len();
    let subtitle = format!(
        "{planned_visits} visits planned · {} unplanned · {} of {} shifts driving · blue visit, \
         violet multi-technician visit, slate break · red: breaks a hard rule · rose: a medium \
         one · grey: off hours",
        unplanned.len(),
        vehicles.len(),
        real.len()
    );
    let timeline = Timeline {
        title: "Routes by shift".into(),
        subtitle: subtitle.clone(),
        label: "Shift".into(),
        model: json!({
            "axis": {"startMinute": 0, "endMinute": span_days * day_minutes, "days": days,
                     "ticks": ticks,
                     "initialViewport": {"startMinute": start_view,
                                         "endMinute": (start_view + 14 * 60).min(span_days * day_minutes)}},
            "lanes": lanes,
        }),
    };
    Some(View::Routes(Routes {
        title: "Routes".into(),
        subtitle,
        vehicles,
        unassigned,
        timeline,
    }))
}
