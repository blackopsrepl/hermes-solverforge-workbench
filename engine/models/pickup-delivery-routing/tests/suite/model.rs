//! The model as a whole: scoring, wire format, names, fill, view, freeze.

use serde_json::{json, Value};
use solverforge::ScoreDirector;
use solverforge_workbench_host::Model;
use solverforge_workbench_pickup_delivery_routing::constraints::{define_constraints, PIN_SHIFT};
use solverforge_workbench_pickup_delivery_routing::PickupDeliveryRouting;

use crate::common::*;

#[test]
fn the_demo_scores_incrementally_like_in_full() {
    let mut plan = demo_plan();
    scramble(&mut plan, 7);
    assert_incremental_matches_full(plan, 400, 11);
}

#[test]
fn a_small_plan_with_breaks_pins_and_pooling_scores_incrementally_like_in_full() {
    let wire = dataset(
        vec![
            driver(
                "ann",
                shift(
                    "ann-mon",
                    &["Aa", "brk:L", "Ba", "Ab", "Bb"],
                    json!({"capacities": [{"type": "seats", "quantity": 1}],
                           "requiredBreaks": [{"id": "L", "type": "FLOATING", "policy": "REJECT_JOBS",
                                               "minStartTime": at(0, 8, 10), "maxEndTime": at(0, 8, 30),
                                               "duration": 15}]}),
                ),
            ),
            driver("bob", shift("bob-mon", &["Ca", "Cb"], json!({}))),
        ],
        vec![
            job(
                "A",
                vec![
                    stop("Aa", 10, json!({})),
                    stop("Ab", 10, json!({"location": [45.49, 9.24]})),
                ],
                json!({"demands": [{"type": "seats", "demand": 1}], "maximumTimeBurden": 0}),
            ),
            job(
                "B",
                vec![
                    stop("Ba", 10, json!({"location": [45.49, 9.24]})),
                    stop("Bb", 10, json!({})),
                ],
                json!({"demands": [{"type": "seats", "demand": 1}], "prohibitJobPooling": true}),
            ),
            job(
                "C",
                vec![
                    stop("Ca", 5, json!({})),
                    stop("Cb", 5, json!({})),
                    stop("Cc", 5, json!({})),
                ],
                json!({"requiredDrivers": ["ann"]}),
            ),
        ],
        json!({}),
    );
    let plan = prepared(wire);
    assert_incremental_matches_full(plan, 600, 3);
}

#[test]
fn the_catalog_names_every_constraint_the_engine_scores() {
    let catalog = PickupDeliveryRouting::catalog();
    assert_eq!(catalog.len(), 36);
    let scored = scores(PickupDeliveryRouting::present(
        &PickupDeliveryRouting::demo("MILAN_TWO_DAYS").unwrap(),
    ));
    assert_eq!(scored.len(), 36);
    for info in &catalog {
        assert!(
            scored.contains_key(info.name),
            "{} is not scored",
            info.name
        );
        assert!(info.description.ends_with('.'), "{}", info.name);
    }
}

