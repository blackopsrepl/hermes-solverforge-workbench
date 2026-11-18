//! The model as a whole: scoring, wire format, names, fill, view, freeze.

use serde_json::{json, Value};
use solverforge::{PlanningSolution, ScoreDirector};
use solverforge_workbench_field_service_routing::constraints::{define_constraints, Rule, RULES};
use solverforge_workbench_field_service_routing::{route, FieldServiceRouting};
use solverforge_workbench_host::Model;

use std::collections::HashMap;

use solverforge_workbench_host::roads::provide_complete;

use crate::common::*;

#[test]
fn the_demo_scores_incrementally_like_in_full() {
    let mut plan = demo_plan();
    scramble(&mut plan, 7);
    assert_incremental_matches_full(plan, 400, 11);
}

#[test]
fn the_unscrambled_demo_scores_incrementally_like_in_full() {
    assert_incremental_matches_full(demo_plan(), 400, 5);
}

/// Every route the views draw is drivable as drawn: each item starts no
/// earlier than its arrival, and each arrival follows the departure before
/// it by the leg's travel, waiting included.
#[test]
fn every_route_is_drawn_as_it_is_driven() {
    for seed in [3, 7] {
        let mut plan = demo_plan();
        scramble(&mut plan, seed);
        plan.update_all_shadows();
        let routes = route::routes(&plan);
        let mut timed = 0;
        for (s, shift) in plan.shifts.iter().enumerate() {
            if plan.facts.shifts[s].bench || !routes[s].active {
                continue;
            }
            let mut departed = routes[s].start;
            for (position, &v) in shift.visits.iter().enumerate() {
                let drawn = routes[s].elements[position];
                if drawn.untimed {
                    continue;
                }
                let at = drawn.at;
                timed += 1;
                assert!(at.arrival <= at.start, "seed {seed}, element {v}: {at:?}");
                assert!(at.start <= at.end, "seed {seed}, element {v}: {at:?}");
                assert!(
                    at.arrival >= departed + at.travel,
                    "seed {seed}, element {v} arrives before it can: {at:?}, left {departed}"
                );
                assert_eq!(at.wait, at.start - at.arrival, "seed {seed}, element {v}");
                departed = at.end;
            }
            assert!(routes[s].end_arrival >= departed, "seed {seed}, shift {s}");
        }
        assert!(timed > 100, "seed {seed}: only {timed} timed");
    }
}

/// A plan with every kind of link: a joint visit across two shifts, a
/// dependency with a delay to the next working day, a same-shift
/// dependency, breaks, a pin and an exclusion.
fn linked_plan() -> Value {
    let lunch = json!({"id": "L", "type": "FLOATING", "minStartTime": at(0, 9, 0),
                       "maxEndTime": at(0, 10, 0), "duration": 15});
    dataset(
        vec![
            json!({"id": "ann", "shifts": [
                shift("ann-mon", &["A", "Ga", "brk:L", "P"], json!({"requiredBreaks": [lunch]})),
                shift("ann-tue", &["D"], json!({"minStartTime": at(1, 8, 0)})),
            ]}),
            vehicle("bob", shift("bob-mon", &["Gb", "E", "X"], json!({}))),
        ],
        vec![
            visit("A", 20, json!({"location": [45.08, 7.69]})),
            visit(
                "D",
                30,
                json!({"visitDependencies": [{"id": "A-D", "precedingVisit": "A",
                    "minDelayTo": {"adjuster": "NEXT_WORKING_DAY", "increment": 1, "time": "09:00"},
                    "coordination": "SAME_VEHICLE"}]}),
            ),
            visit("P", 10, json!({})),
            visit(
                "E",
                10,
                json!({"visitDependencies": [{"id": "P-E", "precedingVisit": "P",
                    "maxDelay": 30, "coordination": "SAME_SHIFT"}]}),
            ),
            visit("X", 10, json!({"exclusions": ["P"]})),
        ],
        json!({"visitGroups": [{"id": "G", "visits": [
            visit("Ga", 40, json!({"location": [45.06, 7.70]})),
            visit("Gb", 40, json!({"location": [45.06, 7.70]})),
        ]}]}),
    )
}

#[test]
fn a_small_plan_with_every_link_scores_incrementally_like_in_full() {
    for seed in [3, 9, 17] {
        assert_incremental_matches_full(prepared(linked_plan()), 600, seed);
    }
}

