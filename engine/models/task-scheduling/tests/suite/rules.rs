//! One or more plans per constraint, each scored through the engine.

use serde_json::json;

use crate::common::{at, dataset, employee, machine, score, task};

const ZERO: &str = "0hard/0medium/0soft";

#[test]
fn a_pinned_task_before_its_earliest_start_costs_the_minutes_it_is_early() {
    let plan = |pinned_at: String| {
        dataset(json!({
            "tasks": [task("A", 60, json!({"minStart": at(10, 0), "pinned": true}))],
            "resources": [machine("M1", &[], json!({"tasks": [{"task": "A", "start": pinned_at}]}))],
        }))
    };
    assert_eq!(
        score(plan(at(9, 30)), "Started before earliest start"),
        "-30hard/0medium/0soft"
    );
    assert_eq!(
        score(plan(at(10, 0)), "Started before earliest start"),
        ZERO
    );
}

#[test]
fn an_unpinned_task_waits_for_its_earliest_start() {
    let plan = dataset(json!({
            "weights": {"Machine idle time": 1},
        "tasks": [task("A", 60, json!({"minStart": at(10, 0)}))],
        "resources": [machine("M1", &["A"], json!({}))],
    }));
    assert_eq!(score(plan.clone(), "Started before earliest start"), ZERO);
    // The machine waits from 08:00 to 10:00.
    assert_eq!(score(plan, "Machine idle time"), "0hard/0medium/-120soft");
}

#[test]
fn finishing_after_the_deadline_costs_the_minutes_over() {
    let plan = |max_end: String| {
        dataset(json!({
            "tasks": [task("A", 90, json!({"maxEnd": max_end}))],
            "resources": [machine("M1", &["A"], json!({}))],
        }))
    };
    assert_eq!(
        score(plan(at(9, 0)), "Finished after deadline"),
        "-30hard/0medium/0soft"
    );
    assert_eq!(score(plan(at(9, 30)), "Finished after deadline"), ZERO);
}

#[test]
fn a_planned_task_whose_dependency_is_unassigned_costs_the_link_penalty() {
    let plan = |placed: &[&str]| {
        dataset(json!({
            "tasks": [task("A", 30, json!({})), task("B", 30, json!({"dependencies": [{"task": "A"}]}))],
            "resources": [machine("M1", placed, json!({}))],
        }))
    };
    assert_eq!(
        score(plan(&["B"]), "Dependency left unassigned"),
        "-1000hard/0medium/0soft"
    );
    assert_eq!(score(plan(&["A", "B"]), "Dependency left unassigned"), ZERO);
    // Both open: only the unassigned rule speaks.
    assert_eq!(score(plan(&[]), "Dependency left unassigned"), ZERO);
}

#[test]
fn a_task_queued_before_the_task_it_waits_on_cannot_be_timed_and_costs_a_broken_link() {
    let plan = |placed: &[&str]| {
        dataset(json!({
            "tasks": [task("A", 30, json!({})), task("B", 30, json!({"dependencies": [{"task": "A"}]}))],
            "resources": [machine("M1", placed, json!({}))],
        }))
    };
    // B waits on A, which runs after it: neither can be timed.
    assert_eq!(
        score(plan(&["B", "A"]), "Dependency offset broken"),
        "-2000hard/0medium/0soft"
    );
    assert_eq!(score(plan(&["B", "A"]), "Task left unassigned"), ZERO);
    assert_eq!(score(plan(&["A", "B"]), "Dependency offset broken"), ZERO);
}

#[test]
fn a_dependent_task_waits_for_its_minimum_offset_and_is_penalized_past_its_maximum() {
    let plan = |m2_start: String| {
        dataset(json!({
            "tasks": [
                task("A", 60, json!({})),
                task("B", 30, json!({"dependencies": [{"task": "A", "minOffset": 15, "maxOffset": 30}]})),
            ],
            "resources": [
                machine("M1", &["A"], json!({})),
                machine("M2", &["B"], json!({"start": m2_start})),
            ],
        }))
    };
    // A ends 09:00; B may start 09:15 to 09:30.
    assert_eq!(score(plan(at(8, 0)), "Dependency offset broken"), ZERO);
    assert_eq!(
        score(plan(at(10, 0)), "Dependency offset broken"),
        "-30hard/0medium/0soft"
    );
}

#[test]
fn dependent_tasks_that_must_share_a_resource_cost_the_link_penalty_when_split() {
    let tasks = json!([
        task("A", 30, json!({})),
        task(
            "B",
            30,
            json!({"dependencies": [{"task": "A", "sameResource": true}]})
        )
    ]);
    let split = dataset(json!({"tasks": tasks,
        "resources": [machine("M1", &["A"], json!({})), machine("M2", &["B"], json!({}))]}));
    let shared =
        dataset(json!({"tasks": tasks, "resources": [machine("M1", &["A", "B"], json!({}))]}));
    assert_eq!(
        score(split, "Dependency split across resources"),
        "-1000hard/0medium/0soft"
    );
    assert_eq!(score(shared, "Dependency split across resources"), ZERO);
}

