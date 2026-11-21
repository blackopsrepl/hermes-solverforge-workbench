//! Every rule, scored through the engine on small datasets. Stops sit at
//! the depot unless a test moves them, so travel is zero and times are
//! exact.

use serde_json::{json, Value};

use crate::common::*;

fn hard(n: i64) -> String {
    format!("-{n}hard/0medium/0soft")
}

fn medium(n: i64) -> String {
    format!("0hard/-{n}medium/0soft")
}

fn soft(n: i64) -> String {
    format!("0hard/0medium/-{n}soft")
}

const ZERO: &str = "0hard/0medium/0soft";

/// Ann's Monday shift driving `itinerary`, plus Bob with an empty one.
fn plan(itinerary: &[&str], shift_extra: Value, jobs: Vec<Value>, extra: Value) -> Value {
    dataset(
        vec![
            driver("ann", shift("ann-mon", itinerary, shift_extra)),
            driver("bob", shift("bob-mon", &[], json!({}))),
        ],
        jobs,
        extra,
    )
}

/// A two-stop job `id` with stops `<id>a` and `<id>b` of ten minutes each.
fn pair(id: &str, extra: Value) -> Value {
    job(
        id,
        vec![
            stop(&format!("{id}a"), 10, json!({})),
            stop(&format!("{id}b"), 10, json!({})),
        ],
        extra,
    )
}

#[test]
fn a_stop_served_before_the_stop_it_follows_costs_the_overlap_plus_an_hour() {
    let jobs = vec![pair("J", json!({}))];
    let rule = "Stop served before the stop it follows";
    assert_eq!(
        score(
            plan(&["Jb", "Ja"], json!({}), jobs.clone(), json!({})),
            rule
        ),
        hard(1200 + 3600)
    );
    assert_eq!(
        score(plan(&["Ja", "Jb"], json!({}), jobs, json!({})), rule),
        ZERO
    );
}

#[test]
fn a_job_split_across_shifts_costs_the_stop_plus_an_hour() {
    let wire = dataset(
        vec![
            driver("ann", shift("ann-mon", &["Ja"], json!({}))),
            driver("bob", shift("bob-mon", &["Jb"], json!({}))),
        ],
        vec![pair("J", json!({}))],
        json!({}),
    );
    let all = scores(wire);
    assert_eq!(all["Job split across shifts"], hard(600 + 3600));
    assert_eq!(all["Stop served before the stop it follows"], ZERO);
}

#[test]
fn a_job_planned_in_part_keeps_its_other_stops_on_the_bench_and_breaks_three_rules() {
    let all = scores(plan(
        &["Jb"],
        json!({}),
        vec![pair("J", json!({}))],
        json!({}),
    ));
    assert_eq!(
        all["Stop planned while the stop before it is not"],
        hard(600 + 600 + 3600)
    );
    assert_eq!(all["Job partly planned"], hard(600 + 3600));
    assert_eq!(
        all["Stop served before the stop it follows"],
        hard(600 + 3600)
    );
}

#[test]
fn a_job_planned_nowhere_breaks_no_link_and_only_costs_its_priority() {
    let all = scores(plan(&[], json!({}), vec![pair("J", json!({}))], json!({})));
    assert_eq!(all["Job partly planned"], ZERO);
    assert_eq!(all["Stop planned while the stop before it is not"], ZERO);
    // Built-in priority 6 weighs 10 000 per stop.
    assert_eq!(all["Mandatory stop unplanned"], medium(20_000));
}

#[test]
fn passing_a_riders_destination_without_dropping_them_off_costs_an_hour() {
    let (y, z) = (json!([45.49, 9.24]), json!([45.5, 9.25]));
    let jobs = vec![
        job(
            "J1",
            vec![
                stop("a", 10, json!({})),
                stop("b", 10, json!({"location": y})),
            ],
            json!({}),
        ),
        job(
            "J2",
            vec![
                stop("c", 10, json!({"location": y})),
                stop("d", 10, json!({"location": z})),
            ],
            json!({}),
        ),
    ];
    let rule = "Ride past a destination";
    assert_eq!(
        score(
            plan(&["a", "c", "b", "d"], json!({}), jobs.clone(), json!({})),
            rule
        ),
        hard(3600)
    );
    assert_eq!(
        score(
            plan(&["a", "b", "c", "d"], json!({}), jobs, json!({})),
            rule
        ),
        ZERO
    );
}

