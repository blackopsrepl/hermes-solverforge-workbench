//! Every rule, scored through the engine on small datasets. Visits sit at
//! the depot unless a test moves them, so travel is zero and times are
//! exact; tests that need travel give a travel matrix.

use serde_json::{json, Value};
use solverforge_workbench_field_service_routing::constraints::Rule;

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

fn reward(n: i64) -> String {
    format!("0hard/0medium/{n}soft")
}

const ZERO: &str = "0hard/0medium/0soft";

/// A place a kilometre north of the depot.
const AWAY: [f64; 2] = [45.0793, 7.6869];

/// Ann's Monday shift driving `itinerary`, plus Bob with an empty one.
fn plan(itinerary: &[&str], shift_extra: Value, visits: Vec<Value>, extra: Value) -> Value {
    dataset(
        vec![
            vehicle("ann", shift("ann-mon", itinerary, shift_extra)),
            vehicle("bob", shift("bob-mon", &[], json!({}))),
        ],
        visits,
        extra,
    )
}

fn rule(plan: Value, rule: Rule) -> String {
    score(plan, rule.name())
}

/// Weighs an off-by-default rule on.
fn weighed(rule: Rule) -> Value {
    json!({"weights": {rule.name(): 1}})
}

/// Ten minutes each way between the depot and [`AWAY`], five kilometres;
/// `null` legs when `cut`.
fn matrix(cut: bool) -> Value {
    let leg = |v: i64| if cut { Value::Null } else { json!(v) };
    json!({"travelMatrix": {
        "locations": [DEPOT, AWAY],
        "seconds": [[0, leg(600)], [leg(600), 0]],
        "meters": [[0, leg(5000)], [leg(5000), 0]],
    }})
}

fn window(from: (i64, i64), to: (i64, i64)) -> Value {
    json!({"minStartTime": at(0, from.0, from.1), "maxEndTime": at(0, to.0, to.1)})
}

// Priority and optional visits.

#[test]
fn an_unplanned_visit_costs_its_priority_weight() {
    let mandatory = vec![visit("a", 10, json!({"priority": "3"}))];
    assert_eq!(
        rule(
            plan(&[], json!({}), mandatory.clone(), json!({})),
            Rule::MandatoryUnplanned
        ),
        medium(10_000_000)
    );
    assert_eq!(
        rule(
            plan(&["a"], json!({}), mandatory, json!({})),
            Rule::MandatoryUnplanned
        ),
        ZERO
    );
    let optional = vec![visit("a", 10, json!({"priority": "opt-2"}))];
    assert_eq!(
        rule(
            plan(&[], json!({}), optional, json!({})),
            Rule::OptionalUnplanned
        ),
        soft(100_000_000)
    );
    let custom = vec![visit("a", 10, json!({"priority": "vip"}))];
    let weights = json!({"priorityWeights": [
        {"priority": "vip", "weight": 7, "assignment": "OPTIONAL"}]});
    assert_eq!(
        rule(
            plan(&[], json!({}), custom, weights),
            Rule::OptionalUnplanned
        ),
        soft(7)
    );
}

#[test]
fn a_planned_visit_earns_its_profit() {
    let visits = vec![visit("a", 10, json!({"profit": 12.5}))];
    assert_eq!(
        rule(plan(&["a"], json!({}), visits, json!({})), Rule::Profit),
        reward(1250)
    );
}

// Requirements, tags and skills.

#[test]
fn a_missing_tag_costs_an_hour_unless_held_then() {
    let visits = || vec![visit("a", 10, json!({"requiredTags": ["ladder"]}))];
    let tags = |extra: Value| rule(plan(&["a"], extra, visits(), json!({})), Rule::TagsMissing);
    assert_eq!(tags(json!({})), hard(3600));
    assert_eq!(tags(json!({"tags": ["ladder"]})), ZERO);
    let lent = |from: i64, to: i64| json!({"temporaryTagSets": [{"start": at(0, from, 0), "end": at(0, to, 0), "tags": ["ladder"]}]});
    assert_eq!(tags(lent(7, 9)), ZERO);
    assert_eq!(tags(lent(9, 10)), hard(3600));
}

#[test]
fn skills_cost_an_hour_per_missing_level_and_soft_per_spare_level() {
    let visits = || {
        vec![visit(
            "a",
            10,
            json!({"requiredSkills": [{"name": "gas", "minLevel": 2}]}),
        )]
    };
    let with = |level: Option<i64>, r: Rule| {
        let skills = match level {
            Some(l) => json!({"skills": [{"name": "gas", "level": l}]}),
            None => json!({}),
        };
        rule(plan(&["a"], skills, visits(), json!({})), r)
    };
    assert_eq!(with(None, Rule::SkillsMissing), hard(7200));
    assert_eq!(with(Some(1), Rule::SkillsMissing), hard(3600));
    assert_eq!(with(Some(2), Rule::SkillsMissing), ZERO);
    assert_eq!(with(Some(2), Rule::UnnecessarySkills), ZERO);
    assert_eq!(with(Some(4), Rule::UnnecessarySkills), soft(2 * 600));
    let temporary = json!({"temporarySkillSets": [{"start": at(0, 7, 0), "end": at(0, 9, 0),
                                                   "skills": [{"name": "gas", "level": 2}]}]});
    assert_eq!(
        rule(
            plan(&["a"], temporary, visits(), json!({})),
            Rule::SkillsMissing
        ),
        ZERO
    );
}