#[test]
fn the_demo_round_trips_through_the_wire_format() {
    let wire =
        PickupDeliveryRouting::present(&PickupDeliveryRouting::demo("MILAN_TWO_DAYS").unwrap());
    let plan = prepared(wire.clone());
    let again = PickupDeliveryRouting::present(&plan);
    assert_eq!(again["drivers"].as_array().unwrap().len(), 15);
    assert_eq!(again["jobs"], wire["jobs"]);
    let stops: usize = again["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|j| j["stops"].as_array().unwrap().len())
        .sum();
    assert_eq!(again["unassignedStops"].as_array().unwrap().len(), stops);
}

#[test]
fn the_demo_is_large_and_uses_every_feature() {
    let demo = PickupDeliveryRouting::demo("MILAN_TWO_DAYS").unwrap();
    let shifts: usize = demo.drivers.iter().map(|d| d.shifts.len()).sum();
    assert_eq!(shifts, 30);
    assert!(demo.jobs.len() > 150, "{}", demo.jobs.len());
    let has = |f: &dyn Fn(&solverforge_workbench_pickup_delivery_routing::wire::Job) -> bool| {
        demo.jobs.iter().any(f)
    };
    assert!(has(&|j| j.stops.len() == 3));
    assert!(has(&|j| j.prohibit_job_pooling));
    assert!(has(&|j| !j.prohibited_jobs.is_empty()));
    assert!(has(&|j| !j.required_tags.is_empty()));
    assert!(has(&|j| !j.required_skills.is_empty()));
    assert!(has(&|j| !j.preferred_drivers.is_empty()));
    assert!(has(&|j| !j.required_drivers.is_empty()));
    assert!(has(&|j| !j.prohibited_drivers.is_empty()));
    assert!(has(&|j| j
        .priority
        .as_deref()
        .is_some_and(|p| p.starts_with("opt-"))));
    assert!(has(&|j| j
        .stops
        .iter()
        .any(|s| s.latest_sla_end_time.is_some())));
    assert!(has(&|j| j.stops.iter().any(|s| s.time_windows.len() == 2)));
    let breaks: Vec<_> = demo
        .drivers
        .iter()
        .flat_map(|d| &d.shifts)
        .flat_map(|s| &s.required_breaks)
        .collect();
    assert!(breaks.iter().any(|b| b.location.is_some()));
    assert!(breaks
        .iter()
        .any(|b| b.start_time.is_some() && b.location.is_none()));
}

/// Rules a plan breaks, by name.
fn broken_rules(plan: solverforge_workbench_pickup_delivery_routing::Plan) -> Vec<String> {
    let mut director = ScoreDirector::new(plan, define_constraints());
    director.calculate_score();
    director
        .constraint_analyses()
        .into_iter()
        .filter(|a| a.score != Default::default())
        .map(|a| a.constraint_ref.full_name())
        .collect()
}

#[test]
fn scrambled_and_crowded_demos_break_every_rule_the_data_can_break() {
    let mut scrambled = demo_plan();
    scramble(&mut scrambled, 5);
    let mut broken = broken_rules(scrambled);
    // Crowded: each shift takes a run of jobs, every pick-up before every
    // drop-off, so jobs ride together.
    let mut crowded = demo_plan();
    let shifts = crowded.shifts.len() - 1;
    for shift in &mut crowded.shifts {
        shift.visits.clear();
    }
    let jobs = crowded.facts.jobs.clone();
    for (n, chunk) in jobs.chunks(jobs.len().div_ceil(shifts)).enumerate() {
        let firsts = chunk.iter().map(|j| j.stops[0]);
        let rest = chunk.iter().flat_map(|j| j.stops[1..].iter().copied());
        crowded.shifts[n].visits = firsts.chain(rest).collect();
    }
    // Each shift takes its breaks in the middle, with jobs on board.
    for (v, fact) in crowded.facts.visits.clone().iter().enumerate() {
        if let Some(s) = fact.owner {
            let list = &mut crowded.shifts[s].visits;
            list.insert(list.len() / 2, v);
        }
    }
    broken.extend(broken_rules(crowded));
    // Straight-line travel reaches everywhere, the demo pins nothing and
    // waiting is off unless weighted.
    let needs_other_data = [
        "Unreachable leg in a route",
        "Stop unreachable from the previous place",
        "Pinned stop off its shift",
        "Pinned stop starts late",
        "Waiting time",
    ];
    for info in PickupDeliveryRouting::catalog() {
        if needs_other_data.contains(&info.name) {
            continue;
        }
        assert!(
            broken.iter().any(|b| b == info.name),
            "{} never breaks",
            info.name
        );
    }
}

#[test]
fn moving_a_pinned_stop_off_its_shift_costs_ten_billion() {
    let mut wire = dataset(
        vec![
            driver("ann", shift("ann-mon", &["a"], json!({}))),
            driver("bob", shift("bob-mon", &[], json!({}))),
        ],
        vec![job("J", vec![stop("a", 10, json!({}))], json!({}))],
        json!({}),
    );
    wire["drivers"][0]["shifts"][0]["itinerary"][0] =
        json!({"id": "a", "kind": "STOP", "pin": true, "startServiceTime": at(0, 8, 0)});
    let plan = prepared(wire);
    let mut director = ScoreDirector::new(plan, define_constraints());
    let before = director.calculate_score();
    director.before_variable_changed(0, 0);
    director.before_variable_changed(0, 1);
    let plan = director.working_solution_mut();
    let v = plan.shifts[0].visits.remove(0);
    plan.shifts[1].visits.push(v);
    director.after_variable_changed(0, 0);
    director.after_variable_changed(0, 1);
    let after = director.calculate_score();
    let pin = director
        .constraint_analyses()
        .into_iter()
        .find(|a| a.constraint_ref.full_name() == PIN_SHIFT)
        .unwrap();
    assert_eq!(pin.score.to_string(), "-10000000000hard/0medium/0soft");
    assert!(after < before);
}

#[test]
fn a_solve_plans_each_job_whole_on_a_shift_that_fits_it() {
    let (code, done) = solve(two_shift_plan(), 2);
    assert_eq!(code, 0, "{done}");
    assert_eq!(done["feasible"], true, "{}", done["score"]);
    let itinerary = &done["plan"]["drivers"][0]["shifts"][0]["itinerary"];
    let ids: Vec<&str> = itinerary
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["Ja", "Jb"]);
}