#[test]
fn a_load_over_capacity_costs_an_hour_per_unit_and_stop() {
    let seat = json!({"demands": [{"type": "seats", "demand": 1}]});
    let jobs = vec![pair("A", seat.clone()), pair("B", seat)];
    let cap = json!({"capacities": [{"type": "seats", "quantity": 1}]});
    let rule = "Vehicle capacity exceeded";
    assert_eq!(
        score(
            plan(
                &["Aa", "Ba", "Ab", "Bb"],
                cap.clone(),
                jobs.clone(),
                json!({})
            ),
            rule
        ),
        hard(3600)
    );
    assert_eq!(
        score(plan(&["Aa", "Ab", "Ba", "Bb"], cap, jobs, json!({})), rule),
        ZERO
    );
}

#[test]
fn driver_requirements_cost_per_stop() {
    let preferred = vec![pair("J", json!({"preferredDrivers": ["bob"]}))];
    assert_eq!(
        score(
            plan(&["Ja", "Jb"], json!({}), preferred, json!({})),
            "Preferred driver not assigned"
        ),
        soft(2)
    );
    let required = vec![pair("J", json!({"requiredDrivers": ["bob"]}))];
    assert_eq!(
        score(
            plan(&["Ja", "Jb"], json!({}), required, json!({})),
            "Driver not among the required"
        ),
        hard(2 * 3600)
    );
    let prohibited = vec![pair("J", json!({"prohibitedDrivers": ["ann"]}))];
    assert_eq!(
        score(
            plan(&["Ja", "Jb"], json!({}), prohibited, json!({})),
            "Prohibited driver assigned"
        ),
        hard(2 * 3600)
    );
    let fine = vec![pair(
        "J",
        json!({"requiredDrivers": ["ann"], "preferredDrivers": ["ann"]}),
    )];
    let all = scores(plan(&["Ja", "Jb"], json!({}), fine, json!({})));
    assert_eq!(all["Driver not among the required"], ZERO);
    assert_eq!(all["Preferred driver not assigned"], ZERO);
}

#[test]
fn pooling_rules_cost_the_seconds_two_jobs_ride_together() {
    let pooled = &["J1a", "J2a", "J2b", "J1b"];
    let burden = vec![
        pair("J1", json!({"maximumTimeBurden": 0})),
        pair("J2", json!({})),
    ];
    assert_eq!(
        score(
            plan(pooled, json!({}), burden.clone(), json!({})),
            "Time burden over the maximum"
        ),
        hard(1200)
    );
    assert_eq!(
        score(
            plan(&["J1a", "J1b", "J2a", "J2b"], json!({}), burden, json!({})),
            "Time burden over the maximum"
        ),
        ZERO
    );
    let prohibited = vec![
        pair("J1", json!({"prohibitedJobs": ["J2"]})),
        pair("J2", json!({})),
    ];
    assert_eq!(
        score(
            plan(pooled, json!({}), prohibited, json!({})),
            "Prohibited jobs on board together"
        ),
        hard(600)
    );
    let alone = vec![
        pair("J1", json!({"prohibitJobPooling": true})),
        pair("J2", json!({})),
    ];
    assert_eq!(
        score(
            plan(pooled, json!({}), alone, json!({})),
            "Pooled with a job that forbids pooling"
        ),
        hard(600)
    );
}