#[test]
fn a_skill_multiplier_scales_its_part_of_the_service() {
    let visits = vec![visit(
        "a",
        20,
        json!({"serviceDurationBreakdown": [{"skill": "gas", "duration": 10}],
               "timeWindows": [window((8, 0), (8, 20))]}),
    )];
    let skills = json!({"skills": [{"name": "gas", "level": 1, "multiplier": 1.5}]});
    // Ten skilled minutes take fifteen: the visit ends at 08:25.
    assert_eq!(
        rule(plan(&["a"], skills, visits, json!({})), Rule::WindowEnd),
        hard(300)
    );
}

#[test]
fn vehicle_requirements_cost_an_hour_and_preferences_reward_the_service() {
    let one = |extra: Value, r: Rule| {
        rule(
            plan(&["a"], json!({}), vec![visit("a", 10, extra)], json!({})),
            r,
        )
    };
    assert_eq!(
        one(
            json!({"requiredVehicles": ["bob"]}),
            Rule::NotRequiredVehicle
        ),
        hard(3600)
    );
    assert_eq!(
        one(
            json!({"requiredVehicles": ["ann"]}),
            Rule::NotRequiredVehicle
        ),
        ZERO
    );
    assert_eq!(
        one(
            json!({"prohibitedVehicles": ["ann"]}),
            Rule::ProhibitedVehicle
        ),
        hard(3600)
    );
    assert_eq!(
        one(
            json!({"preferredVehicles": ["ann"]}),
            Rule::PreferredVehicle
        ),
        reward(600)
    );
    assert_eq!(
        one(
            json!({"preferredVehiclesWeights": [{"id": "ann", "weight": 3}]}),
            Rule::PreferredVehicle
        ),
        reward(1800)
    );
}

#[test]
fn coverage_areas_cost_per_visit_outside_them() {
    let elsewhere = json!([[[46.0, 8.0], [46.1, 8.0], [46.1, 8.1], [46.0, 8.1]]]);
    let around = json!([[[45.0, 7.6], [45.2, 7.6], [45.2, 7.8], [45.0, 7.8]]]);
    let with = |area: &str, polygon: &Value, r: Rule| {
        let mut wire = plan(
            &["a"],
            json!({}),
            vec![visit("a", 10, json!({}))],
            json!({}),
        );
        wire["vehicles"][0][area] = polygon.clone();
        rule(wire, r)
    };
    assert_eq!(
        with("requiredArea", &elsewhere, Rule::OutsideArea),
        hard(3600)
    );
    assert_eq!(with("requiredArea", &around, Rule::OutsideArea), ZERO);
    assert_eq!(
        with("preferredArea", &elsewhere, Rule::OutsidePreferredArea),
        soft(1)
    );
}

#[test]
fn a_better_rated_technician_earns_per_service_second() {
    let mut wire = plan(
        &["a"],
        json!({}),
        vec![visit("a", 10, json!({}))],
        weighed(Rule::Rating),
    );
    wire["vehicles"][0]["technicianRating"] = json!(4.5);
    assert_eq!(rule(wire, Rule::Rating), reward(2700));
}

// Time windows and service level agreements.

#[test]
fn windows_cost_the_seconds_late_to_start_or_end() {
    let one = |w: Value, r: Rule| {
        let visits = vec![visit("a", 30, json!({"timeWindows": [w]}))];
        rule(plan(&["a"], json!({}), visits, json!({})), r)
    };
    assert_eq!(one(window((8, 0), (8, 15)), Rule::WindowEnd), hard(900));
    assert_eq!(one(window((8, 0), (9, 0)), Rule::WindowEnd), ZERO);
    let mut late_start = window((7, 0), (9, 0));
    late_start["maxStartTime"] = json!(at(0, 7, 50));
    assert_eq!(one(late_start, Rule::WindowStart), hard(600));
}

#[test]
fn a_visit_waits_for_its_window_and_the_second_window_serves_when_the_first_cannot() {
    let visits = vec![visit(
        "a",
        30,
        json!({"timeWindows": [window((7, 0), (8, 20)), window((9, 0), (10, 0))]}),
    )];
    let wire = plan(&["a"], json!({}), visits, weighed(Rule::Waiting));
    assert_eq!(rule(wire.clone(), Rule::WindowEnd), ZERO);
    // Set off just in time: no waiting on the first visit.
    assert_eq!(rule(wire, Rule::Waiting), ZERO);
    let both = vec![
        visit("a", 10, json!({})),
        visit("b", 10, json!({"timeWindows": [window((9, 0), (10, 0))]})),
    ];
    assert_eq!(
        rule(
            plan(&["a", "b"], json!({}), both, weighed(Rule::Waiting)),
            Rule::Waiting
        ),
        soft(3000)
    );
}