/// Every entity a crowded plan's analysis blames has an id and a label.
#[test]
fn every_blamed_entity_has_an_id_and_a_label() {
    let mut wire =
        PickupDeliveryRouting::present(&PickupDeliveryRouting::demo("MILAN_TWO_DAYS").unwrap());
    let stops: Vec<Value> = wire["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .take(30)
        .flat_map(|j| j["stops"].as_array().unwrap().clone())
        .map(|s| json!({"id": s["id"], "kind": "STOP"}))
        .collect();
    wire["drivers"][0]["shifts"][0]["itinerary"] = Value::Array(stops);
    let answer = evaluate(wire);
    let mut blamed = 0;
    for constraint in answer["analysis"].as_array().unwrap() {
        for found in constraint["matches"].as_array().unwrap() {
            for entity in found["entities"].as_array().unwrap() {
                blamed += 1;
                assert!(
                    entity["id"].is_string() || entity["label"] == "Unplanned",
                    "{entity}"
                );
                assert!(!entity["label"].as_str().unwrap().is_empty(), "{entity}");
            }
        }
    }
    assert!(blamed > 50, "{blamed}");
}

fn two_shift_plan() -> Value {
    dataset(
        vec![
            driver("ann", shift("ann-mon", &[], json!({"tags": ["lift"]}))),
            driver("bob", shift("bob-mon", &[], json!({}))),
        ],
        vec![job(
            "J",
            vec![
                stop("Ja", 10, json!({})),
                stop("Jb", 10, json!({"location": [45.49, 9.24]})),
            ],
            json!({"requiredTags": ["lift"]}),
        )],
        json!({}),
    )
}

#[test]
fn fill_ranks_the_shifts_for_an_open_job_and_applies_one() {
    let (code, answer) =
        engine(json!({"command": "fill", "plan": two_shift_plan(), "fill": {"top": 2}}));
    assert_eq!(code, 0, "{answer}");
    let items = answer["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], "J");
    let options = items[0]["options"].as_array().unwrap();
    assert_eq!(options[0]["id"], "ann-mon");
    assert_eq!(options[0]["feasible"], true);
    assert_eq!(options[1]["id"], "bob-mon");
    assert_eq!(options[1]["feasible"], false);
    let (code, applied) = engine(json!({"command": "fill", "plan": two_shift_plan(),
                                         "fill": {"apply": {"item": "J", "value": "ann-mon"}}}));
    assert_eq!(code, 0, "{applied}");
    let itinerary = &applied["plan"]["drivers"][0]["shifts"][0]["itinerary"];
    let ids: Vec<&str> = itinerary
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["Ja", "Jb"]);
}

#[test]
fn the_view_draws_routes_on_a_map_and_a_lane_per_shift() {
    let mut wire = two_shift_plan();
    wire["drivers"][0]["shifts"][0]["itinerary"] =
        json!([{"id": "Ja", "kind": "STOP"}, {"id": "Jb", "kind": "STOP"}]);
    let (code, answer) = engine(json!({"command": "view", "plan": wire}));
    assert_eq!(code, 0, "{answer}");
    let view = &answer["view"];
    assert_eq!(view["kind"], "routes");
    assert_eq!(view["vehicles"].as_array().unwrap().len(), 1);
    assert_eq!(view["vehicles"][0]["stops"].as_array().unwrap().len(), 2);
    assert_eq!(view["unassigned"].as_array().unwrap().len(), 0);
    assert_eq!(
        view["timeline"]["model"]["lanes"].as_array().unwrap().len(),
        2
    );
}

#[test]
fn freezing_pins_the_stops_already_started_and_the_one_on_the_way() {
    let mut wire = two_shift_plan();
    wire["drivers"][0]["shifts"][0]["itinerary"] =
        json!([{"id": "Ja", "kind": "STOP"}, {"id": "Jb", "kind": "STOP"}]);
    let mut plan = prepared(wire);
    // Ja runs 08:00–08:10; the drive to Jb takes 99 seconds.
    assert_eq!(
        PickupDeliveryRouting::freeze(&mut plan, &at(0, 8, 11)).unwrap(),
        2
    );
    let out = PickupDeliveryRouting::present(&plan);
    let itinerary = out["drivers"][0]["shifts"][0]["itinerary"]
        .as_array()
        .unwrap();
    assert!(itinerary.iter().all(|i| i["pin"] == true));
    let mut plan = prepared({
        let mut w = two_shift_plan();
        w["drivers"][0]["shifts"][0]["itinerary"] =
            json!([{"id": "Ja", "kind": "STOP"}, {"id": "Jb", "kind": "STOP"}]);
        w["parameters"] = json!({"pinNextStopDuringFreeze": false});
        w
    });
    assert_eq!(
        PickupDeliveryRouting::freeze(&mut plan, &at(0, 8, 11)).unwrap(),
        1
    );
}

#[test]
fn the_dataset_is_checked_before_it_is_scored() {
    let base = two_shift_plan;
    let mut dup = base();
    dup["jobs"][0]["stops"][1]["id"] = json!("Ja");
    assert!(issue_codes(dup).contains(&"duplicate_id".to_string()));
    let mut unknown = base();
    unknown["drivers"][0]["shifts"][0]["itinerary"] = json!([{"id": "nope", "kind": "STOP"}]);
    assert!(issue_codes(unknown).contains(&"unknown_itinerary_item".to_string()));
    let mut twice = base();
    twice["drivers"][0]["shifts"][0]["itinerary"] = json!([{"id": "Ja", "kind": "STOP"}]);
    twice["drivers"][1]["shifts"][0]["itinerary"] = json!([{"id": "Ja", "kind": "STOP"}]);
    assert!(issue_codes(twice).contains(&"assigned_twice".to_string()));
    let mut kind = base();
    kind["drivers"][0]["shifts"][0]["itinerary"] = json!([{"id": "Ja", "kind": "BREAK"}]);
    assert!(issue_codes(kind).contains(&"wrong_item_kind".to_string()));
    let mut pin = base();
    pin["drivers"][0]["shifts"][0]["itinerary"] =
        json!([{"id": "Ja", "kind": "STOP", "pin": true}]);
    assert!(issue_codes(pin).contains(&"pinned_stop_without_start".to_string()));
    let mut time = base();
    time["drivers"][0]["shifts"][0]["minStartTime"] = json!("tomorrow");
    assert!(issue_codes(time).contains(&"bad_time".to_string()));
    let mut start = base();
    start["drivers"][0]["shifts"][0]
        .as_object_mut()
        .unwrap()
        .remove("minStartTime");
    assert!(issue_codes(start).contains(&"shift_without_start".to_string()));
    let mut priority = base();
    priority["jobs"][0]["priority"] = json!("urgent");
    assert!(issue_codes(priority).contains(&"unknown_priority".to_string()));
    let mut demand = base();
    demand["jobs"][0]["stops"]
        .as_array_mut()
        .unwrap()
        .push(stop("Jc", 5, json!({})));
    demand["jobs"][0]["demands"] = json!([{"type": "seats", "demand": 1}]);
    assert!(issue_codes(demand).contains(&"job_demand_needs_two_stops".to_string()));
    let mut matrix = base();
    matrix["travelMatrix"] = json!({"locations": [DEPOT], "seconds": [[0]], "meters": [[0]]});
    assert!(issue_codes(matrix).contains(&"place_not_in_travel_matrix".to_string()));
    let mut unbalanced = base();
    unbalanced["jobs"][0]["stops"][0]["demands"] = json!([{"type": "parcels", "pickup": 3}]);
    assert!(issue_codes(unbalanced).contains(&"unbalanced_demand".to_string()));
    let mut pinned = base();
    pinned["drivers"][0]["shifts"][0]["pinned"] = json!(true);
    pinned["drivers"][0]["shifts"][0]["requiredBreaks"] =
        json!([{"id": "L", "type": "FLOATING", "minStartTime": at(0, 11, 0), "duration": 30}]);
    assert!(issue_codes(pinned).contains(&"pinned_shift_misses_break".to_string()));
}

#[test]
fn the_engine_describes_the_model() {
    let (code, model) = engine(json!({"command": "describe"}));
    assert_eq!(code, 0, "{model}");
    assert_eq!(model["view"], "routes");
    assert_eq!(model["demos"], json!(["MILAN_TWO_DAYS"]));
    assert_eq!(model["constraints"].as_array().unwrap().len(), 36);
    let params: Vec<&str> = model["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        params,
        [
            "defaultPriority",
            "maximumTimeBurden",
            "averageSpeedKmh",
            "detourFactor",
            "pinNextStopDuringFreeze"
        ]
    );
}

#[test]
fn parameters_change_straight_line_travel() {
    let far = || {
        dataset(
            vec![driver("ann", shift("ann-mon", &["a"], json!({})))],
            vec![job(
                "J",
                vec![stop("a", 10, json!({"location": [45.4936, 9.2367]}))],
                json!({}),
            )],
            json!({}),
        )
    };
    let slow = score(far(), "Travel time");
    let mut fast = far();
    fast["parameters"] = json!({"averageSpeedKmh": 64.0});
    let quick = score(fast, "Travel time");
    let secs = |s: &str| -> i64 {
        s.trim_start_matches("0hard/0medium/-")
            .trim_end_matches("soft")
            .parse()
            .unwrap()
    };
    // About a kilometre each way at the default 1.35 detour and 32 km/h.
    assert!((300..320).contains(&secs(&slow)), "{slow}");
    assert!(
        (secs(&quick) * 2 - secs(&slow)).abs() <= 2,
        "{quick} vs {slow}"
    );
}