#[test]
fn every_task_between_two_directly_linked_tasks_costs_the_link_penalty() {
    let plan = |order: &[&str]| {
        dataset(json!({
            "tasks": [
                task("A", 30, json!({})), task("X", 30, json!({})), task("Y", 30, json!({})),
                task("B", 30, json!({"dependencies": [{"task": "A", "sameResource": true,
                                                       "noIntermediateTasks": true}]})),
            ],
            "resources": [machine("M1", order, json!({}))],
        }))
    };
    assert_eq!(
        score(
            plan(&["A", "X", "Y", "B"]),
            "Dependency interrupted by other tasks"
        ),
        "-2000hard/0medium/0soft"
    );
    assert_eq!(
        score(
            plan(&["X", "A", "B", "Y"]),
            "Dependency interrupted by other tasks"
        ),
        ZERO
    );
}

#[test]
fn a_missing_tag_costs_the_minutes_it_does_not_cover() {
    let plan = |tags: serde_json::Value| {
        dataset(json!({
            "tags": [{"id": "Welding"}],
            "tasks": [task("A", 60, json!({"requiredTags": ["Welding"]}))],
            "resources": [employee("E1", &["A"], json!({"tags": tags}))],
        }))
    };
    assert_eq!(
        score(plan(json!([])), "Required tag missing"),
        "-60hard/0medium/0soft"
    );
    // Certified from 08:20: 20 of the 60 minutes are not covered.
    assert_eq!(
        score(
            plan(json!([{"tag": "Welding", "from": at(8, 20)}])),
            "Required tag missing"
        ),
        "-20hard/0medium/0soft"
    );
    assert_eq!(
        score(plan(json!([{"tag": "Welding"}])), "Required tag missing"),
        ZERO
    );
}

#[test]
fn a_resource_outside_the_tasks_list_costs_its_priority() {
    let plan = |on: &str| {
        dataset(json!({
            "tasks": [task("A", 30, json!({"requiredResources": ["M2"], "priority": 3}))],
            "resources": [machine("M1", if on == "M1" { &["A"] } else { &[] }, json!({})),
                          machine("M2", if on == "M2" { &["A"] } else { &[] }, json!({}))],
        }))
    };
    assert_eq!(
        score(plan("M1"), "Resource not allowed"),
        "-3hard/0medium/0soft"
    );
    assert_eq!(score(plan("M2"), "Resource not allowed"), ZERO);
}

fn hand_over(transition: serde_json::Value) -> serde_json::Value {
    dataset(json!({
        "resourceTransitions": [transition],
        "tasks": [task("A", 30, json!({})), task("B", 30, json!({"dependencies": [{"task": "A"}]}))],
        "resources": [machine("M1", &["A"], json!({})), machine("M2", &["B"], json!({}))],
    }))
}

#[test]
fn a_forbidden_hand_over_between_resources_costs_the_link_penalty() {
    let forbidden = hand_over(json!({"from": "M1", "to": "M2", "forbidden": true}));
    assert_eq!(
        score(forbidden, "Forbidden resource hand-over"),
        "-1000hard/0medium/0soft"
    );
    let other_way = hand_over(json!({"from": "M2", "to": "M1", "forbidden": true}));
    assert_eq!(score(other_way, "Forbidden resource hand-over"), ZERO);
}

#[test]
fn a_preferred_hand_over_earns_its_reward() {
    let preferred = hand_over(json!({"from": "M1", "to": "M2", "reward": 15}));
    assert_eq!(
        score(preferred, "Preferred resource hand-over"),
        "0hard/0medium/15soft"
    );
}

#[test]
fn an_unassigned_task_costs_its_priority_at_medium_level() {
    let plan = dataset(json!({
        "tasks": [task("A", 30, json!({"priority": 4})), task("B", 30, json!({}))],
        "resources": [machine("M1", &[], json!({}))],
    }));
    assert_eq!(score(plan, "Task left unassigned"), "0hard/-5medium/0soft");
}

#[test]
fn makespan_is_the_longest_resource_span() {
    let plan = dataset(json!({
        "tasks": [task("A", 60, json!({})), task("B", 30, json!({})), task("C", 45, json!({}))],
        "resources": [machine("M1", &["A", "B"], json!({})), machine("M2", &["C"], json!({}))],
    }));
    assert_eq!(score(plan, "Makespan"), "0hard/0medium/-90soft");
}