#[test]
fn a_preferred_window_costs_the_service_seconds_outside_it() {
    let visits = vec![visit(
        "a",
        10,
        json!({"preferredTimeWindows": [window((8, 5), (9, 0))]}),
    )];
    assert_eq!(
        rule(
            plan(&["a"], json!({}), visits, json!({})),
            Rule::PreferredWindow
        ),
        soft(300)
    );
}

#[test]
fn service_level_agreements_cost_lateness_and_reward_earliness() {
    let visits = || vec![visit("a", 10, json!({"latestSlaEndTime": at(0, 8, 5)}))];
    assert_eq!(
        rule(plan(&["a"], json!({}), visits(), json!({})), Rule::SlaEnd),
        soft(300)
    );
    // Ends at 08:10 of an SLA at 08:05, counted from the window's midnight.
    assert_eq!(
        rule(
            plan(&["a"], json!({}), visits(), json!({})),
            Rule::SlaEarlier
        ),
        soft(1000 * (8 * 3600 + 600) / (8 * 3600 + 300))
    );
    let window = json!({"planningWindow": {"startDate": at(0, 0, 0), "endDate": at(1, 0, 0)}});
    let mut optional = visits();
    optional[0]["priority"] = json!("opt-5");
    let mut weights = weighed(Rule::OptionalSla);
    merge(&mut weights, window.clone());
    assert_eq!(
        rule(plan(&[], json!({}), optional, weights), Rule::OptionalSla),
        soft(1000 * 86_400 / (8 * 3600 + 300))
    );
    let mut weights = weighed(Rule::MandatorySla);
    merge(&mut weights, window);
    assert_eq!(
        rule(plan(&[], json!({}), visits(), weights), Rule::MandatorySla),
        medium(1000 * 86_400 / (8 * 3600 + 300))
    );
}

#[test]
fn an_important_visit_ending_near_the_shift_end_risks_completion() {
    let visits = vec![visit("a", 10, json!({}))];
    let params = json!({"parameters": {"visitCompletionRiskMinimalTimeToShiftEnd": 60}});
    assert_eq!(
        rule(
            plan(&["a"], json!({"maxEndTime": at(0, 9, 0)}), visits, params),
            Rule::CompletionRisk
        ),
        soft(600)
    );
}

// Breaks.

#[test]
fn a_floating_break_ending_late_costs_the_seconds_late() {
    let lunch = json!({"requiredBreaks": [{"id": "L", "type": "FLOATING", "minStartTime": at(0, 8, 0),
                                           "maxEndTime": at(0, 8, 20), "duration": 15}]});
    let visits = || vec![visit("a", 10, json!({}))];
    assert_eq!(
        rule(
            plan(&["a", "brk:L"], lunch.clone(), visits(), json!({})),
            Rule::FloatingBreakLate
        ),
        hard(300)
    );
    assert_eq!(
        rule(
            plan(&["brk:L", "a"], lunch, visits(), json!({})),
            Rule::FloatingBreakLate
        ),
        ZERO
    );
}

#[test]
fn work_running_into_a_fixed_break_costs_the_overlap() {
    let fixed = |location: Option<[f64; 2]>| {
        let mut b =
            json!({"id": "F", "type": "FIXED", "startTime": at(0, 8, 5), "endTime": at(0, 8, 15)});
        if let Some(p) = location {
            b["location"] = json!(p);
        }
        json!({"requiredBreaks": [b]})
    };
    let visits = || vec![visit("a", 10, json!({}))];
    assert_eq!(
        rule(
            plan(&["a", "brk:F"], fixed(None), visits(), json!({})),
            Rule::FixedBreakConflict
        ),
        hard(300)
    );
    let located = plan(&["a", "brk:F"], fixed(Some(AWAY)), visits(), matrix(false));
    assert_eq!(rule(located.clone(), Rule::LocatedBreakConflict), hard(300));
    // The ten-minute drive there makes it later still.
    assert_eq!(rule(located, Rule::FixedBreakDelayed), hard(600));
}

// Dependencies.

/// `E` depends on `P` with `dep`; Ann drives `itinerary`, Bob `bob`.
fn depending(itinerary: &[&str], bob: &[&str], dep: Value, extra: Value) -> Value {
    let mut dependency = json!({"id": "P-E", "precedingVisit": "P"});
    merge(&mut dependency, dep);
    dataset(
        vec![
            vehicle("ann", shift("ann-mon", itinerary, json!({}))),
            vehicle("bob", shift("bob-mon", bob, json!({}))),
        ],
        vec![
            visit("P", 10, json!({})),
            visit("X", 40, json!({})),
            visit("E", 10, json!({"visitDependencies": [dependency]})),
        ],
        extra,
    )
}

