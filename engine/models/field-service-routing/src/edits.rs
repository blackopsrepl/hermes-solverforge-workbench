//! Field service replanning changes: a technician off on a day, a new visit,
//! a visit's new time window.

use std::collections::BTreeSet;

use serde_json::{json, Value};
use solverforge_workbench_host::edit::{
    by_id, day_label, fresh_id, unknown, Args, Choice, Compiled, Edit, Field, Kind,
};
use solverforge_workbench_host::EngineError;

use crate::domain::Plan;

const MODEL: &str = "field-service-routing";

fn technicians(plan: &Plan) -> Vec<Choice> {
    plan.vehicles
        .iter()
        .map(|v| Choice::new(&v.id, v.name.clone().unwrap_or_else(|| v.id.clone())))
        .collect()
}

fn days(plan: &Plan) -> Vec<Choice> {
    let dates: BTreeSet<&str> = plan
        .vehicles
        .iter()
        .flat_map(|v| &v.shifts)
        .filter_map(|s| s.min_start_time.as_deref())
        .filter(|t| t.len() >= 10)
        .map(|t| &t[..10])
        .collect();
    dates
        .into_iter()
        .map(|d| Choice::new(d, day_label(d)))
        .collect()
}

fn visits(plan: &Plan) -> Vec<Choice> {
    plan.visits
        .iter()
        .map(|v| Choice::new(&v.id, v.name.clone().unwrap_or_else(|| v.id.clone())))
        .collect()
}

fn skills(plan: &Plan) -> Vec<Choice> {
    let names: BTreeSet<&str> = plan
        .vehicles
        .iter()
        .flat_map(|v| &v.shifts)
        .flat_map(|s| &s.limits.skills)
        .map(|s| s.name.as_str())
        .collect();
    names.into_iter().map(|n| Choice::new(n, n)).collect()
}

fn window_fields() -> [Field; 2] {
    [
        Field::new("from", "Not before", Kind::DateTime),
        Field::new("until", "Done by", Kind::DateTime),
    ]
}

pub fn edits(plan: &Plan) -> Vec<Edit> {
    let [from, until] = window_fields();
    vec![
        Edit {
            id: "technician_off",
            label: "A technician is off for a day".into(),
            description: "Removes the technician's shifts that start that day, and any trip that \
                          includes them; their visits are planned again."
                .into(),
            fields: vec![
                Field::choice("technician", "Technician", technicians(plan)),
                Field::choice("day", "Day", days(plan)),
            ],
        },
        Edit {
            id: "add_visit",
            label: "Add a visit".into(),
            description: "A new visit to plan, with its place, length and time window.".into(),
            fields: vec![
                Field::new("name", "Name", Kind::Text),
                Field::new("location", "Place", Kind::Location),
                Field::new("duration", "Length", Kind::Minutes),
                from.clone(),
                until.clone(),
                Field::choice("skill", "Needs skill", skills(plan)).optional(),
            ],
        },
        Edit {
            id: "visit_window",
            label: "Change a visit's time window".into(),
            description: "The visit may only be served inside the new window.".into(),
            fields: vec![Field::choice("visit", "Visit", visits(plan)), from, until],
        },
    ]
}

fn window(args: &Args) -> Result<(String, String), EngineError> {
    let (from, until) = (args.date_time("from")?, args.date_time("until")?);
    if from >= until {
        return Err(EngineError::new(
            "invalid_edit",
            "the window must end after it starts",
        ));
    }
    Ok((from, until))
}

pub fn edit(plan: &Plan, id: &str, raw: &Value) -> Result<Compiled, EngineError> {
    let args = Args::new(id, raw);
    match id {
        "technician_off" => {
            let who = args.choice("technician", &technicians(plan))?;
            let day = args.choice("day", &days(plan))?;
            let vehicle = plan.vehicles.iter().find(|v| v.id == who).expect("offered");
            let name = vehicle.name.clone().unwrap_or_else(|| vehicle.id.clone());
            let gone: Vec<&str> = vehicle
                .shifts
                .iter()
                .filter(|s| {
                    s.min_start_time
                        .as_deref()
                        .is_some_and(|t| t.starts_with(day))
                })
                .map(|s| s.id.as_str())
                .collect();
            if gone.is_empty() {
                return Err(EngineError::new(
                    "invalid_edit",
                    format!("{name} has no shift on {}", day_label(day)),
                ));
            }
            let trips: Vec<&str> = vehicle
                .linked_shifts
                .iter()
                .filter(|l| {
                    l.shifts.iter().any(|link| {
                        gone.contains(&link.from_shift_id.as_str())
                            || gone.contains(&link.to_shift_id.as_str())
                    })
                })
                .map(|l| l.id.as_str())
                .collect();
            let base = format!("/vehicles/{}", by_id(who));
            let operations = trips
                .iter()
                .map(|t| json!({"op": "remove", "path": format!("{base}/linkedShifts/{}", by_id(t))}))
                .chain(
                    gone.iter()
                        .map(|s| json!({"op": "remove", "path": format!("{base}/shifts/{}", by_id(s))})),
                )
                .collect();
            let trip_words = if trips.is_empty() {
                String::new()
            } else {
                format!(" and the trip {}", trips.join(", "))
            };
            Ok(Compiled {
                operations,
                summary: format!(
                    "{name} is off on {}: shift {}{trip_words} removed; their visits are planned again.",
                    day_label(day),
                    gone.join(", ")
                ),
            })
        }
        "add_visit" => {
            let name = args.text("name")?;
            let location = args.location("location")?;
            let duration = args.minutes("duration")?;
            let (from, until) = window(&args)?;
            let skill = match args.text_opt("skill") {
                Some(_) => Some(args.choice("skill", &skills(plan))?),
                None => None,
            };
            let taken = plan
                .visits
                .iter()
                .chain(plan.visit_groups.iter().flat_map(|g| &g.visits))
                .map(|v| v.id.as_str());
            let id = fresh_id("NEW", taken);
            let mut visit = json!({
                "id": id, "name": name, "location": location, "serviceDuration": duration,
                "timeWindows": [{"minStartTime": from, "maxEndTime": until}],
            });
            if let Some(skill) = skill {
                visit["requiredSkills"] = json!([{"name": skill}]);
            }
            Ok(Compiled {
                operations: vec![json!({"op": "add", "path": "/visits/-", "value": visit})],
                summary: format!(
                    "New visit {name} ({duration} min) between {} and {}.",
                    from.replace('T', " "),
                    until.replace('T', " ")
                ),
            })
        }
        "visit_window" => {
            let visit = args.choice("visit", &visits(plan))?;
            let (from, until) = window(&args)?;
            let label = visits(plan)
                .into_iter()
                .find(|c| c.value == visit)
                .map(|c| c.label)
                .unwrap_or_default();
            Ok(Compiled {
                operations: vec![json!({
                    "op": "replace",
                    "path": format!("/visits/{}/timeWindows", by_id(visit)),
                    "value": [{"minStartTime": from, "maxEndTime": until}],
                })],
                summary: format!(
                    "{label} now between {} and {}.",
                    from.replace('T', " "),
                    until.replace('T', " ")
                ),
            })
        }
        _ => Err(unknown(MODEL, id, &edits(plan))),
    }
}
