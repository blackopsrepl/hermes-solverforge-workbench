//! The model through the engine: demos, validation, incremental scoring,
//! names, fill, the view, freezing and parameters.

use std::collections::HashSet;

use serde_json::{json, Value};
use solverforge_workbench_host::Model;
use solverforge_workbench_task_scheduling::TaskScheduling;

use crate::common::{
    apply, assert_construction_matches_full, assert_incremental_matches_full, at, dataset,
    demo_plan, engine, evaluate, issue_codes, machine, scores, task,
};

#[test]
fn every_demo_reads_back_as_it_was_written() {
    for id in TaskScheduling::demos() {
        let wire = TaskScheduling::present(&TaskScheduling::demo(id).unwrap());
        let again = TaskScheduling::present(&demo_plan(id));
        assert_eq!(wire, again, "demo {id}");
    }
}

#[test]
fn the_catalog_lists_every_rule_once_with_nine_hard_one_medium_and_seven_soft() {
    let catalog = TaskScheduling::catalog();
    let names: HashSet<&str> = catalog.iter().map(|c| c.name).collect();
    assert_eq!(names.len(), 17);
    let count = |level| catalog.iter().filter(|c| c.level == level).count();
    use solverforge_workbench_host::Level;
    assert_eq!(
        (count(Level::Hard), count(Level::Medium), count(Level::Soft)),
        (9, 1, 7)
    );
    assert!(catalog
        .iter()
        .all(|c| !c.group.is_empty() && !c.description.is_empty()));
}

#[test]
fn bad_records_are_refused_with_the_reason() {
    let codes = issue_codes(dataset(json!({
        "tasks": [task("A", 30, json!({"dependencies": [{"task": "Z"}]})), task("A", 30, json!({}))],
        "resources": [machine("M1", &[], json!({}))],
    })));
    assert!(
        codes.contains(&"unknown_reference".to_string()),
        "{codes:?}"
    );
    assert!(codes.contains(&"duplicate_id".to_string()), "{codes:?}");
    let cycle = issue_codes(dataset(json!({
        "tasks": [task("A", 30, json!({"dependencies": [{"task": "B"}]})),
                  task("B", 30, json!({"dependencies": [{"task": "A"}]}))],
        "resources": [machine("M1", &[], json!({}))],
    })));
    assert!(cycle.contains(&"dependency_cycle".to_string()), "{cycle:?}");
    let window = issue_codes(dataset(json!({
        "tasks": [task("A", 30, json!({"minStart": at(12, 0), "maxEnd": at(12, 10)}))],
        "resources": [machine("M1", &[], json!({}))],
    })));
    assert!(
        window.contains(&"impossible_window".to_string()),
        "{window:?}"
    );
}

#[test]
fn construction_scores_match_full_ones_on_the_demo() {
    assert_construction_matches_full(demo_plan("FACTORY"), 3);
}

#[test]
fn incremental_scores_match_full_ones_on_the_demo() {
    assert_incremental_matches_full(demo_plan("FACTORY"), 3000, 11);
    assert_incremental_matches_full(demo_plan("FACTORY"), 3000, 5);
}