#[test]
fn a_dependent_visit_waits_its_minimum_delay_and_pays_past_its_maximum() {
    // P ends 08:10; E may start 08:40 at the earliest.
    assert_eq!(
        rule(
            depending(
                &["P", "E"],
                &[],
                json!({"minDelay": 30}),
                weighed(Rule::Waiting)
            ),
            Rule::Waiting
        ),
        soft(1800)
    );
    // P ends 08:10, X runs to 08:50; E had to start by 08:40.
    assert_eq!(
        rule(
            depending(&["P", "X", "E"], &[], json!({"maxDelay": 30}), json!({})),
            Rule::DependencyDelay
        ),
        hard(600)
    );
}

#[test]
fn coordination_costs_the_dependent_service_plus_an_hour() {
    for (coordination, r) in [
        ("SAME_VEHICLE", Rule::DependencySameVehicle),
        ("SAME_SHIFT", Rule::DependencySameShift),
    ] {
        let dep = json!({"coordination": coordination});
        assert_eq!(
            rule(depending(&["P"], &["E"], dep.clone(), json!({})), r),
            hard(600 + 3600)
        );
        assert_eq!(rule(depending(&["P", "E"], &[], dep, json!({})), r), ZERO);
    }
}

#[test]
fn a_visit_between_linked_visits_costs_an_hour_when_pooling_rules_it_out() {
    let pooling = |kind: &str| {
        rule(
            depending(
                &["P", "X", "E"],
                &[],
                json!({"poolingType": kind}),
                json!({}),
            ),
            Rule::DependencyPooling,
        )
    };
    assert_eq!(pooling("DISALLOW_POOLING"), hard(3600));
    assert_eq!(pooling("ALLOW_POOLING_ALL"), ZERO);
    // X is in no dependency: pooling only dependencies rules it out.
    assert_eq!(pooling("ALLOW_POOLING_DEPENDENCIES"), hard(3600));
    assert_eq!(pooling("ALLOW_POOLING_VISITS"), ZERO);
}

#[test]
fn half_a_dependency_planned_costs_the_service_plus_an_hour() {
    assert_eq!(
        rule(
            depending(&["E"], &[], json!({}), json!({})),
            Rule::DependencyPrerequisite
        ),
        hard(600 + 3600)
    );
    assert_eq!(
        rule(
            depending(&["P"], &[], json!({}), json!({})),
            Rule::DependentUnplanned
        ),
        hard(600 + 3600)
    );
    assert_eq!(
        rule(
            depending(&[], &[], json!({}), json!({})),
            Rule::DependentUnplanned
        ),
        ZERO
    );
}

#[test]
fn a_visit_waiting_on_one_after_it_cannot_be_timed() {
    // E waits on P after it, and P on E's state before it.
    assert_eq!(
        rule(
            depending(&["E", "P"], &[], json!({}), json!({})),
            Rule::Loops
        ),
        hard(2 * (600 + 3600))
    );
}

#[test]
fn visits_excluding_each_other_cost_an_hour_when_both_are_planned() {
    let visits = || {
        vec![
            visit("a", 10, json!({"exclusions": ["b"]})),
            visit("b", 10, json!({})),
        ]
    };
    assert_eq!(
        rule(
            plan(&["a", "b"], json!({}), visits(), json!({})),
            Rule::Exclusion
        ),
        hard(3600)
    );
    assert_eq!(
        rule(
            plan(&["a"], json!({}), visits(), json!({})),
            Rule::Exclusion
        ),
        ZERO
    );
}

#[test]
fn a_group_planned_in_part_costs_the_rest_plus_an_hour_each() {
    let group = json!({"visitGroups": [{"id": "G", "visits": [
        visit("Ga", 40, json!({})), visit("Gb", 40, json!({}))]}]});
    assert_eq!(
        rule(
            plan(&["Ga"], json!({}), Vec::new(), group),
            Rule::SemiAssigned
        ),
        hard(2400 + 3600)
    );
}

// Shift limits.

#[test]
fn shift_limits_cost_what_runs_over() {
    let two = || vec![visit("a", 10, json!({})), visit("b", 10, json!({}))];
    let over = |extra: Value, r: Rule| rule(plan(&["a", "b"], extra, two(), json!({})), r);
    assert_eq!(
        over(json!({"maxVisits": 1}), Rule::MaxVisitsHard),
        hard(3600)
    );
    assert_eq!(
        over(json!({"maxSoftVisits": 1}), Rule::MaxVisitsSoft),
        soft(1)
    );
    assert_eq!(
        over(json!({"maxServiceDuration": 15}), Rule::MaxServiceHard),
        hard(300)
    );
    assert_eq!(
        over(json!({"maxSoftServiceDuration": 15}), Rule::MaxServiceSoft),
        soft(300)
    );
    assert_eq!(
        over(json!({"maxEndTime": at(0, 8, 15)}), Rule::EndHard),
        hard(300)
    );
    assert_eq!(
        over(json!({"maxSoftEndTime": at(0, 8, 15)}), Rule::EndSoft),
        soft(300)
    );
    assert_eq!(
        over(
            json!({"maxLastVisitDepartureTime": at(0, 8, 15)}),
            Rule::LastDepartureHard
        ),
        hard(300)
    );
    assert_eq!(
        over(
            json!({"maxSoftLastVisitDepartureTime": at(0, 8, 15)}),
            Rule::LastDepartureSoft
        ),
        soft(300)
    );
}