#[test]
fn missing_tags_and_skills_cost_an_hour_each_and_extra_skill_costs_soft() {
    let tags = vec![pair("J", json!({"requiredTags": ["lift", "cold"]}))];
    assert_eq!(
        score(
            plan(&["Ja", "Jb"], json!({"tags": ["lift"]}), tags, json!({})),
            "Required tags missing"
        ),
        hard(2 * 3600)
    );
    let skill = vec![pair(
        "J",
        json!({"requiredSkills": [{"name": "x", "minLevel": 2}]}),
    )];
    assert_eq!(
        score(
            plan(&["Ja", "Jb"], json!({}), skill.clone(), json!({})),
            "Required skills missing"
        ),
        hard(2 * 3 * 3600)
    );
    assert_eq!(
        score(
            plan(
                &["Ja", "Jb"],
                json!({"skills": [{"name": "x", "level": 1}]}),
                skill,
                json!({})
            ),
            "Required skills missing"
        ),
        hard(2 * 3600)
    );
    let low = vec![pair(
        "J",
        json!({"requiredSkills": [{"name": "x", "minLevel": 1}]}),
    )];
    let all = scores(plan(
        &["Ja", "Jb"],
        json!({"skills": [{"name": "x", "level": 3}]}),
        low,
        json!({}),
    ));
    assert_eq!(all["Driver more skilled than needed"], soft(2 * 2 * 600));
    assert_eq!(all["Required skills missing"], ZERO);
}

#[test]
fn a_skill_multiplier_scales_the_service() {
    let jobs = vec![pair("J", json!({"requiredSkills": [{"name": "x"}]}))];
    let wire = plan(
        &["Ja", "Jb"],
        json!({"skills": [{"name": "x", "multiplier": 0.5}], "maxLastStopDepartureTime": at(0, 8, 0)}),
        jobs,
        json!({}),
    );
    // Two five-minute stops end at 08:10.
    assert_eq!(
        score(wire, "Last stop departs after the latest time"),
        hard(600)
    );
}

fn with_break(itinerary: &[&str], brk: Value, jobs: Vec<Value>) -> Value {
    plan(itinerary, json!({"requiredBreaks": [brk]}), jobs, json!({}))
}

#[test]
fn breaks_that_run_late_cost_the_seconds_late() {
    let one = || vec![job("J", vec![stop("a", 10, json!({}))], json!({}))];
    let floating = json!({"id": "L", "type": "FLOATING", "minStartTime": at(0, 8, 0),
                          "maxEndTime": at(0, 8, 20), "duration": 30});
    assert_eq!(
        score(
            with_break(&["a", "brk:L"], floating, one()),
            "Floating break ends too late"
        ),
        hard(1200)
    );
    let fixed =
        json!({"id": "F", "type": "FIXED", "startTime": at(0, 8, 5), "endTime": at(0, 8, 20)});
    assert_eq!(
        score(
            with_break(&["a", "brk:F"], fixed.clone(), one()),
            "Late for a fixed break"
        ),
        hard(300)
    );
    assert_eq!(
        score(
            with_break(&["brk:F", "a"], fixed, one()),
            "Late for a fixed break"
        ),
        ZERO
    );
    let located = json!({"id": "D", "type": "FIXED", "location": DEPOT,
                         "startTime": at(0, 8, 5), "endTime": at(0, 8, 20)});
    let all = scores(with_break(&["a", "brk:D"], located, one()));
    assert_eq!(all["Late for an appointment"], hard(300));
    assert_eq!(all["Late for a fixed break"], ZERO);
}

#[test]
fn breaks_while_a_job_is_on_board_cost_the_break_where_forbidden() {
    let fixed =
        json!({"id": "F", "type": "FIXED", "startTime": at(0, 8, 10), "endTime": at(0, 8, 20)});
    let strict = vec![pair("J", json!({"allowInterruption": "NONE"}))];
    let all = scores(with_break(&["Ja", "brk:F", "Jb"], fixed.clone(), strict));
    assert_eq!(all["Fixed break while a job is on board"], hard(600));
    assert_eq!(all["Late for a fixed break"], ZERO);
    let relaxed = vec![pair("J", json!({}))];
    assert_eq!(
        score(
            with_break(&["Ja", "brk:F", "Jb"], fixed, relaxed.clone()),
            "Fixed break while a job is on board"
        ),
        ZERO
    );
    let rejecting = json!({"id": "L", "type": "FLOATING", "policy": "REJECT_JOBS",
                           "minStartTime": at(0, 8, 10), "duration": 10});
    assert_eq!(
        score(
            with_break(&["Ja", "brk:L", "Jb"], rejecting, relaxed),
            "Floating break while a job is on board"
        ),
        hard(600)
    );
}