/// The factory demo with tasks crowded onto the first resources, so most
/// rules have matches.
fn crowded() -> Value {
    let mut plan = TaskScheduling::present(&TaskScheduling::demo("FACTORY").unwrap());
    let pinned: HashSet<String> = plan["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["pinned"] == true)
        .map(|t| t["id"].as_str().unwrap().to_string())
        .collect();
    let ids: Vec<Value> = plan["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["pinned"] != true)
        .map(|t| t["id"].clone())
        .collect();
    let resources = plan["resources"].as_array_mut().unwrap();
    // Pinned tasks stay first where they are; everything else crowds onto two resources.
    for resource in resources.iter_mut() {
        let kept: Vec<Value> = resource["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|a| pinned.contains(a["task"].as_str().unwrap()))
            .cloned()
            .collect();
        resource["tasks"] = json!(kept);
    }
    for (n, id) in ids.iter().enumerate() {
        if n % 5 == 4 {
            continue;
        }
        resources[n % 2]["tasks"]
            .as_array_mut()
            .unwrap()
            .push(json!({"task": id}));
    }
    plan
}

#[test]
fn every_blamed_task_and_resource_is_named_by_id_and_label() {
    let answer = evaluate(crowded());
    let mut blamed = 0;
    for constraint in answer["analysis"].as_array().unwrap() {
        for found in constraint["matches"].as_array().unwrap() {
            for entity in found["entities"].as_array().unwrap() {
                blamed += 1;
                assert!(
                    entity["id"].as_str().is_some_and(|id| !id.is_empty()),
                    "{entity}"
                );
                assert!(
                    entity["label"].as_str().is_some_and(|l| !l.is_empty()),
                    "{entity}"
                );
            }
        }
    }
    assert!(blamed > 20, "{blamed}");
}

#[test]
fn the_crowded_factory_breaks_hard_rules_the_demo_does_not() {
    let crowded = scores(crowded());
    let costing: Vec<&String> = crowded
        .iter()
        .filter(|(_, s)| s.starts_with('-') || s.contains("/-"))
        .map(|(n, _)| n)
        .collect();
    assert!(costing.len() >= 6, "{costing:?}");
    assert_ne!(crowded["Resource not allowed"], "0hard/0medium/0soft");
}

#[test]
fn fill_ranks_places_for_an_open_task_and_applies_one() {
    let plan = TaskScheduling::present(&TaskScheduling::demo("FACTORY").unwrap());
    let first = plan["tasks"][0]["id"].as_str().unwrap().to_string();
    let (code, answer) =
        engine(json!({"command": "fill", "plan": plan, "fill": {"items": [first], "top": 3}}));
    assert_eq!(code, 0, "{answer}");
    let options = answer["items"][0]["options"].as_array().unwrap().clone();
    assert!(!options.is_empty() && options.len() <= 3);
    // Best first, by the score each reaches.
    assert!(options
        .windows(2)
        .all(|w| w[0]["feasible"].as_bool() >= w[1]["feasible"].as_bool()));
    let value = options[0]["id"].as_str().unwrap();
    assert!(value.contains('@'), "{value}");
    let (code, applied) = engine(json!({"command": "fill", "plan": plan,
        "fill": {"apply": {"item": plan["tasks"][0]["id"], "value": value}}}));
    assert_eq!(code, 0, "{applied}");
    let resource = value.split('@').next().unwrap();
    let placed = applied["plan"]["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == resource)
        .unwrap();
    assert!(placed["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["task"] == plan["tasks"][0]["id"]));
}

#[test]
fn the_view_has_a_lane_per_resource_and_one_for_unscheduled_tasks() {
    let plan = crowded();
    let (code, answer) = engine(json!({"command": "view", "plan": plan}));
    assert_eq!(code, 0, "{answer}");
    let view = &answer["view"];
    assert_eq!(view["kind"], "timeline");
    let lanes = view["model"]["lanes"].as_array().unwrap();
    assert_eq!(lanes.len(), plan["resources"].as_array().unwrap().len() + 1);
    let red = lanes
        .iter()
        .flat_map(|l| l["items"].as_array().unwrap())
        .filter(|i| i["tone"] == "red")
        .count();
    assert!(red > 0);
}

#[test]
fn freezing_pins_the_tasks_that_start_before_the_moment() {
    let plan = dataset(json!({
        "tasks": [task("A", 60, json!({})), task("B", 60, json!({})), task("C", 60, json!({}))],
        "resources": [machine("M1", &["A", "B", "C"], json!({}))],
    }));
    // A starts 08:00, B 09:00: both before 09:30.
    let (code, answer) =
        engine(json!({"command": "evaluate", "plan": plan, "freeze_before": at(9, 30)}));
    assert_eq!(code, 0, "{answer}");
    let frozen = answer["issues"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["code"] == "frozen")
        .unwrap();
    assert!(
        frozen["message"].as_str().unwrap().starts_with("2 item(s)"),
        "{frozen}"
    );
    let (code, refused) =
        engine(json!({"command": "evaluate", "plan": plan, "freeze_before": "Monday"}));
    assert_eq!(code, 1);
    assert_eq!(refused["error"]["code"], "invalid_freeze");
}

#[test]
fn a_frozen_task_keeps_its_time_when_its_predecessor_runs_on_a_later_resource() {
    // M1 comes first, so its MILL is collected into the frozen lane before
    // the CUT on M2 it depends on; both start before 09:30 and stay timed.
    let plan = dataset(json!({
        "tasks": [
            task("CUT", 60, json!({})),
            task("MILL", 60, json!({"dependencies": [{"task": "CUT"}]})),
        ],
        "resources": [machine("M1", &["MILL"], json!({})), machine("M2", &["CUT"], json!({}))],
    }));
    let open = score_of(json!({"command": "evaluate", "plan": plan}));
    let frozen = score_of(json!({"command": "evaluate", "plan": plan, "freeze_before": at(9, 30)}));
    assert!(open.starts_with("0hard"), "{open}");
    assert_eq!(frozen, open);
}

fn score_of(request: Value) -> String {
    let (code, answer) = engine(request);
    assert_eq!(code, 0, "{answer}");
    answer["score"].as_str().unwrap().to_string()
}

#[test]
fn the_broken_link_penalty_parameter_sets_the_cost_of_a_broken_link() {
    let plan = dataset(json!({
        "parameters": {"brokenLinkPenalty": 50},
        "tasks": [task("A", 30, json!({})), task("B", 30, json!({"dependencies": [{"task": "A"}]}))],
        "resources": [machine("M1", &["B"], json!({}))],
    }));
    assert_eq!(
        scores(plan)["Dependency left unassigned"],
        "-50hard/0medium/0soft"
    );
    let listed: Vec<&str> = TaskScheduling::parameters()
        .iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(
        listed,
        ["defaultPriority", "interruptible", "brokenLinkPenalty"]
    );
}

#[test]
fn fill_offers_only_the_resources_a_task_may_run_on() {
    let plan = dataset(json!({
        "tasks": [task("A", 30, json!({"requiredResources": ["M2"]}))],
        "resources": [machine("M1", &[], json!({})), machine("M2", &[], json!({}))],
    }));
    let (code, answer) = engine(json!({"command": "fill", "plan": plan, "fill": {"top": 10}}));
    assert_eq!(code, 0, "{answer}");
    let ids: Vec<&str> = answer["items"][0]["options"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["M2@0"]);
}

#[test]
fn each_demo_summary_states_the_size_the_demo_has() {
    use solverforge_workbench_host::Model;
    type M = solverforge_workbench_task_scheduling::TaskScheduling;
    for demo in M::demo_info() {
        let plan = M::present(&M::demo(demo.id).expect("the demo builds"));
        for field in M::size_fields() {
            let count = field.count(&plan);
            assert!(
                count > 0
                    && demo.summary.contains(&format!(
                        "{count} {}",
                        field.label.split(' ').next().unwrap()
                    )),
                "{}: {count} {} not in {:?}",
                demo.id,
                field.label,
                demo.summary
            );
        }
    }
}

#[test]
fn replanning_changes_compile_to_patches_the_engine_accepts() {
    let plan = TaskScheduling::present(&demo_plan(TaskScheduling::demos()[0]));
    let (code, out) = engine(json!({"command": "edits", "plan": plan}));
    assert_eq!(code, 0, "{out}");
    let ids: Vec<&str> = out["edits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["resource_down", "task_due"]);
    let compile = |id: &str, args: Value| {
        let (code, out) =
            engine(json!({"command": "edit", "plan": plan, "edit": {"id": id, "args": args}}));
        assert_eq!(code, 0, "{out}");
        out
    };
    let down = compile(
        "resource_down",
        json!({"resource": "LASER-1", "from": "2027-03-02T08:00", "until": "2027-03-02T12:00", "reason": "Repair"}),
    );
    assert_eq!(
        down["operations"],
        json!([{"op": "add", "path": "/resources/[id=LASER-1]/unavailable/-",
                "value": {"start": "2027-03-02T08:00", "end": "2027-03-02T12:00", "label": "Repair"}}])
    );
    let due = compile(
        "task_due",
        json!({"task": "O001-cut", "due": "2027-03-02T10:00"}),
    );
    assert_eq!(due["operations"][0]["op"], "add");
    let mut changed = plan.clone();
    for compiled in [&down, &due] {
        apply(&mut changed, compiled["operations"].as_array().unwrap());
    }
    assert_eq!(changed["tasks"][0]["dueDate"], "2027-03-02T10:00");
    evaluate(changed);
    let (code, refused) = engine(json!({"command": "edit", "plan": plan,
        "edit": {"id": "resource_down", "args": {"resource": "LASER-1", "from": "2027-03-02T12:00", "until": "2027-03-02T08:00"}}}));
    assert_ne!(code, 0);
    assert_eq!(refused["error"]["code"], "invalid_edit");
}