#[test]
fn travel_costs_its_seconds_and_runs_over_its_limits() {
    let away = || vec![visit("a", 10, json!({"location": AWAY}))];
    let drive = |extra: Value, r: Rule| rule(plan(&["a"], extra, away(), matrix(false)), r);
    assert_eq!(drive(json!({}), Rule::TravelTime), soft(1200));
    assert_eq!(
        drive(json!({"maxTravelTime": 15}), Rule::MaxTravelHard),
        hard(300)
    );
    assert_eq!(
        drive(json!({"maxSoftTravelTime": 15}), Rule::MaxTravelSoft),
        soft(300)
    );
    assert_eq!(
        drive(json!({"maxTravelTimePerVisit": 5}), Rule::MaxTravelPerVisit),
        hard(300)
    );
    // A travel multiplier and extra minutes stretch every drive.
    assert_eq!(
        drive(
            json!({"travelTimeAdjustment": {"multiplier": 1.5, "extraTime": 1}}),
            Rule::TravelTime
        ),
        soft(2 * (900 + 60))
    );
    let mut weights = weighed(Rule::TravelDistance);
    merge(&mut weights, matrix(false));
    assert_eq!(
        rule(
            plan(&["a"], json!({}), away(), weights),
            Rule::TravelDistance
        ),
        soft(10_000)
    );
}

#[test]
fn an_unreachable_visit_costs_an_hour_and_each_unreachable_leg_another() {
    let away = || vec![visit("a", 10, json!({"location": AWAY}))];
    assert_eq!(
        rule(
            plan(&["a"], json!({}), away(), matrix(true)),
            Rule::ReachableVisit
        ),
        hard(3600)
    );
    assert_eq!(
        rule(
            plan(&["a"], json!({}), away(), matrix(true)),
            Rule::ReachableItinerary
        ),
        hard(2 * 3600)
    );
}

#[test]
fn a_shift_serving_nothing_still_pays_its_drive_to_a_break() {
    let lunch = json!({"requiredBreaks": [{"id": "F", "type": "FIXED", "location": AWAY,
                                           "startTime": at(0, 12, 0), "endTime": at(0, 12, 30)}]});
    // A visit nobody serves: a dataset needs at least one.
    let idle = || vec![visit("a", 10, json!({}))];
    let wire = plan(&["brk:F"], lunch.clone(), idle(), matrix(false));
    assert_eq!(rule(wire.clone(), Rule::InactiveTime), soft(1200));
    assert_eq!(rule(wire, Rule::TravelTime), ZERO);
    let mut weights = weighed(Rule::InactiveDistance);
    merge(&mut weights, matrix(false));
    assert_eq!(
        rule(
            plan(&["brk:F"], lunch, idle(), weights),
            Rule::InactiveDistance
        ),
        soft(10_000)
    );
}

#[test]
fn a_shift_costs_its_fixed_cost_and_its_rates_over_its_length() {
    let cost = json!({"cost": {"fixedCost": 25.0, "rates": [
        {"duration": 10, "activationCost": 0.0, "costPerUnit": 30.0, "unit": "HOUR"},
        {"activationCost": 2.0, "costPerUnit": 1.0, "unit": "MINUTE"}]}});
    let visits = vec![visit("a", 20, json!({}))];
    // 25.00, ten minutes at 30.00 an hour, 2.00, ten minutes at 1.00.
    assert_eq!(
        rule(plan(&["a"], cost, visits, json!({})), Rule::ShiftCost),
        soft(2500 + 500 + 200 + 1000)
    );
}

/// A visit that may be served Monday or Tuesday.
fn movable() -> Value {
    visit(
        "m",
        20,
        json!({"timeWindows": [
            {"minStartTime": at(0, 8, 0), "maxEndTime": at(0, 17, 0)},
            {"minStartTime": at(1, 8, 0), "maxEndTime": at(1, 17, 0)}]}),
    )
}

#[test]
fn movable_work_over_its_share_of_a_shift_costs_hard() {
    let limits = json!({"maxEndTime": at(0, 10, 0), "movableOccupationRatioThreshold": 0.1});
    // A tenth of two hours is 720 seconds; the visit takes 1200.
    assert_eq!(
        rule(
            plan(&["m"], limits, vec![movable()], json!({})),
            Rule::MovableBalance
        ),
        hard(480)
    );
}

#[test]
fn a_movable_visit_served_a_day_after_a_shift_could_serve_it_costs_a_thousand() {
    let wire = dataset(
        vec![json!({"id": "ann", "shifts": [
            shift("ann-mon", &[], json!({})),
            shift("ann-tue", &["m"], json!({"minStartTime": at(1, 8, 0)})),
        ]})],
        vec![movable()],
        json!({}),
    );
    assert_eq!(rule(wire, Rule::EarliestDay), soft(1000));
}