#[test]
fn a_joint_visit_starts_together_on_both_shifts() {
    let mut plan = prepared(linked_plan());
    plan.update_all_shadows();
    let index = |id: &str| plan.facts.visits.iter().position(|v| v.id == id).unwrap();
    let (a, b) = (index("Ga"), index("Gb"));
    let start = |v: usize| plan.elements[v].at().unwrap().start;
    // Ann reaches Ga after A; Bob could be there sooner: both start when
    // Ann arrives.
    assert_eq!(start(a), start(b));
    // Bob sets off late, just in time to start with Ann.
    let bob = plan.elements[b].shift.unwrap();
    let routes = route::routes(&plan);
    assert!(routes[bob].start > plan.facts.shifts[bob].min_start);
    assert_eq!(plan.elements[b].at().unwrap().wait, 0);
}

#[test]
fn a_dependency_delayed_to_the_next_working_day_waits_for_it() {
    let mut plan = prepared(linked_plan());
    plan.update_all_shadows();
    let d = plan.facts.visits.iter().position(|v| v.id == "D").unwrap();
    // Tuesday 09:00, though Ann's Tuesday starts at 08:00.
    assert_eq!(
        solverforge_workbench_field_service_routing::time::format_date_time(
            plan.elements[d].at().unwrap().start
        ),
        at(1, 9, 0)
    );
}

#[test]
fn the_catalog_names_every_constraint_the_engine_scores() {
    let catalog = FieldServiceRouting::catalog();
    assert_eq!(catalog.len(), 64);
    let hard = catalog
        .iter()
        .filter(|c| matches!(c.level, solverforge_workbench_host::Level::Hard));
    assert_eq!(hard.count(), 32);
    let scored = scores(FieldServiceRouting::present(
        &FieldServiceRouting::demo("TURIN_TWO_DAYS").unwrap(),
    ));
    assert_eq!(scored.len(), 64);
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
    let wire = FieldServiceRouting::present(&FieldServiceRouting::demo("TURIN_TWO_DAYS").unwrap());
    let plan = prepared(wire.clone());
    let again = FieldServiceRouting::present(&plan);
    assert_eq!(again["vehicles"].as_array().unwrap().len(), 30);
    assert_eq!(again["visits"], wire["visits"]);
    assert_eq!(again["visitGroups"], wire["visitGroups"]);
    // One visit is already on Luca's itinerary.
    let all = plan.facts.first_break;
    assert_eq!(again["unassignedVisits"].as_array().unwrap().len(), all - 1);
}

#[test]
fn the_demo_is_large_and_uses_every_feature() {
    let demo = FieldServiceRouting::demo("TURIN_TWO_DAYS").unwrap();
    let shifts: usize = demo.vehicles.iter().map(|v| v.shifts.len()).sum();
    assert_eq!(shifts, 60);
    assert!(demo.visits.len() > 200, "{}", demo.visits.len());
    let has = |f: &dyn Fn(&solverforge_workbench_field_service_routing::wire::Visit) -> bool| {
        demo.visits.iter().any(f)
    };
    assert!(has(&|v| !v.visit_dependencies.is_empty()));
    assert!(has(&|v| !v.exclusions.is_empty()));
    assert!(has(&|v| !v.required_tags.is_empty()));
    assert!(has(&|v| !v.required_vehicles.is_empty()));
    assert!(has(&|v| !v.prohibited_vehicles.is_empty()));
    assert!(has(&|v| !v.preferred_vehicles.is_empty()));
    assert!(has(&|v| !v.preferred_time_windows.is_empty()));
    assert!(has(&|v| v.preferred_consecutive_group_id.is_some()));
    assert!(has(&|v| v.latest_sla_end_time.is_some()));
    assert!(has(&|v| v.time_windows.len() == 2));
    assert!(has(&|v| v.profit.is_some()));
    assert!(has(&|v| v
        .priority
        .as_deref()
        .is_some_and(|p| p.starts_with("opt-"))));
    assert!(!demo.visit_groups.is_empty());
    assert!(demo.vehicles.iter().any(|v| !v.linked_shifts.is_empty()));
    assert!(demo.vehicles.iter().any(|v| v.required_area.is_some()));
    assert!(demo.vehicles.iter().any(|v| v.preferred_area.is_some()));
    let breaks: Vec<_> = demo
        .vehicles
        .iter()
        .flat_map(|v| &v.shifts)
        .flat_map(|s| &s.limits.required_breaks)
        .collect();
    assert!(breaks.iter().any(|b| b.location.is_some()));
    assert!(breaks
        .iter()
        .any(|b| b.start_time.is_some() && b.location.is_none()));
}

