//! Contract work limits, each against a hand-built plan with a known penalty.

mod common;

use common::*;
use serde_json::{json, Value};

/// A plan where every listed employee works under contract `c` with `rules`.
fn under(
    rules: Value,
    employees: Vec<Value>,
    shifts: Vec<Value>,
    end: &str,
) -> solverforge_workbench_employee_scheduling::Plan {
    plan(dataset(json!({
        "schedule": {"start": "2027-03-01T00:00", "end": end},
        "contracts": [{"id": "c", "rules": rules}],
        "employees": employees,
        "shifts": shifts,
    })))
}

fn on(id: &str) -> Value {
    employee(id, json!({"contracts": ["c"]}))
}

/// 07:00-15:00 on `day`.
fn day_shift(id: &str, day: i64, who: &str) -> Value {
    shift(id, &at(day, 7, 0), &at(day, 15, 0), Some(who))
}

const WEEK: &str = "2027-03-08T00:00";
const FOUR_WEEKS: &str = "2027-03-29T00:00";

#[test]
fn minutes_per_period() {
    let plan = under(
        json!([
            {"kind": "minutesWorked", "period": "week", "max": 600},
            {"kind": "minutesWorked", "period": "week", "level": "preferred", "min": 120, "max": 1200, "weight": 2},
        ]),
        vec![
            on("ann"),
            employee("bob", json!({"contracts": ["c"], "priority": 3})),
        ],
        vec![
            day_shift("a", 0, "ann"),
            shift_with(
                "b",
                &at(1, 7, 0),
                &at(1, 15, 0),
                Some("ann"),
                json!({"breaks": [{"from": at(1, 11, 0), "to": at(1, 12, 0)}]}),
            ),
            day_shift("c", 2, "ann"),
        ],
        WEEK,
    );
    // ann works 480 + 420 + 480 = 1380 minutes; bob none.
    assert_eq!(
        score(&plan, "Minutes per period outside required range"),
        hard(-780)
    );
    assert_eq!(
        score(&plan, "Minutes per period outside preferred range"),
        soft(-(180 * 2 + 120 * 2 * 3))
    );
}

#[test]
fn minutes_in_rolling_window() {
    let plan = under(
        json!([
            {"kind": "minutesWorked", "rollingDays": 2, "max": 480},
            {"kind": "minutesWorked", "rollingDays": 3, "level": "preferred", "min": 60},
        ]),
        vec![on("ann"), on("bob")],
        vec![
            day_shift("a", 0, "ann"),
            shift_with(
                "b",
                &at(1, 7, 0),
                &at(1, 15, 0),
                Some("ann"),
                json!({"breaks": [{"from": at(1, 11, 0), "to": at(1, 12, 0)}]}),
            ),
            day_shift("c", 2, "ann"),
        ],
        WEEK,
    );
    // Two-day windows Mon-Tue and Tue-Wed hold 900 minutes each.
    assert_eq!(
        score(&plan, "Minutes in rolling window outside required range"),
        hard(-840)
    );
    // Three-day windows starting Thu and Fri are empty for ann; all five for bob.
    assert_eq!(
        score(&plan, "Minutes in rolling window outside preferred range"),
        soft(-420)
    );
}

#[test]
fn logged_minutes_per_period() {
    let plan = under(
        json!([
            {"kind": "minutesLogged", "period": "week", "max": 600},
            {"kind": "minutesLogged", "period": "week", "level": "preferred", "min": 1000},
        ]),
        vec![
            employee(
                "ann",
                json!({"contracts": ["c"], "loggedBefore": [{"date": "2027-03-01", "minutes": 100}]}),
            ),
            on("bob"),
        ],
        vec![
            shift_with(
                "a",
                &at(0, 7, 0),
                &at(0, 15, 0),
                Some("ann"),
                json!({"loggedMinutes": 300}),
            ),
            shift_with(
                "b",
                &at(1, 7, 0),
                &at(1, 15, 0),
                Some("ann"),
                json!({"loggedMinutes": 300}),
            ),
            day_shift("c", 2, "ann"),
        ],
        WEEK,
    );
    // ann logs 100 + 300 + 300 + 480 = 1180.
    assert_eq!(
        score(&plan, "Logged minutes per period outside required range"),
        hard(-580)
    );
    assert_eq!(
        score(&plan, "Logged minutes per period outside preferred range"),
        soft(-1000)
    );
}