#[test]
fn a_movable_visit_counts_its_days_from_the_first_shift_that_may_serve_it() {
    let tuesday = || shift("ann-tue", &["m"], json!({"minStartTime": at(1, 8, 0)}));
    // No shift works Monday: Tuesday is the earliest day it can be served.
    let only_tuesday = dataset(vec![vehicle("ann", tuesday())], vec![movable()], json!({}));
    assert_eq!(rule(only_tuesday, Rule::EarliestDay), ZERO);
    // Bob works Monday, but the visit may not go to him.
    let mut barred = movable();
    barred["prohibitedVehicles"] = json!(["bob"]);
    let bob_barred = dataset(
        vec![
            vehicle("bob", shift("bob-mon", &[], json!({}))),
            vehicle("ann", tuesday()),
        ],
        vec![barred],
        json!({}),
    );
    assert_eq!(rule(bob_barred, Rule::EarliestDay), ZERO);
    // Bob works Monday without the skill it needs.
    let mut skilled = movable();
    skilled["requiredSkills"] = json!([{"name": "hvac"}]);
    let bob_unskilled = dataset(
        vec![
            vehicle("bob", shift("bob-mon", &[], json!({}))),
            vehicle(
                "ann",
                shift(
                    "ann-tue",
                    &["m"],
                    json!({"minStartTime": at(1, 8, 0), "skills": [{"name": "hvac"}]}),
                ),
            ),
        ],
        vec![skilled],
        json!({}),
    );
    assert_eq!(rule(bob_unskilled, Rule::EarliestDay), ZERO);
}

#[test]
fn utilization_costs_the_square_of_each_technicians_share() {
    let visits = vec![visit("a", 60, json!({}))];
    // An hour of eight available: a share of 1250 in ten thousand.
    assert_eq!(
        rule(
            plan(
                &["a"],
                json!({"maxEndTime": at(0, 16, 0)}),
                visits,
                json!({})
            ),
            Rule::Fairness
        ),
        soft(1250 * 1250 / 10_000)
    );
}

/// Ann's Monday and Tuesday shifts and the two linked as one trip.
fn linked(mon: &[&str], tue: &[&str], trip: &[&str]) -> Value {
    let day = |d: i64| json!({"minStartTime": at(d, 8, 0), "maxEndTime": at(d, 17, 0)});
    dataset(
        vec![json!({"id": "ann",
            "shifts": [shift("ann-mon", mon, day(0)), shift("ann-tue", tue, day(1))],
            "linkedShifts": [{"id": "ann-trip", "itinerary": items(trip),
                "shifts": [{"id": "l", "fromShiftId": "ann-mon", "toShiftId": "ann-tue"}]}]})],
        vec![
            visit("a", 10, json!({})),
            visit(
                "b",
                10,
                json!({"timeWindows": [{"minStartTime": at(1, 8, 0), "maxEndTime": at(1, 17, 0)}]}),
            ),
            visit("c", 10, json!({})),
        ],
        json!({}),
    )
}

#[test]
fn a_linked_shift_excludes_the_shifts_it_links() {
    assert_eq!(
        rule(linked(&["a"], &[], &["b"]), Rule::ExcludesShifts),
        hard(3600)
    );
    // An hour per visit on the component, so emptying it pays visit by visit.
    assert_eq!(
        rule(linked(&[], &["b", "a"], &["c"]), Rule::ExcludesShifts),
        hard(2 * 3600)
    );
    assert_eq!(
        rule(linked(&[], &[], &["a", "b"]), Rule::ExcludesShifts),
        ZERO
    );
}

#[test]
fn a_night_out_between_two_visits_near_home_costs_ten_times_the_top_priority() {
    // The default priority weighs 10_000.
    assert_eq!(
        rule(linked(&[], &[], &["a", "b"]), Rule::Overnight),
        medium(100_000)
    );
    assert_eq!(rule(linked(&["a"], &["b"], &[]), Rule::Overnight), ZERO);
}

// Replanning.

fn pinned(start: (i64, i64), extra: Value) -> Value {
    let mut wire = plan(
        &["x", "a"],
        json!({}),
        vec![visit("x", 40, json!({})), visit("a", 10, json!({}))],
        extra,
    );
    wire["vehicles"][0]["shifts"][0]["itinerary"][1] = json!({"id": "a", "kind": "VISIT",
        "pin": true, "startServiceTime": at(0, start.0, start.1)});
    wire
}

#[test]
fn a_pinned_visit_waits_for_its_start_and_costs_hard_past_the_allowed_delay() {
    // x runs 08:00–08:40; a was kept at 08:30.
    assert_eq!(
        rule(pinned((8, 30), json!({})), Rule::PinStart),
        hard(600 * 100_000)
    );
    let allowed = json!({"parameters": {"pinnedStartAllowedDelay": 15}});
    assert_eq!(rule(pinned((8, 30), allowed.clone()), Rule::PinStart), ZERO);
    assert_eq!(
        rule(pinned((8, 30), allowed), Rule::PinStartDelay),
        soft(600)
    );
    assert_eq!(
        rule(pinned((9, 0), weighed(Rule::Waiting)), Rule::Waiting),
        soft(1200)
    );
}