/// Rules a plan breaks, by name.
fn broken_rules(plan: solverforge_workbench_field_service_routing::Plan) -> Vec<String> {
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
fn a_scrambled_demo_breaks_most_rules() {
    let mut scrambled = demo_plan();
    scramble(&mut scrambled, 5);
    let mut broken = broken_rules(scrambled);
    // Crowded: twelve shifts take every visit in turn, so shifts run long
    // and full, and the first repair stays unplanned after its inspection.
    let mut crowded = demo_plan();
    for shift in &mut crowded.shifts {
        shift.visits.clear();
    }
    let bench = crowded.facts.bench();
    for v in 0..crowded.facts.first_break {
        let s = if crowded.facts.visits[v].id == "REPAIR-1" {
            bench
        } else {
            v % 12
        };
        crowded.shifts[s].visits.push(v);
    }
    broken.extend(broken_rules(crowded));
    // Rules the demo's data cannot break when scrambled: straight-line
    // travel reaches everywhere, nothing is pinned, the off-by-default
    // rules weigh nothing, rewards are earned rather than broken, and the one overnight-capable shift pair is
    // linked rather than set to stay over.
    let needs_other_data: Vec<&str> = RULES
        .iter()
        .filter(|r| r.off_by_default() || r.rewards())
        .map(|r| r.name())
        .chain([
            Rule::ReachableVisit.name(),
            Rule::ReachableItinerary.name(),
            Rule::PinStart.name(),
            Rule::PinShift.name(),
            Rule::PinStartDelay.name(),
            Rule::Overnight.name(),
            Rule::Loops.name(),
            Rule::DependencyPooling.name(),
        ])
        .collect();
    let missing: Vec<&str> = RULES
        .iter()
        .map(|r| r.name())
        .filter(|n| !needs_other_data.contains(n) && !broken.iter().any(|b| b == n))
        .collect();
    assert!(missing.is_empty(), "never broken: {missing:?}");
}

#[test]
fn moving_a_pinned_visit_off_its_shift_costs_ten_billion() {
    let mut wire = dataset(
        vec![
            vehicle("ann", shift("ann-mon", &["a"], json!({}))),
            vehicle("bob", shift("bob-mon", &[], json!({}))),
        ],
        vec![visit("a", 10, json!({}))],
        json!({}),
    );
    wire["vehicles"][0]["shifts"][0]["itinerary"][0] =
        json!({"id": "a", "kind": "VISIT", "pin": true, "startServiceTime": at(0, 8, 0)});
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
        .find(|a| a.constraint_ref.full_name() == Rule::PinShift.name())
        .unwrap();
    assert_eq!(pin.score.to_string(), "-10000000000hard/0medium/0soft");
    assert!(after < before);
}

fn two_shift_plan() -> Value {
    dataset(
        vec![
            vehicle("ann", shift("ann-mon", &[], json!({"tags": ["ladder"]}))),
            vehicle("bob", shift("bob-mon", &[], json!({}))),
        ],
        vec![
            visit("J", 30, json!({"requiredTags": ["ladder"]})),
            visit("K", 30, json!({"location": [45.08, 7.69]})),
        ],
        json!({}),
    )
}

#[test]
fn a_solve_plans_every_visit_on_a_shift_that_fits_it() {
    let (code, done) = solve(two_shift_plan(), 2);
    assert_eq!(code, 0, "{done}");
    assert_eq!(done["feasible"], true, "{}", done["score"]);
    let ids = |v: usize| -> Vec<String> {
        done["plan"]["vehicles"][v]["shifts"][0]["itinerary"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["id"].as_str().unwrap().to_string())
            .collect()
    };
    assert!(ids(0).contains(&"J".to_string()));
    assert_eq!(ids(0).len() + ids(1).len(), 2);
}

/// Every entity a crowded plan's analysis blames has an id and a label.
#[test]
fn every_blamed_entity_has_an_id_and_a_label() {
    let mut wire =
        FieldServiceRouting::present(&FieldServiceRouting::demo("TURIN_TWO_DAYS").unwrap());
    let items: Vec<Value> = wire["visits"]
        .as_array()
        .unwrap()
        .iter()
        .take(30)
        .map(|v| json!({"id": v["id"], "kind": "VISIT"}))
        .collect();
    for vehicle in wire["vehicles"].as_array_mut().unwrap() {
        for shift in vehicle["shifts"].as_array_mut().unwrap() {
            shift["itinerary"] = json!([]);
        }
    }
    wire["vehicles"][0]["shifts"][0]["itinerary"] = Value::Array(items);
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
                let label = entity["label"].as_str().unwrap();
                assert!(!label.is_empty() && !label.contains('{'), "{entity}");
            }
        }
    }
    assert!(blamed > 50, "{blamed}");
}

