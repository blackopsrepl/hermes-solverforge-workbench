//! The roster view, freezing and fill through the engine.

use serde_json::{json, Value};
use solverforge_workbench_employee_scheduling::EmployeeScheduling;
use solverforge_workbench_host::Model;

use crate::common::{apply, engine};

fn month() -> Value {
    EmployeeScheduling::present(&EmployeeScheduling::demo("LARGE_MONTH").unwrap())
}

/// The month with every shift on the first employee, a quarter left open.
fn crowded() -> Value {
    let mut plan = month();
    let first = plan["employees"][0]["id"].clone();
    for (n, shift) in plan["shifts"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        shift["employee"] = if n % 4 == 3 {
            Value::Null
        } else {
            first.clone()
        };
    }
    plan
}

fn request(command: &str, plan: Value, extra: Value) -> (i32, Value) {
    let mut body = json!({"command": command, "model": "employee-scheduling", "plan": plan});
    for (key, value) in extra.as_object().unwrap() {
        body[key] = value.clone();
    }
    engine(body)
}

#[test]
fn the_roster_has_a_lane_per_employee_and_one_for_open_shifts() {
    let plan = month();
    let (code, answer) = request("view", plan.clone(), json!({}));
    assert_eq!(code, 0);
    let view = &answer["view"];
    assert_eq!(view["kind"], "timeline");
    assert_eq!(view["label"], "Employee");
    let lanes = view["model"]["lanes"].as_array().unwrap();
    let employees = plan["employees"].as_array().unwrap().len();
    assert_eq!(lanes[0]["id"], "open");
    assert_eq!(lanes.len(), employees + 1);
    let items: usize = lanes
        .iter()
        .map(|l| l["items"].as_array().unwrap().len())
        .sum();
    // Every shift is drawn except an alternative nobody works.
    let drawn = plan["shifts"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| !s["employee"].is_null() || s["alternativeOf"].is_null())
        .count();
    assert_eq!(items, drawn);
}

#[test]
fn a_rule_over_an_employees_roster_badges_their_row_and_a_shift_rule_marks_the_shift() {
    let (_, answer) = request("view", crowded(), json!({}));
    let lanes = answer["view"]["model"]["lanes"].as_array().unwrap().clone();
    let first = &lanes[1];
    let badges = first["badges"].as_array().unwrap();
    // Overlapping shifts is a rule over the employee's roster: it badges the row.
    assert!(
        badges.iter().any(|b| b["label"] == "Overlapping shifts"),
        "{badges:?}"
    );
    assert!(badges[0]["style"]["color"] == "#b91c1c");
    // Open mandatory shifts break a medium rule of their own: rose.
    let open = &lanes[0]["items"].as_array().unwrap()[0];
    assert_eq!(open["tone"], "rose");
    assert!(open["meta"].as_str().unwrap().contains("breaks: "));
}

#[test]
fn freezing_pins_every_shift_not_yet_pinned_that_starts_before_the_moment() {
    let plan = crowded();
    let before = plan["shifts"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["start"].as_str().unwrap() < "2027-03-03T00:00")
        .filter(|s| s["pinned"] != true)
        .count();
    assert!(before > 0);
    let (code, answer) = request(
        "evaluate",
        plan,
        json!({"freeze_before": "2027-03-03T00:00"}),
    );
    assert_eq!(code, 0);
    let issue = answer["issues"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["code"] == "frozen")
        .expect("a frozen notice");
    assert!(
        issue["message"]
            .as_str()
            .unwrap()
            .starts_with(&format!("{before} item(s)")),
        "{issue}"
    );
}

#[test]
fn freezing_refuses_a_moment_that_is_not_a_date_time() {
    let (code, answer) = request("evaluate", month(), json!({"freeze_before": "tomorrow"}));
    assert_eq!(code, 1);
    assert_eq!(answer["error"]["code"], "invalid_freeze");
}