#[test]
fn moving_a_visit_from_its_input_time_costs_the_seconds_moved() {
    let mut wire = plan(
        &["a"],
        json!({}),
        vec![visit("a", 10, json!({}))],
        weighed(Rule::Disruption),
    );
    wire["vehicles"][0]["shifts"][0]["itinerary"][0] =
        json!({"id": "a", "kind": "VISIT", "startServiceTime": at(0, 8, 5)});
    assert_eq!(rule(wire, Rule::Disruption), soft(300));
}

// Route shape, off unless weighed.

#[test]
fn consecutive_items_at_one_place_and_in_one_group_earn_a_point() {
    let visits = || {
        vec![
            visit("a", 10, json!({"preferredConsecutiveGroupId": "block"})),
            visit("b", 10, json!({"preferredConsecutiveGroupId": "block"})),
        ]
    };
    let shape = |extra: Value, r: Rule| rule(plan(&["a", "b"], json!({}), visits(), extra), r);
    assert_eq!(shape(weighed(Rule::Colocated), Rule::Colocated), reward(1));
    assert_eq!(
        shape(weighed(Rule::Consecutive), Rule::Consecutive),
        reward(1)
    );
    assert_eq!(shape(json!({}), Rule::Consecutive), ZERO);
}

#[test]
fn moving_away_from_the_end_location_costs_the_meters() {
    let visits = vec![
        visit("a", 10, json!({})),
        visit("b", 10, json!({"location": AWAY})),
    ];
    let mut weights = weighed(Rule::TowardsEnd);
    merge(&mut weights, matrix(false));
    assert_eq!(
        rule(
            plan(&["a", "b"], json!({}), visits, weights),
            Rule::TowardsEnd
        ),
        soft(5000)
    );
}

#[test]
fn visits_use_up_the_parts_their_shift_loads_in_route_order() {
    let stock = json!({"stock": [{"part": "filter", "quantity": 3}]});
    let visits = vec![
        visit(
            "a",
            10,
            json!({"parts": [{"part": "filter", "quantity": 2}]}),
        ),
        visit(
            "b",
            10,
            json!({"parts": [{"part": "filter", "quantity": 2}, {"part": "valve", "quantity": 1}]}),
        ),
    ];
    // Three filters: b finds one, is a filter and the valve the shift never loaded short.
    assert_eq!(
        rule(
            plan(&["a", "b"], stock.clone(), visits.clone(), json!({})),
            Rule::PartsMissing
        ),
        hard(2 * 3600)
    );
    // Enough of both: nothing short.
    let full =
        json!({"stock": [{"part": "filter", "quantity": 4}, {"part": "valve", "quantity": 1}]});
    assert_eq!(
        rule(
            plan(&["a", "b"], full, visits.clone(), json!({})),
            Rule::PartsMissing
        ),
        ZERO
    );
    // Unplanned visits use nothing.
    assert_eq!(
        rule(plan(&["a"], stock, visits, json!({})), Rule::PartsMissing),
        ZERO
    );
}

#[test]
fn a_negative_part_quantity_is_refused() {
    let visits = vec![visit(
        "a",
        10,
        json!({"parts": [{"part": "filter", "quantity": -1}]}),
    )];
    assert!(
        issue_codes(plan(&["a"], json!({}), visits, json!({}))).contains(&"bad_parts".to_string())
    );
}

#[test]
fn a_shared_tool_serves_one_visit_at_a_time_and_is_carried_between_sites() {
    // Ann at the depot and Bob a kilometre away both start a one-hour visit
    // needing the lift at 08:00; one lift, ten minutes' drive between them.
    let two = |count: i64, bob_from: i64| {
        let mut visits = vec![
            visit("a", 60, json!({"requiredTools": ["lift"]})),
            visit(
                "b",
                60,
                json!({"location": AWAY, "requiredTools": ["lift"], "timeWindows": [{"minStartTime": at(0, bob_from / 60, bob_from % 60), "maxEndTime": at(0, 18, 0)}]}),
            ),
        ];
        visits.push(visit("c", 30, json!({})));
        let mut extra = matrix(false);
        extra["tools"] = json!([{"id": "lift", "name": "Scissor lift", "count": count}]);
        dataset(
            vec![
                vehicle("ann", shift("ann-mon", &["a"], json!({}))),
                vehicle(
                    "bob",
                    shift("bob-mon", &["b"], json!({"startLocation": AWAY})),
                ),
            ],
            visits,
            extra,
        )
    };
    // Bob starts at 08:00 while Ann holds the lift until 09:00, and it
    // reaches him at 09:10: seventy minutes.
    assert_eq!(rule(two(1, 480), Rule::ToolBusy), hard(70 * 60));
    // From 09:10 on he finds it there.
    assert_eq!(rule(two(1, 550), Rule::ToolBusy), ZERO);
    // With two lifts nobody waits.
    assert_eq!(rule(two(2, 480), Rule::ToolBusy), ZERO);
}