#[test]
fn days_worked_per_period_and_rolling() {
    let plan = under(
        json!([
            {"kind": "daysWorked", "period": "week", "max": 2},
            {"kind": "daysWorked", "period": "week", "level": "preferred", "min": 4},
            {"kind": "daysWorked", "rollingDays": 3, "max": 1},
            {"kind": "daysWorked", "rollingDays": 7, "level": "preferred", "min": 1},
        ]),
        vec![on("ann"), on("bob")],
        vec![
            shift("a", &at(0, 7, 0), &at(0, 11, 0), Some("ann")),
            shift("b", &at(0, 12, 0), &at(0, 16, 0), Some("ann")),
            day_shift("c", 1, "ann"),
            day_shift("d", 2, "ann"),
        ],
        WEEK,
    );
    assert_eq!(
        score(&plan, "Days worked per period outside required range"),
        hard(-1)
    );
    assert_eq!(
        score(&plan, "Days worked per period outside preferred range"),
        soft(-5)
    );
    // Windows Mon-Wed (3 days) and Tue-Thu (2 days) exceed one day.
    assert_eq!(
        score(
            &plan,
            "Days worked in rolling window outside required range"
        ),
        hard(-3)
    );
    assert_eq!(
        score(
            &plan,
            "Days worked in rolling window outside preferred range"
        ),
        soft(-1)
    );
}

#[test]
fn employee_shifts_per_period_and_rolling() {
    let plan = under(
        json!([
            {"kind": "shiftsWorked", "period": "week", "max": 2},
            {"kind": "shiftsWorked", "period": "week", "level": "preferred", "min": 3, "max": 3, "weight": 2},
            {"kind": "shiftsWorked", "rollingDays": 2, "max": 1},
            {"kind": "shiftsWorked", "rollingDays": 7, "level": "preferred", "max": 2},
        ]),
        vec![on("ann"), on("bob")],
        vec![
            shift("a", &at(0, 7, 0), &at(0, 11, 0), Some("ann")),
            shift("b", &at(0, 12, 0), &at(0, 16, 0), Some("ann")),
            day_shift("c", 1, "ann"),
            day_shift("d", 2, "ann"),
        ],
        WEEK,
    );
    assert_eq!(
        score(&plan, "Employee shifts per period outside required range"),
        hard(-2)
    );
    assert_eq!(
        score(&plan, "Employee shifts per period outside preferred range"),
        soft(-(2 + 6))
    );
    assert_eq!(
        score(
            &plan,
            "Employee shifts in rolling window outside required range"
        ),
        hard(-3)
    );
    assert_eq!(
        score(
            &plan,
            "Employee shifts in rolling window outside preferred range"
        ),
        soft(-2)
    );
}

#[test]
fn weekends_worked_per_period_and_rolling() {
    let plan = under(
        json!([
            {"kind": "weekendsWorked", "period": "schedule", "max": 1},
            {"kind": "weekendsWorked", "period": "week", "level": "preferred", "min": 1},
            {"kind": "weekendsWorked", "rollingDays": 14, "max": 1},
            {"kind": "weekendsWorked", "rollingDays": 7, "level": "preferred", "max": 0},
        ]),
        vec![on("ann"), employee("bob", json!({}))],
        vec![day_shift("sat", 5, "ann"), day_shift("sun", 13, "ann")],
        "2027-03-15T00:00",
    );
    assert_eq!(
        score(&plan, "Weekends worked per period outside required range"),
        hard(-1)
    );
    assert_eq!(
        score(&plan, "Weekends worked per period outside preferred range"),
        soft(0)
    );
    assert_eq!(
        score(
            &plan,
            "Weekends worked in rolling window outside required range"
        ),
        hard(-1)
    );
    // Seven-day windows starting Mon..Sat of week one and Mon of week two each hold a weekend.
    assert_eq!(
        score(
            &plan,
            "Weekends worked in rolling window outside preferred range"
        ),
        soft(-7)
    );
}

#[test]
fn weekend_minutes_per_period() {
    let plan = under(
        json!([
            {"kind": "weekendMinutes", "period": "week", "max": 300},
            {"kind": "weekendMinutes", "period": "week", "level": "preferred", "max": 600},
        ]),
        vec![on("ann")],
        vec![
            shift("fri", &at(4, 20, 0), &at(5, 4, 0), Some("ann")),
            shift("sun", &at(6, 8, 0), &at(6, 16, 0), Some("ann")),
        ],
        WEEK,
    );
    // 240 weekend minutes Saturday night plus 480 on Sunday.
    assert_eq!(
        score(&plan, "Weekend minutes per period outside required range"),
        hard(-420)
    );
    assert_eq!(
        score(&plan, "Weekend minutes per period outside preferred range"),
        soft(-120)
    );
}

#[test]
fn consecutive_weekends_worked() {
    let plan = under(
        json!([
            {"kind": "consecutiveWeekendsWorked", "period": "schedule", "max": 2},
            {"kind": "consecutiveWeekendsWorked", "period": "schedule", "level": "preferred", "min": 2, "max": 2},
        ]),
        vec![on("ann"), on("bob")],
        vec![
            day_shift("a", 5, "ann"),
            day_shift("b", 12, "ann"),
            day_shift("c", 19, "ann"),
            day_shift("d", 26, "bob"),
        ],
        FOUR_WEEKS,
    );
    assert_eq!(
        score(&plan, "Consecutive weekends worked outside required range"),
        hard(-1)
    );
    assert_eq!(
        score(&plan, "Consecutive weekends worked outside preferred range"),
        soft(-2)
    );
}