#[test]
fn fill_ranks_employees_for_an_open_shift_by_the_score_they_reach() {
    let plan = crowded();
    let overloaded = plan["employees"][0]["id"].clone();
    let open = plan["shifts"][3]["id"].as_str().unwrap().to_string();
    let (code, answer) = request(
        "fill",
        plan,
        json!({"fill": {"items": [open.clone()], "top": 3}}),
    );
    assert_eq!(code, 0, "{answer}");
    let items = answer["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], open.as_str());
    let options = items[0]["options"].as_array().unwrap();
    assert_eq!(options.len(), 3);
    // Each option names an employee and the constraints its placement moves.
    assert!(options
        .iter()
        .all(|o| o["label"].as_str().is_some_and(|l| !l.is_empty())));
    assert!(!options[0]["changes"].as_array().unwrap().is_empty());
    // The employee already working every other shift is never the best place.
    assert_ne!(options[0]["id"], overloaded);
}

#[test]
fn fill_without_items_offers_every_open_shift() {
    let plan = crowded();
    let open = plan["shifts"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["employee"].is_null())
        .count();
    let (code, answer) = request("fill", plan, json!({"fill": {"top": 1}}));
    assert_eq!(code, 0);
    assert_eq!(answer["items"].as_array().unwrap().len(), open);
}

#[test]
fn fill_applies_an_option_to_the_plan() {
    let plan = crowded();
    let open = plan["shifts"][3]["id"].clone();
    let employee = plan["employees"][1]["id"].clone();
    let (code, answer) = request(
        "fill",
        plan,
        json!({"fill": {"apply": {"item": open, "value": employee}}}),
    );
    assert_eq!(code, 0, "{answer}");
    assert_eq!(answer["plan"]["shifts"][3]["employee"], employee);
}

#[test]
fn replanning_changes_compile_to_patches_the_engine_accepts() {
    let plan = month();
    let (code, out) =
        engine(json!({"command": "edits", "model": "employee-scheduling", "plan": plan}));
    assert_eq!(code, 0, "{out}");
    let ids: Vec<&str> = out["edits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["employee_off", "shift_cancelled"]);
    let compile = |id: &str, args: Value| {
        let (code, out) = engine(
            json!({"command": "edit", "model": "employee-scheduling", "plan": plan, "edit": {"id": id, "args": args}}),
        );
        assert_eq!(code, 0, "{out}");
        out
    };
    let off = compile(
        "employee_off",
        json!({"employee": "e01", "day": "2027-03-01"}),
    );
    assert_eq!(
        off["operations"],
        json!([
            {"op": "add", "path": "/employees/[id=e01]/unavailable/-",
             "value": {"from": "2027-03-01T00:00", "to": "2027-03-02T00:00"}},
            {"op": "replace", "path": "/shifts/[id=2027-03-01-ward-a-early-0]/pinned", "value": false},
            {"op": "remove", "path": "/shifts/[id=2027-03-01-ward-a-early-0]/employee"},
        ])
    );
    assert_eq!(
        off["summary"],
        "Ada North is off on Mon 1 Mar: 1 shift is free to cover."
    );
    let cancelled = compile(
        "shift_cancelled",
        json!({"shift": "2027-03-28-ward-c-night-0"}),
    );
    assert_eq!(
        cancelled["summary"],
        "The shift Sun 28 Mar 23:00–07:00 night, ward-c, weekend is cancelled."
    );
    let mut changed = plan.clone();
    for compiled in [&off, &cancelled] {
        apply(&mut changed, compiled["operations"].as_array().unwrap());
    }
    let (code, out) =
        engine(json!({"command": "evaluate", "model": "employee-scheduling", "plan": changed}));
    assert_eq!(code, 0, "{out}");
}

#[test]
fn the_data_tables_match_the_demo() {
    // Every field of the demo has a column, and every column is a field the
    // model reads and writes back: the data editor can change all of it.
    let demo = month();
    let tables = EmployeeScheduling::tables();
    let mut problems = solverforge_workbench_host::data::mismatches(&demo, &tables, &[]);
    problems.extend(solverforge_workbench_host::data::unread(
        &demo,
        &tables,
        |plan| {
            serde_json::from_value::<solverforge_workbench_employee_scheduling::Plan>(plan)
                .map(|plan| EmployeeScheduling::present(&plan))
                .map_err(|error| error.to_string())
        },
    ));
    assert!(problems.is_empty(), "{problems:#?}");
}