#[test]
fn a_visit_needing_a_tool_the_plan_does_not_list_is_refused() {
    let visits = vec![visit("a", 60, json!({"requiredTools": ["rig"]}))];
    assert!(
        issue_codes(plan(&["a"], json!({}), visits.clone(), json!({})))
            .contains(&"unknown_tool".to_string())
    );
    let tools = json!({"tools": [{"id": "lift"}]});
    assert!(
        issue_codes(plan(&["a"], json!({}), visits, tools)).contains(&"unknown_tool".to_string())
    );
}

#[test]
fn a_technician_rests_after_so_long_at_the_wheel() {
    // Three hours each way between the depot and the site; a 30-minute rest
    // after every two hours at the wheel.
    let far = json!({"travelMatrix": {
        "locations": [DEPOT, AWAY],
        "seconds": [[0, 10_800], [10_800, 0]],
        "meters": [[0, 300_000], [300_000, 0]],
    }});
    let plan = |rest: Value| {
        let mut shift_extra = json!({"maxEndTime": at(0, 15, 0)});
        if !rest.is_null() {
            shift_extra["drivingRest"] = rest;
        }
        let visits = vec![visit(
            "a",
            60,
            json!({"location": AWAY, "timeWindows": [{"minStartTime": at(0, 8, 0), "maxEndTime": at(0, 12, 0)}]}),
        )];
        dataset(
            vec![vehicle("ann", shift("ann-mon", &["a"], shift_extra))],
            visits,
            far.clone(),
        )
    };
    let rest = json!({"afterMinutes": 120, "minutes": 30});
    // Without rests: there at 11:00, done at 12:00, home at 15:00.
    assert_eq!(rule(plan(Value::Null), Rule::WindowEnd), ZERO);
    assert_eq!(rule(plan(Value::Null), Rule::EndHard), ZERO);
    // A rest on the way out ends the visit at 12:30, and another on the way
    // back (one hour driven before the visit, three after) brings him home
    // at 16:00.
    assert_eq!(rule(plan(rest.clone()), Rule::WindowEnd), hard(1800));
    assert_eq!(rule(plan(rest), Rule::EndHard), hard(3600));
    // The rest is no travel.
    assert_eq!(
        rule(
            plan(json!({"afterMinutes": 120, "minutes": 30})),
            Rule::TravelTime
        ),
        soft(21_600)
    );
}

#[test]
fn a_wait_as_long_as_the_rest_counts_as_one() {
    let far = json!({"travelMatrix": {
        "locations": [DEPOT, AWAY],
        "seconds": [[0, 6_000], [6_000, 0]],
        "meters": [[0, 100_000], [100_000, 0]],
    }});
    // 100 minutes each way, a rest after 150: the round trip needs one,
    // unless the technician waits half an hour at the visit.
    let plan = |opens: i64| {
        let visits = vec![
            visit(
                "a",
                10,
                json!({"location": AWAY, "timeWindows": [{"minStartTime": at(0, 9, 40), "maxEndTime": at(0, 18, 0)}]}),
            ),
            visit(
                "b",
                10,
                json!({"location": AWAY, "timeWindows": [{"minStartTime": at(0, opens / 60, opens % 60), "maxEndTime": at(0, 18, 0)}]}),
            ),
        ];
        dataset(
            vec![vehicle(
                "ann",
                shift(
                    "ann-mon",
                    &["a", "b"],
                    json!({"maxEndTime": at(0, 11, 30), "drivingRest": {"afterMinutes": 150, "minutes": 30}}),
                ),
            )],
            visits,
            far.clone(),
        )
    };
    // Back to back: 09:40-09:50, 09:50-10:00, then 100 minutes and a rest:
    // home at 12:10, forty minutes past 11:30.
    assert_eq!(rule(plan(9 * 60 + 50), Rule::EndHard), hard(2400));
    // A twenty-minute wait after a ends at 09:50 is no rest: b 10:10-10:20,
    // then the drive and a rest, home at 12:30.
    assert_eq!(rule(plan(10 * 60 + 10), Rule::EndHard), hard(3600));
    // A thirty-minute wait is: b 10:20-10:30, home at 12:10 without
    // stopping, ten minutes later starting but twenty earlier home.
    assert_eq!(rule(plan(10 * 60 + 20), Rule::EndHard), hard(2400));
}

#[test]
fn a_driving_rest_needs_a_limit_above_zero() {
    let visits = vec![visit("a", 10, json!({}))];
    let bad = json!({"drivingRest": {"afterMinutes": 0, "minutes": 30}});
    assert!(
        issue_codes(plan(&["a"], bad, visits, json!({}))).contains(&"bad_driving_rest".to_string())
    );
}
