//! Pick-up and delivery replanning changes: a driver off on a day, a new job
//! from one place to another, a stop's new time window.

use std::collections::BTreeSet;

use serde_json::{json, Value};
use solverforge_workbench_host::edit::{
    by_id, day_label, fresh_id, unknown, Args, Choice, Compiled, Edit, Field, Kind,
};
use solverforge_workbench_host::EngineError;

use crate::domain::Plan;

const MODEL: &str = "pickup-delivery-routing";

fn drivers(plan: &Plan) -> Vec<Choice> {
    plan.drivers
        .iter()
        .map(|d| Choice::new(&d.id, d.name.clone().unwrap_or_else(|| d.id.clone())))
        .collect()
}

fn days(plan: &Plan) -> Vec<Choice> {
    let dates: BTreeSet<&str> = plan
        .drivers
        .iter()
        .flat_map(|d| &d.shifts)
        .filter_map(|s| s.min_start_time.as_deref())
        .filter(|t| t.len() >= 10)
        .map(|t| &t[..10])
        .collect();
    dates
        .into_iter()
        .map(|d| Choice::new(d, day_label(d)))
        .collect()
}

fn stops(plan: &Plan) -> Vec<Choice> {
    plan.jobs
        .iter()
        .flat_map(|job| {
            let job_name = job.name.clone().unwrap_or_else(|| job.id.clone());
            job.stops.iter().map(move |stop| {
                let stop_name = stop.name.clone().unwrap_or_else(|| stop.id.clone());
                Choice::new(&stop.id, format!("{job_name} · {stop_name}"))
            })
        })
        .collect()
}

pub fn edits(plan: &Plan) -> Vec<Edit> {
    vec![
        Edit {
            id: "driver_off",
            label: "A driver is off for a day".into(),
            description: "Removes the driver's shifts that start that day; their jobs are planned again."
                .into(),
            fields: vec![
                Field::choice("driver", "Driver", drivers(plan)),
                Field::choice("day", "Day", days(plan)),
            ],
        },
        Edit {
            id: "add_job",
            label: "Add a job".into(),
            description: "A new job: picked up at one place and dropped off at another, inside a time window."
                .into(),
            fields: vec![
                Field::new("name", "Name", Kind::Text),
                Field::new("pickup", "Pick up at", Kind::Location),
                Field::new("dropoff", "Drop off at", Kind::Location),
                Field::new("stop_minutes", "Minutes at each stop", Kind::Minutes),
                Field::new("from", "Not before", Kind::DateTime),
                Field::new("until", "Delivered by", Kind::DateTime),
            ],
        },
        Edit {
            id: "stop_window",
            label: "Change a stop's time window".into(),
            description: "The stop may only be served inside the new window.".into(),
            fields: vec![
                Field::choice("stop", "Stop", stops(plan)),
                Field::new("from", "Not before", Kind::DateTime),
                Field::new("until", "Done by", Kind::DateTime),
            ],
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

fn when(from: &str, until: &str) -> String {
    format!(
        "between {} and {}",
        from.replace('T', " "),
        until.replace('T', " ")
    )
}

pub fn edit(plan: &Plan, id: &str, raw: &Value) -> Result<Compiled, EngineError> {
    let args = Args::new(id, raw);
    match id {
        "driver_off" => {
            let who = args.choice("driver", &drivers(plan))?;
            let day = args.choice("day", &days(plan))?;
            let driver = plan.drivers.iter().find(|d| d.id == who).expect("offered");
            let name = driver.name.clone().unwrap_or_else(|| driver.id.clone());
            let gone: Vec<&str> = driver
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
            Ok(Compiled {
                operations: gone
                    .iter()
                    .map(|s| {
                        json!({"op": "remove",
                               "path": format!("/drivers/{}/shifts/{}", by_id(who), by_id(s))})
                    })
                    .collect(),
                summary: format!(
                    "{name} is off on {}: shift {} removed; their jobs are planned again.",
                    day_label(day),
                    gone.join(", ")
                ),
            })
        }
        "add_job" => {
            let name = args.text("name")?;
            let (pickup, dropoff) = (args.location("pickup")?, args.location("dropoff")?);
            let minutes = args.minutes("stop_minutes")?;
            let (from, until) = window(&args)?;
            let id = fresh_id("NEW", plan.jobs.iter().map(|j| j.id.as_str()));
            let stop = |suffix: &str, place: [f64; 2], label: &str| {
                json!({"id": format!("{id}-{suffix}"), "name": label, "location": place,
                       "duration": minutes,
                       "timeWindows": [{"minStartTime": from, "maxEndTime": until}]})
            };
            let job = json!({
                "id": id, "name": name,
                "stops": [stop("a", pickup, "Pick-up"), stop("b", dropoff, "Drop-off")],
            });
            Ok(Compiled {
                operations: vec![json!({"op": "add", "path": "/jobs/-", "value": job})],
                summary: format!("New job {name}, {}.", when(&from, &until)),
            })
        }
        "stop_window" => {
            let options = stops(plan);
            let stop = args.choice("stop", &options)?;
            let (from, until) = window(&args)?;
            let job = plan
                .jobs
                .iter()
                .find(|j| j.stops.iter().any(|s| s.id == stop))
                .expect("offered");
            let label = options
                .into_iter()
                .find(|c| c.value == stop)
                .expect("offered")
                .label;
            Ok(Compiled {
                operations: vec![json!({
                    "op": "replace",
                    "path": format!("/jobs/{}/stops/{}/timeWindows", by_id(&job.id), by_id(stop)),
                    "value": [{"minStartTime": from, "maxEndTime": until}],
                })],
                summary: format!("{label} now {}.", when(&from, &until)),
            })
        }
        _ => Err(unknown(MODEL, id, &edits(plan))),
    }
}