#[test]
fn a_movable_stop_served_a_day_late_costs_a_thousand() {
    let windows = json!({"timeWindows": [
        {"minStartTime": at(0, 8, 0), "maxEndTime": at(0, 9, 0)},
        {"minStartTime": at(1, 8, 0), "maxEndTime": at(1, 17, 0)}]});
    let jobs = vec![job("J", vec![stop("a", 10, windows)], json!({}))];
    let tuesday = json!({"minStartTime": at(1, 8, 0)});
    assert_eq!(
        score(
            plan(&["a"], tuesday, jobs.clone(), json!({})),
            "Stop later than its earliest day"
        ),
        soft(1000)
    );
    assert_eq!(
        score(
            plan(&["a"], json!({}), jobs, json!({})),
            "Stop later than its earliest day"
        ),
        ZERO
    );
}

#[test]
fn a_movable_stop_counts_its_days_from_the_first_shift_that_may_serve_it() {
    let windows = json!({"timeWindows": [
        {"minStartTime": at(0, 8, 0), "maxEndTime": at(0, 9, 0)},
        {"minStartTime": at(1, 8, 0), "maxEndTime": at(1, 17, 0)}]});
    let tuesday = || shift("ann-tue", &["a"], json!({"minStartTime": at(1, 8, 0)}));
    // No shift works Monday: Tuesday is the earliest day it can be served.
    let only_tuesday = dataset(
        vec![driver("ann", tuesday())],
        vec![job("J", vec![stop("a", 10, windows.clone())], json!({}))],
        json!({}),
    );
    assert_eq!(
        score(only_tuesday, "Stop later than its earliest day"),
        ZERO
    );
    // Bob works Monday, but the job may not go to him.
    let bob_barred = dataset(
        vec![
            driver("bob", shift("bob-mon", &[], json!({}))),
            driver("ann", tuesday()),
        ],
        vec![job(
            "J",
            vec![stop("a", 10, windows)],
            json!({"prohibitedDrivers": ["bob"]}),
        )],
        json!({}),
    );
    assert_eq!(score(bob_barred, "Stop later than its earliest day"), ZERO);
}

#[test]
fn an_optional_job_left_unplanned_costs_soft() {
    let jobs = vec![pair("J", json!({"priority": "opt-6"}))];
    let all = scores(plan(&[], json!({}), jobs, json!({})));
    assert_eq!(all["Optional stop unplanned"], soft(20_000));
    assert_eq!(all["Mandatory stop unplanned"], ZERO);
}

#[test]
fn custom_priorities_weigh_unplanned_stops() {
    let jobs = vec![pair("J", json!({"priority": "VIP"}))];
    let extra = json!({"priorityWeights": [{"priority": "VIP", "weight": 7}]});
    assert_eq!(
        score(
            plan(&[], json!({}), jobs, extra),
            "Mandatory stop unplanned"
        ),
        medium(14)
    );
}

#[test]
fn a_pinned_stop_starting_after_its_kept_start_costs_a_hundred_thousand_per_second() {
    let mut wire = plan(
        &["a"],
        json!({}),
        vec![job("J", vec![stop("a", 10, json!({}))], json!({}))],
        json!({}),
    );
    wire["drivers"][0]["shifts"][0]["itinerary"][0] =
        json!({"id": "a", "kind": "STOP", "pin": true, "startServiceTime": at(0, 7, 50)});
    assert_eq!(
        score(wire.clone(), "Pinned stop starts late"),
        hard(600 * 100_000)
    );
    wire["drivers"][0]["shifts"][0]["itinerary"][0]["startServiceTime"] = json!(at(0, 8, 30));
    let all = scores(wire);
    // The stop waits for its kept start instead.
    assert_eq!(all["Pinned stop starts late"], ZERO);
    assert_eq!(all["Pinned stop off its shift"], ZERO);
}

fn matrix(seconds: Value) -> Value {
    json!({"travelMatrix": {"locations": [DEPOT, [45.5, 9.3]], "seconds": seconds.clone(), "meters": seconds}})
}