#[test]
fn fill_ranks_the_shifts_for_an_open_visit_and_applies_one() {
    let (code, answer) = engine(json!({"command": "fill", "plan": two_shift_plan(),
                                       "fill": {"items": ["J"], "top": 2}}));
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
    let itinerary = &applied["plan"]["vehicles"][0]["shifts"][0]["itinerary"];
    assert_eq!(itinerary[0]["id"], "J");
}

#[test]
fn fill_offers_only_the_vehicles_a_visit_may_use() {
    let mut wire = two_shift_plan();
    wire["visits"][1]["prohibitedVehicles"] = json!(["ann"]);
    let (code, answer) = engine(json!({"command": "fill", "plan": wire,
                                       "fill": {"items": ["K"], "top": 5}}));
    assert_eq!(code, 0, "{answer}");
    let options = answer["items"][0]["options"].as_array().unwrap();
    assert_eq!(options.len(), 1);
    assert_eq!(options[0]["id"], "bob-mon");
}

#[test]
fn the_view_draws_routes_on_a_map_and_a_lane_per_shift() {
    let mut wire = two_shift_plan();
    wire["vehicles"][0]["shifts"][0]["itinerary"] =
        json!([{"id": "J", "kind": "VISIT"}, {"id": "K", "kind": "VISIT"}]);
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
fn freezing_pins_the_visits_already_started_and_the_one_on_the_way() {
    let planned = |pin_next: bool| {
        let mut w = two_shift_plan();
        w["vehicles"][0]["shifts"][0]["itinerary"] =
            json!([{"id": "J", "kind": "VISIT"}, {"id": "K", "kind": "VISIT"}]);
        w["parameters"] = json!({"pinNextVisitDuringFreeze": pin_next});
        prepared(w)
    };
    // J runs 08:00–08:30; the drive to K takes a few minutes.
    let mut plan = planned(true);
    assert_eq!(
        FieldServiceRouting::freeze(&mut plan, &at(0, 8, 31)).unwrap(),
        2
    );
    let out = FieldServiceRouting::present(&plan);
    let itinerary = out["vehicles"][0]["shifts"][0]["itinerary"]
        .as_array()
        .unwrap();
    assert!(itinerary.iter().all(|i| i["pin"] == true));
    let mut plan = planned(false);
    assert_eq!(
        FieldServiceRouting::freeze(&mut plan, &at(0, 8, 31)).unwrap(),
        1
    );
}

#[test]
fn the_dataset_is_checked_before_it_is_scored() {
    let base = two_shift_plan;
    let has = |plan: Value, code: &str| {
        let codes = issue_codes(plan);
        assert!(codes.iter().any(|c| c == code), "{code} not in {codes:?}");
    };
    let mut dup = base();
    dup["visits"][1]["id"] = json!("J");
    has(dup, "duplicate_id");
    let mut unknown = base();
    unknown["vehicles"][0]["shifts"][0]["itinerary"] = json!([{"id": "nope", "kind": "VISIT"}]);
    has(unknown, "unknown_itinerary_item");
    let mut twice = base();
    twice["vehicles"][0]["shifts"][0]["itinerary"] = json!([{"id": "J", "kind": "VISIT"}]);
    twice["vehicles"][1]["shifts"][0]["itinerary"] = json!([{"id": "J", "kind": "VISIT"}]);
    has(twice, "assigned_twice");
    let mut kind = base();
    kind["vehicles"][0]["shifts"][0]["itinerary"] = json!([{"id": "J", "kind": "BREAK"}]);
    has(kind, "wrong_item_kind");
    let mut pin = base();
    pin["vehicles"][0]["shifts"][0]["itinerary"] =
        json!([{"id": "J", "kind": "VISIT", "pin": true}]);
    has(pin, "pinned_visit_without_start");
    let mut time = base();
    time["vehicles"][0]["shifts"][0]["minStartTime"] = json!("tomorrow");
    has(time, "bad_time");
    let mut start = base();
    start["vehicles"][0]["shifts"][0]
        .as_object_mut()
        .unwrap()
        .remove("minStartTime");
    has(start, "shift_without_start");
    let mut priority = base();
    priority["visits"][0]["priority"] = json!("urgent");
    has(priority, "unknown_priority");
    let mut matrix = base();
    matrix["travelMatrix"] = json!({"locations": [DEPOT], "seconds": [[0]], "meters": [[0]]});
    has(matrix, "place_not_in_travel_matrix");
    let mut pinned = base();
    pinned["vehicles"][0]["shifts"][0]["pinned"] = json!(true);
    pinned["vehicles"][0]["shifts"][0]["requiredBreaks"] =
        json!([{"id": "L", "type": "FLOATING", "minStartTime": at(0, 11, 0), "duration": 30}]);
    has(pinned, "pinned_shift_misses_break");
    let mut dependency = base();
    dependency["visits"][0]["visitDependencies"] = json!([{"id": "d", "precedingVisit": "nope"}]);
    has(dependency, "unknown_preceding_visit");
    let mut group = base();
    group["visitGroups"] = json!([{"id": "G", "visits": [visit("Ga", 10, json!({}))]}]);
    has(group, "small_visit_group");
    let mut linked = base();
    linked["vehicles"][0]["linkedShifts"] = json!([{"id": "LS", "shifts": [
        {"id": "l1", "fromShiftId": "ann-mon", "toShiftId": "bob-mon"}]}]);
    has(linked, "unknown_linked_shift");
    let mut closed = base();
    closed["visits"][0]["timeWindows"] = json!([{"minStartTime": at(0, 13, 0),
        "maxStartTime": at(0, 11, 0), "maxEndTime": at(0, 17, 0)}]);
    has(closed, "bad_time_window");
}

#[test]
fn the_engine_describes_the_model() {
    let (code, model) = engine(json!({"command": "describe"}));
    assert_eq!(code, 0, "{model}");
    assert_eq!(model["view"], "routes");
    assert_eq!(model["demos"], json!(["TURIN_TWO_DAYS"]));
    assert_eq!(model["constraints"].as_array().unwrap().len(), 64);
    assert_eq!(model["parameters"].as_array().unwrap().len(), 11);
    for p in model["parameters"].as_array().unwrap() {
        assert!(!p["label"].as_str().unwrap().is_empty(), "{p}");
        if ["number", "integer", "duration"].contains(&p["kind"].as_str().unwrap()) {
            assert!(p["unit"].is_string(), "{p} has a unit");
        }
        if let Some(options) = p["options"].as_array() {
            assert!(
                options.iter().any(|o| o["value"] == p["default"]),
                "{p}: its default is an option"
            );
        }
    }

    assert_eq!(model["roads"], true);
}

#[test]
fn travel_follows_the_roads_one_way_streets_and_all() {
    const HOME: [f64; 2] = [45.3101, 8.4101];
    const THERE: [f64; 2] = [45.3201, 8.4101];
    const AROUND: [f64; 2] = [45.3151, 8.4301];
    let plan = || {
        dataset(
            vec![vehicle(
                "ann",
                shift(
                    "ann-mon",
                    &["a"],
                    json!({"startLocation": HOME, "endLocation": HOME}),
                ),
            )],
            vec![visit("a", 10, json!({"location": THERE}))],
            json!({}),
        )
    };
    // Out is one road of 100 s; back is one-way the long way round, 300 s and 400 s.
    provide_complete(&[HOME, THERE, AROUND], |a, b| match (a, b) {
        (HOME, THERE) => Some((100.0, 1200.0)),
        (THERE, AROUND) => Some((300.0, 4000.0)),
        (AROUND, HOME) => Some((400.0, 5000.0)),
        _ => None,
    });
    let (code, out) = engine(
        json!({"command": "evaluate", "plan": plan(), "roads": {"cache_dir": "/nonexistent"}}),
    );
    assert_eq!(code, 0, "{out}");
    let travel: HashMap<String, String> = out["analysis"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["name"].as_str().unwrap().into(),
                c["score"].as_str().unwrap().into(),
            )
        })
        .collect();
    assert_eq!(
        travel[Rule::TravelTime.name()],
        "0hard/0medium/-800soft",
        "{travel:?}"
    );
    // The roads command hands the measured plan back to keep; a plan that
    // carries road travel for every place is not measured again.
    let (code, out) =
        engine(json!({"command": "roads", "plan": plan(), "roads": {"cache_dir": "/nonexistent"}}));
    assert_eq!(code, 0, "{out}");
    let kept = out["plan"].clone();
    assert_eq!(kept["travelMatrix"]["source"], "roads");
    assert_eq!(kept["travelMatrix"]["seconds"][0][1], 100, "{kept}");
    let (code, out) = engine(json!({"command": "evaluate", "plan": kept, "roads": null}));
    assert_eq!(code, 0, "measured travel needs no network: {out}");
    // With no network to read and no cache to keep one in, the engine says so.
    let mut elsewhere = plan();
    for place in ["startLocation", "endLocation"] {
        elsewhere["vehicles"][0]["shifts"][0][place] = json!([46.3101, 8.4101]);
    }
    let (code, out) = engine(json!({"command": "evaluate", "plan": elsewhere, "roads": null}));
    assert_ne!(code, 0);
    assert_eq!(out["error"]["code"], "roads_unavailable", "{out}");
}