#[test]
fn consecutive_days_worked() {
    let plan = under(
        json!([
            {"kind": "consecutiveDaysWorked", "max": 4},
            {"kind": "consecutiveDaysWorked", "level": "preferred", "min": 2, "max": 3},
        ]),
        vec![on("ann"), on("bob")],
        vec![
            day_shift("a0", 0, "ann"),
            day_shift("a1", 1, "ann"),
            day_shift("a2", 2, "ann"),
            day_shift("a3", 3, "ann"),
            day_shift("a4", 4, "ann"),
            day_shift("b0", 0, "bob"),
            day_shift("b2", 2, "bob"),
        ],
        WEEK,
    );
    assert_eq!(
        score(&plan, "Consecutive days worked outside required range"),
        hard(-1)
    );
    assert_eq!(
        score(&plan, "Consecutive days worked outside preferred range"),
        soft(-4)
    );
}

#[test]
fn consecutive_shifts_and_rest_after_them() {
    let plan = under(
        json!([
            {"kind": "consecutiveShifts", "max": 2, "restAfterMinutes": 3000, "restAfterRunsOf": 2},
            {"kind": "consecutiveShifts", "level": "preferred", "min": 2, "restAfterMinutes": 2500},
        ]),
        vec![on("ann")],
        vec![
            day_shift("a", 0, "ann"),
            shift("b", &at(0, 16, 0), &at(0, 20, 0), Some("ann")),
            day_shift("c", 1, "ann"),
            day_shift("d", 3, "ann"),
        ],
        WEEK,
    );
    // Runs: Mon early + Mon evening + Tue (3 shifts), then Thu (1); 2400 minutes between.
    assert_eq!(
        score(&plan, "Consecutive shifts outside required range"),
        hard(-1)
    );
    assert_eq!(
        score(&plan, "Rest after a run of shifts below required minimum"),
        hard(-600)
    );
    assert_eq!(
        score(&plan, "Consecutive shifts outside preferred range"),
        soft(-1)
    );
    assert_eq!(
        score(&plan, "Rest after a run of shifts below preferred minimum"),
        soft(-100)
    );
}

#[test]
fn run_categories_start_days_gaps_and_compactness() {
    let plan = under(
        json!([
            {"kind": "consecutiveDaysWorked", "categories": ["day", "night"],
             "startDays": ["monday", "thursday"], "minGapMinutes": 2880},
            {"kind": "consecutiveDaysWorked", "level": "preferred", "categories": ["day", "night"],
             "startDays": ["monday"], "compact": true, "minGapMinutes": 2000},
        ]),
        vec![on("ann")],
        vec![
            shift_with(
                "mon",
                &at(0, 7, 0),
                &at(0, 15, 0),
                Some("ann"),
                json!({"tags": ["day"]}),
            ),
            shift_with(
                "tue",
                &at(1, 7, 0),
                &at(1, 15, 0),
                Some("ann"),
                json!({"tags": ["day"]}),
            ),
            shift_with(
                "wed",
                &at(2, 23, 0),
                &at(3, 7, 0),
                Some("ann"),
                json!({"tags": ["night"]}),
            ),
            shift_with(
                "thu",
                &at(3, 23, 0),
                &at(4, 7, 0),
                Some("ann"),
                json!({"tags": ["night"]}),
            ),
        ],
        WEEK,
    );
    // Runs: Mon-Tue days, Wed-Thu nights; 32 h from Tue 15:00 to Wed 23:00.
    assert_eq!(score(&plan, "Run starts on a disallowed day"), hard(-1));
    assert_eq!(score(&plan, "Run starts on an allowed day"), soft(1));
    assert_eq!(score(&plan, "Run ends on a start day"), soft(-1));
    assert_eq!(score(&plan, "Compact run of one category"), soft(2));
    assert_eq!(
        score(&plan, "Gap between differing runs below required minimum"),
        hard(-960)
    );
    assert_eq!(
        score(&plan, "Gap between differing runs below preferred minimum"),
        soft(-80)
    );
    assert_eq!(
        score(&plan, "Consecutive days worked outside required range"),
        hard(0)
    );
}

#[test]
fn contract_rules_are_validated() {
    let codes = issue_codes(dataset(json!({
        "contracts": [{"id": "c", "rules": [
            {"kind": "minutesWorked", "max": 10},
            {"kind": "minutesWorked", "period": "fortnight", "max": 10},
            {"kind": "minutesWorked", "period": "week", "rollingDays": 3, "max": 10},
            {"kind": "minutesWorked", "period": "week", "min": 20, "max": 10},
            {"kind": "minutesWorked", "period": "week"},
            {"kind": "minutesLogged", "rollingDays": 3, "max": 10},
            {"kind": "nearDayOff", "stance": "preferred", "before": {}},
        ]}],
        "employees": [employee("ann", json!({"contracts": ["c", "nope"]}))],
    })));
    for code in [
        "rule_window_missing",
        "unknown_period",
        "rule_window_conflict",
        "rule_min_above_max",
        "rule_limit_missing",
        "rule_stance_invalid",
        "unknown_contract",
    ] {
        assert!(codes.contains(&code), "missing {code} in {codes:?}");
    }
}