#[test]
fn unreachable_legs_cost_an_hour_each_and_travel_costs_its_seconds() {
    let far = || {
        vec![job(
            "J",
            vec![stop("a", 10, json!({"location": [45.5, 9.3]}))],
            json!({}),
        )]
    };
    let cut = matrix(json!([[0, null], [null, 0]]));
    let all = scores(plan(&["a"], json!({}), far(), cut));
    assert_eq!(all["Unreachable leg in a route"], hard(2 * 3600));
    assert_eq!(all["Stop unreachable from the previous place"], hard(3600));
    let roads = matrix(json!([[0, 600], [900, 0]]));
    let all = scores(plan(&["a"], json!({}), far(), roads));
    assert_eq!(all["Travel time"], soft(1500));
    assert_eq!(all["Unreachable leg in a route"], ZERO);
}

#[test]
fn waiting_is_off_unless_weighted() {
    let jobs = vec![
        job("A", vec![stop("a", 10, json!({}))], json!({})),
        job(
            "B",
            vec![stop(
                "b",
                10,
                json!({"timeWindows": [{"minStartTime": at(0, 9, 0), "maxEndTime": at(0, 10, 0)}]}),
            )],
            json!({}),
        ),
    ];
    assert_eq!(
        score(
            plan(&["a", "b"], json!({}), jobs.clone(), json!({})),
            "Waiting time"
        ),
        ZERO
    );
    let weighted = json!({"weights": {"Waiting time": 1}});
    assert_eq!(
        score(plan(&["a", "b"], json!({}), jobs, weighted), "Waiting time"),
        soft(3000)
    );
}

#[test]
fn a_missed_sla_costs_the_seconds_late_planned_or_not() {
    let sla = |t: String| {
        vec![job(
            "J",
            vec![stop("a", 10, json!({"latestSlaEndTime": t}))],
            json!({}),
        )]
    };
    assert_eq!(
        score(
            plan(&["a"], json!({}), sla(at(0, 8, 5)), json!({})),
            "SLA missed"
        ),
        soft(300)
    );
    let window = json!({"planningWindow": {"startDate": at(0, 0, 0), "endDate": at(1, 0, 0)}});
    assert_eq!(
        score(
            plan(&[], json!({}), sla(at(0, 20, 0)), window),
            "SLA missed"
        ),
        soft(4 * 3600)
    );
}

#[test]
fn shift_hours_cost_the_seconds_late() {
    let jobs = || vec![job("J", vec![stop("a", 10, json!({}))], json!({}))];
    let limits = json!({
        "maxLastStopDepartureTime": at(0, 8, 5), "maxSoftLastStopDepartureTime": at(0, 8, 4),
        "maxEndTime": at(0, 8, 5), "maxSoftEndTime": at(0, 8, 3)});
    let all = scores(plan(&["a"], limits, jobs(), json!({})));
    assert_eq!(all["Last stop departs after the latest time"], hard(300));
    assert_eq!(all["Shift ends after the latest time"], hard(300));
    assert_eq!(all["Last stop departs after the preferred time"], soft(360));
    assert_eq!(all["Shift ends after the preferred time"], soft(420));
}

#[test]
fn service_outside_its_window_costs_the_seconds_late() {
    let tight = vec![job(
        "J",
        vec![stop(
            "a",
            10,
            json!({"timeWindows": [{"minStartTime": at(0, 8, 0), "maxEndTime": at(0, 8, 5)}]}),
        )],
        json!({}),
    )];
    assert_eq!(
        score(
            plan(&["a"], json!({}), tight, json!({})),
            "Service ends after its window"
        ),
        hard(300)
    );
    let start = vec![job(
        "J",
        vec![stop(
            "a",
            10,
            json!({"timeWindows": [{"minStartTime": at(0, 7, 0),
            "maxStartTime": at(0, 7, 55), "maxEndTime": at(0, 9, 0)}]}),
        )],
        json!({}),
    )];
    let all = scores(plan(&["a"], json!({}), start, json!({})));
    assert_eq!(all["Service starts after its window"], hard(300));
    assert_eq!(all["Service ends after its window"], ZERO);
}