#[test]
fn each_demo_summary_states_the_size_the_demo_has() {
    use solverforge_workbench_host::Model;
    type M = solverforge_workbench_field_service_routing::FieldServiceRouting;
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
    let demo = FieldServiceRouting::present(&FieldServiceRouting::demo("TURIN_TWO_DAYS").unwrap());
    let (code, out) = engine(json!({"command": "edits", "plan": demo}));
    assert_eq!(code, 0, "{out}");
    let edits = out["edits"].as_array().unwrap();
    let ids: Vec<&str> = edits.iter().map(|e| e["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["technician_off", "add_visit", "visit_window"]);
    let days: Vec<&str> = edits[0]["fields"][1]["options"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["label"].as_str().unwrap())
        .collect();
    assert_eq!(days, ["Mon 1 Feb", "Tue 2 Feb"]);
    let compile = |id: &str, args: Value| {
        let (code, out) =
            engine(json!({"command": "edit", "plan": demo, "edit": {"id": id, "args": args}}));
        assert_eq!(code, 0, "{out}");
        out
    };
    let off = compile(
        "technician_off",
        json!({"technician": "giorgio", "day": "2027-02-02"}),
    );
    assert_eq!(
        off["operations"],
        json!([
            {"op": "remove", "path": "/vehicles/[id=giorgio]/linkedShifts/[id=giorgio-aosta]"},
            {"op": "remove", "path": "/vehicles/[id=giorgio]/shifts/[id=giorgio-tue]"},
        ])
    );
    assert!(
        off["summary"].as_str().unwrap().starts_with("Giorgio"),
        "{off}"
    );
    let added = compile(
        "add_visit",
        json!({"name": "Leak", "location": [45.07, 7.68], "duration": 45,
               "from": "2027-02-02T09:00", "until": "2027-02-02T13:00", "skill": "plumbing"}),
    );
    let first = demo["visits"][0]["id"].as_str().unwrap().to_string();
    let window = compile(
        "visit_window",
        json!({"visit": first, "from": "2027-02-01T10:00", "until": "2027-02-01T12:00"}),
    );
    let mut changed = demo.clone();
    for compiled in [&off, &added, &window] {
        apply(&mut changed, compiled["operations"].as_array().unwrap());
    }
    assert!(changed["vehicles"]
        .as_array()
        .unwrap()
        .iter()
        .all(|v| v["id"] != "giorgio"
            || v["shifts"]
                .as_array()
                .unwrap()
                .iter()
                .all(|s| s["id"] != "giorgio-tue")));
    assert_eq!(
        changed["visits"].as_array().unwrap().last().unwrap()["id"],
        "NEW-1"
    );
    evaluate(changed);
    let (code, refused) = engine(json!({"command": "edit", "plan": demo,
        "edit": {"id": "technician_off", "args": {"technician": "nobody", "day": "2027-02-02"}}}));
    assert_ne!(code, 0);
    assert_eq!(refused["error"]["code"], "invalid_edit");
}

#[test]
fn the_data_tables_match_the_demo() {
    // Every field of the demo has a column, and every column is a field the
    // model reads and writes back: the data editor can change all of it.
    let demo = FieldServiceRouting::present(&FieldServiceRouting::demo("TURIN_TWO_DAYS").unwrap());
    let tables = FieldServiceRouting::tables();
    let mut problems = solverforge_workbench_host::data::mismatches(&demo, &tables, &[]);
    problems.extend(solverforge_workbench_host::data::unread(
        &demo,
        &tables,
        |plan| {
            serde_json::from_value::<solverforge_workbench_field_service_routing::Plan>(plan)
                .map(|plan| FieldServiceRouting::present(&plan))
                .map_err(|error| error.to_string())
        },
    ));
    assert!(problems.is_empty(), "{problems:#?}");
}
