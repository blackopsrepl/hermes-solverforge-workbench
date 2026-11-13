//! Rotations and patterns, each against a hand-built plan with a known penalty.

use crate::common::*;
use serde_json::{json, Value};
use solverforge_workbench_employee_scheduling::Plan;

fn under(rules: Value, employees: Vec<Value>, shifts: Vec<Value>, end: &str) -> Plan {
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

/// `who` works `from`-`to` hours on `day` (to may pass 24), tagged `tags`.
fn work(id: &str, day: i64, from: i64, to: i64, who: &str, tags: &[&str]) -> Value {
    shift_with(
        id,
        &at(day, from, 0),
        &at(day, to, 0),
        Some(who),
        json!({"tags": tags}),
    )
}

const WEEK: &str = "2027-03-08T00:00";

#[test]
fn overlapping_shifts_cost_their_shared_minutes() {
    let plan = under(
        json!([{"kind": "overlapAllowed", "withTags": ["oncall"]}]),
        vec![on("ann"), employee("bob", json!({}))],
        vec![
            work("a", 0, 7, 15, "ann", &[]),
            work("b", 0, 14, 18, "ann", &[]),
            work("c", 0, 8, 10, "ann", &["oncall"]),
            work("d", 0, 9, 11, "ann", &["oncall"]),
            work("e", 0, 7, 15, "bob", &[]),
            work("f", 0, 10, 12, "bob", &[]),
        ],
        WEEK,
    );
    // ann: a-b 60, a-c 120, a-d 120 (c-d may overlap); bob: e-f 120.
    assert_eq!(score(&plan, "Overlapping shifts"), hard(-420));
}

#[test]
fn rest_between_shifts() {
    let plan = under(
        json!([
            {"kind": "restBetween", "min": 600, "max": 2880, "withinMinutes": 4320},
            {"kind": "restBetween", "level": "preferred", "min": 1000, "before": {"withTags": ["night"]}},
        ]),
        vec![on("ann")],
        vec![
            work("a", 0, 7, 15, "ann", &["day"]),
            work("n1", 0, 23, 31, "ann", &["night"]),
            work("n2", 1, 23, 31, "ann", &["night"]),
            work("f", 4, 7, 15, "ann", &["day"]),
            work("s", 6, 22, 23, "ann", &["day"]),
        ],
        WEEK,
    );
    // a->n1 rests 480 (120 short); f->s rests 3300 (420 long).
    assert_eq!(
        score(&plan, "Rest between shifts outside required range"),
        hard(-540)
    );
    // n1->n2 rests 960 after a night (40 short).
    assert_eq!(
        score(&plan, "Rest between shifts outside preferred range"),
        soft(-40)
    );
}

#[test]
fn start_time_spread_per_period() {
    let plan = under(
        json!([
            {"kind": "startSpread", "period": "week", "max": 120},
            {"kind": "startSpread", "period": "week", "level": "preferred", "max": 60, "weight": 2},
        ]),
        vec![on("ann"), on("bob")],
        vec![
            shift("a1", &at(0, 7, 0), &at(0, 15, 0), Some("ann")),
            shift("a2", &at(1, 8, 30), &at(1, 15, 0), Some("ann")),
            shift("a3", &at(2, 7, 30), &at(2, 15, 0), Some("ann")),
            shift("b1", &at(0, 6, 0), &at(0, 14, 0), Some("bob")),
            shift("b2", &at(1, 10, 0), &at(1, 18, 0), Some("bob")),
        ],
        WEEK,
    );
    assert_eq!(
        score(&plan, "Start time spread per period outside required range"),
        hard(-120)
    );
    assert_eq!(
        score(
            &plan,
            "Start time spread per period outside preferred range"
        ),
        soft(-(60 + 360))
    );
}

#[test]
fn split_shifts_close_together_are_rewarded() {
    let plan = under(
        json!([{"kind": "splitShifts", "max": 180}]),
        vec![on("ann")],
        vec![
            work("a", 0, 7, 11, "ann", &[]),
            work("b", 0, 13, 17, "ann", &[]),
            work("c", 1, 7, 11, "ann", &[]),
            work("d", 1, 17, 21, "ann", &[]),
        ],
        WEEK,
    );
    assert_eq!(score(&plan, "Split shifts close together"), soft(60));
}

#[test]
fn daily_shift_pairings() {
    let plan = under(
        json!([
            {"kind": "dailyPairing", "stance": "required", "withTags": ["early"], "paired": {"withTags": ["late"]}},
            {"kind": "dailyPairing", "stance": "preferred", "withTags": ["night"], "paired": {"withTags": ["rest"]}, "dayOffset": 1},
            {"kind": "dailyPairing", "stance": "barred", "withTags": ["early"], "paired": {"withTags": ["night"]}},
            {"kind": "dailyPairing", "stance": "unwanted", "withTags": ["late"], "paired": {"withTags": ["early"]}, "dayOffset": 1, "weight": 3},
        ]),
        vec![on("ann")],
        vec![
            work("e0", 0, 7, 11, "ann", &["early"]),
            work("l0", 0, 15, 19, "ann", &["late"]),
            work("e1", 1, 7, 11, "ann", &["early"]),
            work("n1", 1, 23, 31, "ann", &["night"]),
        ],
        WEEK,
    );
    assert_eq!(score(&plan, "Required shift pairing missing"), hard(-1));
    assert_eq!(score(&plan, "Preferred shift pairing missing"), soft(-1));
    assert_eq!(score(&plan, "Barred shift pairing worked"), hard(-1));
    assert_eq!(score(&plan, "Unwanted shift pairing worked"), soft(-3));
}

#[test]
fn single_day_patterns() {
    let plan = under(
        json!([
            {"kind": "dayPattern", "stance": "required", "ordered": true, "steps": [{"withTags": ["a"]}, {"withTags": ["b"]}]},
            {"kind": "dayPattern", "stance": "preferred", "steps": [{"withTags": ["c"]}, {"withTags": ["d"]}]},
            {"kind": "dayPattern", "stance": "barred", "steps": [{"withTags": ["e"]}, {"withTags": ["e"]}]},
            {"kind": "dayPattern", "stance": "unwanted", "steps": [{"withTags": ["f"]}], "weight": 2},
        ]),
        vec![on("ann")],
        vec![
            work("m1", 0, 7, 9, "ann", &["a"]),
            work("m2", 0, 10, 12, "ann", &["b"]),
            work("t1", 1, 7, 9, "ann", &["b"]),
            work("t2", 1, 10, 12, "ann", &["a"]),
            work("w1", 2, 7, 9, "ann", &["d"]),
            work("w2", 2, 10, 12, "ann", &["c"]),
            work("h1", 3, 7, 9, "ann", &["e"]),
            work("h2", 3, 10, 12, "ann", &["e"]),
            work("f1", 4, 7, 9, "ann", &["f"]),
            work("s1", 5, 7, 9, "ann", &["e"]),
        ],
        WEEK,
    );
    assert_eq!(score(&plan, "Required day pattern broken"), hard(-1));
    assert_eq!(score(&plan, "Preferred day pattern worked"), soft(1));
    assert_eq!(score(&plan, "Barred day pattern worked"), hard(-1));
    assert_eq!(score(&plan, "Unwanted day pattern worked"), soft(-2));
}

#[test]
fn multi_day_patterns() {
    let plan = under(
        json!([
            {"kind": "multiDayPattern", "stance": "required", "days": [
                {"work": true, "withTags": ["night"]}, {"work": false}, {"work": true}]},
            {"kind": "multiDayPattern", "stance": "preferred", "days": [{"work": true}, {"work": true}]},
            {"kind": "multiDayPattern", "stance": "barred", "days": [
                {"work": true, "withTags": ["night"]}, {"work": true, "withTags": ["early"]}]},
            {"kind": "multiDayPattern", "stance": "unwanted", "days": [{"work": true}, {"work": false}, {"work": true}]},
        ]),
        vec![on("ann")],
        vec![
            work("mon", 0, 23, 31, "ann", &["night"]),
            work("wed", 2, 7, 15, "ann", &["early"]),
            work("thu", 3, 23, 31, "ann", &["night"]),
            work("fri", 4, 7, 15, "ann", &["early"]),
        ],
        WEEK,
    );
    // Thu night is not followed by a free Friday.
    assert_eq!(score(&plan, "Required multi-day pattern broken"), hard(-1));
    // Wed-Thu and Thu-Fri are both worked.
    assert_eq!(score(&plan, "Preferred multi-day pattern worked"), soft(2));
    assert_eq!(score(&plan, "Barred multi-day pattern worked"), hard(-1));
    // Mon worked, Tue free, Wed worked.
    assert_eq!(score(&plan, "Unwanted multi-day pattern worked"), soft(-1));
}

#[test]
fn weekly_rotation() {
    let plan = under(
        json!([
            {"kind": "rotation", "cycle": "weekly", "groups": [{"withTags": ["day"]}, {"withTags": ["night"]}]},
            {"kind": "rotation", "cycle": "weekly", "level": "preferred", "weight": 2,
             "groups": [{"withTags": ["day"]}, {"withTags": ["night"]}]},
        ]),
        vec![on("ann")],
        vec![
            work("w0a", 0, 7, 15, "ann", &["day"]),
            work("w0b", 1, 7, 15, "ann", &["day"]),
            work("w1a", 7, 7, 15, "ann", &["day"]),
            work("w1b", 8, 23, 31, "ann", &["night"]),
            work("w2a", 14, 23, 31, "ann", &["night"]),
        ],
        "2027-03-22T00:00",
    );
    // Week two stays on days (no rotation) and mixes in a night.
    assert_eq!(score(&plan, "Required weekly rotation broken"), hard(-2));
    assert_eq!(score(&plan, "Preferred weekly rotation broken"), soft(-4));
}

#[test]
fn rotation_blocks() {
    let plan = under(
        json!([
            {"kind": "rotation", "cycle": "blocks", "max": 3, "groups": [{"withTags": ["day"]}, {"withTags": ["night"]}]},
            {"kind": "rotation", "cycle": "blocks", "level": "preferred", "min": 2, "max": 2,
             "groups": [{"withTags": ["day"]}, {"withTags": ["night"]}]},
        ]),
        vec![on("ann")],
        vec![
            work("d0", 0, 7, 15, "ann", &["day"]),
            work("d1", 1, 7, 15, "ann", &["day"]),
            work("d2", 2, 7, 15, "ann", &["day"]),
            work("d3", 3, 7, 15, "ann", &["day"]),
            work("n4", 4, 23, 31, "ann", &["night"]),
        ],
        WEEK,
    );
    assert_eq!(
        score(&plan, "Rotation block longer than required maximum"),
        hard(-1)
    );
    assert_eq!(
        score(&plan, "Rotation block length outside preferred range"),
        soft(-3)
    );
}

#[test]
fn patterns_score_incrementally_like_in_full() {
    let mut shifts = Vec::new();
    for d in 0..14 {
        for (kind, from, to) in [
            ("early", 7, 15),
            ("late", 15, 23),
            ("night", 23, 31),
            ("split", 9, 12),
        ] {
            shifts.push(shift_with(
                &format!("{kind}{d}"),
                &at(d, from, 0),
                &at(d, to, 0),
                None,
                json!({"tags": [kind, if kind == "night" { "nights" } else { "days" }]}),
            ));
        }
    }
    let plan = under(
        json!([
            {"kind": "overlapAllowed", "withTags": ["split"]},
            {"kind": "restBetween", "min": 600, "withinMinutes": 2000},
            {"kind": "restBetween", "level": "preferred", "min": 720, "before": {"withTags": ["night"]}},
            {"kind": "startSpread", "period": "week", "max": 480},
            {"kind": "splitShifts", "max": 240},
            {"kind": "dailyPairing", "stance": "required", "withTags": ["early"], "paired": {"withTags": ["split"]}},
            {"kind": "dailyPairing", "stance": "unwanted", "withTags": ["late"], "paired": {"withTags": ["early"]}, "dayOffset": 1},
            {"kind": "dayPattern", "stance": "required", "ordered": true, "steps": [{"withTags": ["early"]}, {"withTags": ["late"]}]},
            {"kind": "dayPattern", "stance": "preferred", "steps": [{"withTags": ["split"]}, {"withTags": ["late"]}]},
            {"kind": "multiDayPattern", "stance": "required", "days": [{"work": true, "withTags": ["night"]}, {"work": false}, {"work": true}]},
            {"kind": "multiDayPattern", "stance": "unwanted", "days": [{"work": true}, {"work": false}, {"work": true}]},
            {"kind": "rotation", "cycle": "weekly", "groups": [{"withTags": ["days"]}, {"withTags": ["nights"]}]},
            {"kind": "rotation", "cycle": "blocks", "level": "preferred", "min": 2, "max": 4,
             "groups": [{"withTags": ["days"]}, {"withTags": ["nights"]}]},
        ]),
        vec![on("ann"), on("bob"), on("cy"), employee("dee", json!({}))],
        shifts,
        "2027-03-15T00:00",
    );
    assert_incremental_matches_full(plan, 600, 5);
}