#[test]
fn ending_away_from_the_ideal_end_costs_minutes_times_priority() {
    let plan = |ideal: String| {
        dataset(json!({
            "tasks": [task("A", 60, json!({"idealEnd": ideal, "priority": 2}))],
            "resources": [machine("M1", &["A"], json!({}))],
        }))
    };
    assert_eq!(
        score(plan(at(8, 30)), "Distance from ideal end"),
        "0hard/0medium/-60soft"
    );
    assert_eq!(
        score(plan(at(9, 30)), "Distance from ideal end"),
        "0hard/0medium/-60soft"
    );
    assert_eq!(score(plan(at(9, 0)), "Distance from ideal end"), ZERO);
}

#[test]
fn finishing_after_the_due_date_costs_the_minutes_late() {
    let plan = dataset(json!({
        "tasks": [task("A", 60, json!({"dueDate": at(8, 45)})), task("B", 30, json!({"dueDate": at(12, 0)}))],
        "resources": [machine("M1", &["A", "B"], json!({}))],
    }));
    assert_eq!(
        score(plan, "Lateness after due date"),
        "0hard/0medium/-15soft"
    );
}

#[test]
fn a_change_of_task_type_on_a_machine_counts_a_changeover_and_its_setup_minutes() {
    let plan = |order: &[&str]| {
        dataset(json!({
            "taskTypes": [{"id": "steel"}, {"id": "aluminium"}],
            "machineTypes": [{"id": "Mill"}],
            "changeovers": [{"machineType": "Mill", "from": "steel", "to": "aluminium", "duration": 30}],
            "tasks": [task("S1", 30, json!({"taskType": "steel"})), task("S2", 30, json!({"taskType": "steel"})),
                      task("A1", 30, json!({"taskType": "aluminium"}))],
            "resources": [machine("M1", order, json!({"machineType": "Mill"}))],
        }))
    };
    let mixed = plan(&["S1", "A1", "S2"]);
    // steel -> aluminium has 30 minutes of setup; aluminium -> steel has none listed but still changes type.
    assert_eq!(
        score(mixed.clone(), "Changeover count"),
        "0hard/0medium/-2soft"
    );
    assert_eq!(score(mixed, "Changeover time"), "0hard/0medium/-30soft");
    let grouped = plan(&["S1", "S2", "A1"]);
    assert_eq!(
        score(grouped.clone(), "Changeover count"),
        "0hard/0medium/-1soft"
    );
    // The setup delays the aluminium task: the makespan includes it.
    assert_eq!(score(grouped, "Makespan"), "0hard/0medium/-120soft");
}

#[test]
fn a_machine_waiting_for_a_dependency_is_idle_but_an_employee_is_not() {
    let plan = |kind_b: &str| {
        let second = if kind_b == "machine" {
            machine("R2", &["B"], json!({}))
        } else {
            employee("R2", &["B"], json!({}))
        };
        dataset(json!({
            "weights": {"Machine idle time": 1},
            "tasks": [task("A", 60, json!({})), task("B", 30, json!({"dependencies": [{"task": "A"}]}))],
            "resources": [machine("M1", &["A"], json!({})), second],
        }))
    };
    assert_eq!(
        score(plan("machine"), "Machine idle time"),
        "0hard/0medium/-60soft"
    );
    assert_eq!(score(plan("employee"), "Machine idle time"), ZERO);
}

#[test]
fn unavailable_time_pauses_an_interruptible_task_and_is_not_idle_time() {
    let plan = |interruptible: bool| {
        dataset(json!({
            "weights": {"Machine idle time": 1},
            "parameters": {"interruptible": interruptible},
            "tasks": [task("A", 60, json!({})), task("B", 60, json!({}))],
            "resources": [machine("M1", &["A", "B"], json!({
                "unavailable": [{"start": at(9, 30), "end": at(10, 30)}]}))],
        }))
    };
    // Interruptible: B runs 09:00-09:30 and 10:30-11:00.
    assert_eq!(score(plan(true), "Makespan"), "0hard/0medium/-180soft");
    assert_eq!(score(plan(true), "Machine idle time"), ZERO);
    // Whole: B waits for 10:30 and ends 11:30; the 30 minutes before the break are idle.
    assert_eq!(score(plan(false), "Makespan"), "0hard/0medium/-210soft");
    assert_eq!(
        score(plan(false), "Machine idle time"),
        "0hard/0medium/-30soft"
    );
}

#[test]
fn idle_time_is_off_unless_weighted() {
    let plan = dataset(json!({
        "tasks": [task("A", 60, json!({"minStart": at(10, 0)}))],
        "resources": [machine("M1", &["A"], json!({}))],
    }));
    assert_eq!(score(plan, "Machine idle time"), ZERO);
}