#[test]
fn a_stop_takes_its_first_window_that_still_fits() {
    let jobs = vec![job(
        "J",
        vec![stop(
            "a",
            10,
            json!({"timeWindows": [
            {"minStartTime": at(0, 7, 0), "maxEndTime": at(0, 7, 30)},
            {"minStartTime": at(0, 9, 0), "maxEndTime": at(0, 10, 0)}]}),
        )],
        json!({}),
    )];
    let wire = plan(
        &["a"],
        json!({"maxLastStopDepartureTime": at(0, 9, 0)}),
        jobs,
        json!({}),
    );
    let all = scores(wire);
    // Too late for the 07:00 window, so it waits for 09:00 and leaves 09:10.
    assert_eq!(all["Service ends after its window"], ZERO);
    assert_eq!(all["Last stop departs after the latest time"], hard(600));
}

#[test]
fn each_driver_used_costs_an_hour_when_weighed() {
    let weights = json!({"weights": {"Driver used": 1}});
    let jobs = || vec![pair("J", json!({})), pair("K", json!({}))];
    let one = plan(
        &["Ja", "Jb", "Ka", "Kb"],
        json!({}),
        jobs(),
        weights.clone(),
    );
    assert_eq!(score(one, "Driver used"), soft(3600));
    let mut two = plan(&["Ja", "Jb"], json!({}), jobs(), weights);
    two["drivers"][1]["shifts"][0]["itinerary"] = json!([
        {"id": "Ka", "kind": "STOP"}, {"id": "Kb", "kind": "STOP"}
    ]);
    assert_eq!(score(two, "Driver used"), soft(2 * 3600));
    // Off unless weighed.
    let quiet = plan(&["Ja", "Jb"], json!({}), jobs(), json!({}));
    assert_eq!(score(quiet, "Driver used"), ZERO);
}

#[test]
fn work_shared_evenly_between_drivers_costs_least() {
    let weights = json!({"weights": {"Work unbalanced between drivers": 1}});
    let jobs = || vec![pair("J", json!({})), pair("K", json!({}))];
    // Ann alone works 40 minutes: 1600.
    let alone = plan(
        &["Ja", "Jb", "Ka", "Kb"],
        json!({}),
        jobs(),
        weights.clone(),
    );
    assert_eq!(score(alone, "Work unbalanced between drivers"), soft(1600));
    // Twenty minutes each: 400 + 400.
    let mut shared = plan(&["Ja", "Jb"], json!({}), jobs(), weights);
    shared["drivers"][1]["shifts"][0]["itinerary"] = json!([
        {"id": "Ka", "kind": "STOP"}, {"id": "Kb", "kind": "STOP"}
    ]);
    assert_eq!(score(shared, "Work unbalanced between drivers"), soft(800));
}

#[test]
fn stops_use_up_the_parts_their_shift_loads_in_route_order() {
    let jobs = vec![
        job(
            "J",
            vec![
                stop("Ja", 10, json!({})),
                stop(
                    "Jb",
                    10,
                    json!({"parts": [{"part": "meter", "quantity": 2}]}),
                ),
            ],
            json!({}),
        ),
        job(
            "K",
            vec![
                stop("Ka", 10, json!({})),
                stop(
                    "Kb",
                    10,
                    json!({"parts": [{"part": "meter", "quantity": 1}]}),
                ),
            ],
            json!({}),
        ),
    ];
    let route = ["Ja", "Jb", "Ka", "Kb"];
    let two = json!({"stock": [{"part": "meter", "quantity": 2}]});
    assert_eq!(
        score(
            plan(&route, two, jobs.clone(), json!({})),
            "Parts out of stock"
        ),
        hard(3600)
    );
    let three = json!({"stock": [{"part": "meter", "quantity": 3}]});
    assert_eq!(
        score(
            plan(&route, three, jobs.clone(), json!({})),
            "Parts out of stock"
        ),
        ZERO
    );
    assert_eq!(
        score(
            plan(&route, json!({}), jobs, json!({})),
            "Parts out of stock"
        ),
        hard(3 * 3600)
    );
}
