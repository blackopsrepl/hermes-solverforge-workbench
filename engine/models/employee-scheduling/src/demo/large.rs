//! `LARGE_MONTH`: a hospital with three wards on two sites, fifty staff and
//! four weeks, using every feature of the model.
//!
//! The dataset is written in the wire format, the same JSON a user would
//! send, so it doubles as a worked example of every field.

use serde_json::{json, Value};

use super::rng::Rng;
use crate::domain::Plan;
use crate::time::{format_date, format_date_time, parse_date, DAY};

const FIRST_DAY: &str = "2027-03-01";
const WEEKS: i64 = 4;
const STAFF: usize = 50;
const WARDS: [&str; 3] = ["ward-a", "ward-b", "ward-c"];
const NAMES: [&str; 25] = [
    "Ada", "Bea", "Cas", "Dov", "Eda", "Fin", "Gil", "Hal", "Ines", "Jo", "Kai", "Lea", "Max",
    "Nia", "Oto", "Pia", "Quin", "Rui", "Sol", "Tao", "Uma", "Vic", "Wes", "Yan", "Zoe",
];
const SURNAMES: [&str; 2] = ["North", "South"];

/// Where the wards are: A and B on the north site, C on the south site.
fn site(ward: &str) -> Value {
    if ward == "ward-c" {
        json!({"lat": 51.012, "lon": 3.801})
    } else {
        json!({"lat": 51.054, "lon": 3.722})
    }
}

fn at(day: i64, minute: i64) -> String {
    format_date_time(day * DAY + minute)
}

fn id(n: usize) -> String {
    format!("e{:02}", n + 1)
}

/// The month's dataset: nothing assigned except a few pinned shifts, with
/// the first week published from an earlier plan.
pub fn large_month() -> Plan {
    let dataset = dataset();
    serde_json::from_value(dataset).expect("the large demo is valid wire data")
}

fn dataset() -> Value {
    let first = parse_date(FIRST_DAY).expect("demo date");
    let days = WEEKS * 7;
    let mut rng = Rng::new(2027);

    let employees: Vec<Value> = (0..STAFF).map(|n| employee(n, first, &mut rng)).collect();
    let shifts = shifts(first, days, &mut rng);
    let clinic_days: Vec<Value> = (0..days)
        .filter(|d| d % 7 < 5)
        .map(|d| json!({"id": format!("clinic-{}", format_date(first + d)), "count": 1}))
        .collect();

    json!({
        "schedule": {
            "start": at(first, 0),
            "end": at(first + days, 0),
            "weekStart": "monday",
            "weekend": ["saturday", "sunday"],
            "periods": [{"id": "fortnight", "ranges": [
                {"from": format_date(first), "to": format_date(first + 13)},
                {"from": format_date(first + 14), "to": format_date(first + 27)},
            ]}],
        },
        "tags": WARDS.iter().map(|w| json!({"id": w, "type": "ward"})).collect::<Vec<_>>(),
        "contracts": contracts(),
        "costGroups": [
            {"id": "staff", "share": 4},
            {"id": "agency", "activationCost": 200, "share": 1, "fillUp": true},
        ],
        "alternatives": clinic_days,
        "teams": [{
            "id": "wound-care-team",
            "leads": [id(10)],
            "members": [id(11), id(12)],
            "tagType": "ward",
            "maxShiftsPerDay": 2,
        }],
        "scheduleRules": schedule_rules(first, days),
        "employees": employees,
        "shifts": shifts,
    })
}

fn employee(n: usize, first: i64, rng: &mut Rng) -> Value {
    let agency = n >= 40;
    let contract = if n < 25 {
        "full-time"
    } else if !agency {
        "part-time"
    } else {
        "agency"
    };
    let mut skills = vec![json!("nurse")];
    let mut tags = vec![json!(if agency { "agency" } else { "staff" })];
    if (n.is_multiple_of(5) || n % 7 == 1) && !agency {
        skills.push(json!({"id": "senior", "level": 1 + (n % 3) as u32}));
        tags.push(json!("senior"));
    }
    if n.is_multiple_of(4) && !agency {
        skills.push(json!("icu"));
    }
    if n % 3 == 1 {
        skills.push(json!("wound-care"));
    }
    let home_ward = WARDS[n % 3];
    let home = json!({
        "lat": 51.03 + (rng.below(80) as f64 - 40.0) / 1000.0,
        "lon": 3.76 + (rng.below(120) as f64 - 60.0) / 1000.0,
    });
    // Pinned shifts go to e01 and e06 on day 0; keep them free that day.
    let off = 1 + rng.below(27) as i64;
    let mut record = json!({
        "id": id(n),
        "name": format!("{} {}", NAMES[n % NAMES.len()], SURNAMES[n / NAMES.len()]),
        "tags": tags,
        "skills": skills,
        "contracts": [contract],
        "costGroup": if agency { "agency" } else { "staff" },
        "home": home,
        "unavailable": [{"from": at(first + off, 0), "to": at(first + off + 1, 0)}],
        "preferredShifts": {"withTags": [home_ward]},
    });
    let extra = record.as_object_mut().expect("object");
    if n % 7 == 3 {
        extra.insert("priority".into(), json!(2));
    }
    if n.is_multiple_of(2) {
        let day = first + rng.below(28) as i64;
        extra.insert(
            "preferredTimes".into(),
            json!([{"from": at(day, 7 * 60), "to": at(day, 15 * 60)}]),
        );
    } else {
        let day = first + rng.below(28) as i64;
        extra.insert(
            "unwantedTimes".into(),
            json!([{"from": at(day, 15 * 60), "to": at(day, 23 * 60)}]),
        );
    }
    if agency {
        // Agency staff only work days and evenings.
        let windows: Vec<Value> = (0..WEEKS * 7)
            .map(|d| json!({"from": at(first + d, 7 * 60), "to": at(first + d, 23 * 60)}))
            .collect();
        extra.insert("availableOnly".into(), json!(windows));
    }
    if n % 10 == 3 {
        extra.insert("requiredShifts".into(), json!({"withoutTags": ["night"]}));
    }
    if n % 9 == 4 {
        extra.insert("avoidRisks".into(), json!(["radiation"]));
    }
    if n % 6 == 2 && !agency {
        let day = first + 2 + rng.below(25) as i64;
        if day - first != off {
            extra.insert("daysOff".into(), json!([format_date(day)]));
        }
    }
    if n < 25 {
        extra.insert(
            "loggedBefore".into(),
            json!([{"date": format_date(first - 1), "minutes": 480}]),
        );
    }
    let pairings = match n {
        2 => json!([{"stance": "preferred", "with": id(3)}]),
        4 => json!([{"stance": "unwanted", "with": id(5)}]),
        6 => json!([{"stance": "barred", "with": id(7)}]),
        8 => json!([{"stance": "required", "withTags": ["senior"], "onTags": ["night"]}]),
        _ => json!([]),
    };
    extra.insert("pairings".into(), pairings);
    record
}

fn shifts(first: i64, days: i64, rng: &mut Rng) -> Vec<Value> {
    let mut out = Vec::new();
    for offset in 0..days {
        let day = first + offset;
        let date = format_date(day);
        let weekend = offset % 7 >= 5;
        for ward in WARDS {
            let base = |kind: &str, slot: usize, from: i64, to: i64| {
                let mut tags = vec![json!(kind), json!(ward)];
                if weekend {
                    tags.push(json!("weekend"));
                }
                json!({
                    "id": format!("{date}-{ward}-{kind}-{slot}"),
                    "start": at(day, from),
                    "end": at(day, to),
                    "tags": tags,
                    "requiredSkills": ["nurse"],
                    "location": site(ward),
                    "costGroup": if kind == "night" { "night" } else { "day" },
                })
            };
            let (earlies, lates) = if weekend { (2, 1) } else { (3, 2) };
            for slot in 0..earlies {
                let mut shift = base("early", slot, 7 * 60, 15 * 60);
                if slot == 0 {
                    shift["requiredSkills"] = json!(["nurse", {"id": "senior", "level": 1}]);
                }
                shift["preferredSkills"] = json!(["wound-care"]);
                shift["breaks"] = json!([{"from": at(day, 11 * 60), "to": at(day, 11 * 60 + 30)}]);
                out.push(shift);
            }
            for slot in 0..lates {
                out.push(base("late", slot, 15 * 60, 23 * 60));
            }
            let mut night = base("night", 0, 23 * 60, DAY + 7 * 60);
            if ward == "ward-b" {
                night["requiredSkills"] = json!(["nurse", "icu"]);
            }
            if ward == "ward-c" {
                night["risks"] = json!(["radiation"]);
            }
            out.push(night);
        }
        if !weekend {
            // Two optional support shifts compete for one treatment room.
            for (slot, (from, to, importance)) in [(9 * 60, 13 * 60, 2), (10 * 60, 14 * 60, 1)]
                .into_iter()
                .enumerate()
            {
                let mut support = json!({
                    "id": format!("{date}-support-{slot}"),
                    "start": at(day, from),
                    "end": at(day, to),
                    "tags": ["support", "ward-a"],
                    "requiredSkills": ["nurse"],
                    "location": site("ward-a"),
                    "optional": true,
                    "importance": importance,
                });
                if rng.below(3) == 0 {
                    support["preferredEmployees"] = json!([id(25 + rng.below(15) as usize)]);
                }
                out.push(support);
            }
            for (slot, (from, to)) in [(8 * 60, 12 * 60), (13 * 60, 17 * 60)].iter().enumerate() {
                out.push(json!({
                    "id": format!("{date}-clinic-{slot}"),
                    "start": at(day, *from),
                    "end": at(day, *to),
                    "tags": ["clinic", "ward-c"],
                    "requiredSkills": ["nurse", "wound-care"],
                    "location": site("ward-c"),
                    "alternativeOf": format!("clinic-{date}"),
                }));
            }
        }
    }
    // A few shifts name employees they prefer, dislike or bar.
    for (n, shift) in out.iter_mut().enumerate().filter(|(n, _)| n % 37 == 5) {
        let who = id(n % STAFF);
        match n % 3 {
            0 => shift["unwantedEmployees"] = json!([who]),
            1 => shift["barredEmployees"] = json!([who]),
            _ => shift["preferredEmployees"] = json!([who]),
        }
    }
    // The first week was published before; a few shifts are already promised.
    for (n, shift) in out.iter_mut().enumerate() {
        let start = shift["start"].as_str().unwrap_or_default().to_owned();
        if start < at(first + 7, 0) && n.is_multiple_of(4) && shift.get("alternativeOf").is_none() {
            shift["published"] = json!(id(rng.below(25) as usize));
        }
    }
    let pin = |out: &mut Vec<Value>, shift_id: &str, who: usize| {
        if let Some(shift) = out.iter_mut().find(|s| s["id"] == shift_id) {
            shift["employee"] = json!(id(who));
            shift["pinned"] = json!(true);
            shift["published"] = json!(id(who));
            if let Some(tags) = shift["tags"].as_array_mut() {
                tags.push(json!("promised"));
            }
        }
    };
    let date = format_date(first);
    pin(&mut out, &format!("{date}-ward-a-early-0"), 0);
    pin(&mut out, &format!("{date}-ward-b-early-0"), 5);
    out
}

fn schedule_rules(first: i64, days: i64) -> Value {
    let slots = |from: i64, to: i64, min: Option<i64>, max: Option<i64>| -> Vec<Value> {
        (0..days)
            .map(|d| {
                let mut slot = json!({"from": at(first + d, from), "to": at(first + d, to)});
                if let Some(min) = min {
                    slot["min"] = json!(min);
                }
                if let Some(max) = max {
                    slot["max"] = json!(max);
                }
                slot
            })
            .collect()
    };
    json!({
        "concurrency": [
            {"max": 3, "withTags": ["night"]},
            // One treatment room for support and clinic work.
            {"max": 1, "withTags": ["support", "clinic"]},
        ],
        "shiftCounts": [
            {"period": "day", "withTags": ["support"], "max": 1},
            {"period": "week", "level": "preferred", "employees": {"withTags": ["agency"]}, "max": 12},
        ],
        "distinctTags": [
            {"period": "day", "level": "preferred", "tagTypes": ["ward"], "min": 3},
            {"period": "day", "tagTypes": ["ward"], "min": 1},
        ],
        "costs": [
            // Agency hours are the spend to keep down; the rest is a budget.
            {"period": "week", "level": "preferred", "employees": {"withTags": ["agency"]}, "rates": [
                {"employeeGroup": "agency", "hourly": 55.0},
            ]},
            {"period": "schedule", "max": 200000, "rates": [
                {"employeeGroup": "agency", "hourly": 55.0},
                {"shiftGroup": "night", "hourly": 45.0},
                {"hourly": 40.0},
            ]},
            {"period": "fortnight", "level": "preferred", "max": 75000, "rates": [{"hourly": 40.0}]},
        ],
        "demand": [
            {"withTags": ["ward-a"], "slots": slots(10 * 60, 12 * 60, Some(2), None)},
            {"employees": {"withTags": ["agency"]}, "slots": slots(10 * 60, 12 * 60, None, Some(2))},
            {"level": "preferred", "slots": slots(16 * 60, 20 * 60, Some(4), None)},
            {"level": "preferred", "withTags": ["ward-a"], "slots": slots(16 * 60, 20 * 60, Some(1), Some(1))},
        ],
        "fairness": [
            {"balance": "minutes", "employees": {"withTags": ["staff"]}, "history": {"e01": 600, "e02": 300}},
            {"balance": "shifts", "withTags": ["weekend"]},
        ],
    })
}

fn rule(kind: &str, extra: Value) -> Value {
    let mut rule = json!({"kind": kind});
    if let (Some(rule), Value::Object(extra)) = (rule.as_object_mut(), extra) {
        rule.extend(extra);
    }
    rule
}

/// Full-time, part-time and agency contracts using every rule kind.
fn contracts() -> Value {
    let shared = || {
        vec![
            rule("restBetween", json!({"min": 600, "withinMinutes": 1440})),
            rule(
                "restBetween",
                json!({"level": "preferred", "min": 720, "withinMinutes": 1440}),
            ),
            rule("consecutiveShifts", json!({"max": 1, "linkMinutes": 600})),
            rule(
                "consecutiveShifts",
                json!({"level": "preferred", "max": 1, "linkMinutes": 720}),
            ),
            rule(
                "consecutiveShifts",
                json!({"withTags": ["night"], "max": 3, "restAfterMinutes": 2880, "restAfterRunsOf": 2}),
            ),
            rule(
                "consecutiveShifts",
                json!({"withTags": ["night"], "level": "preferred", "max": 3, "restAfterMinutes": 3600, "restAfterRunsOf": 2}),
            ),
            rule(
                "dayPattern",
                json!({"stance": "barred", "steps": [{"withTags": ["early"]}, {"withTags": ["late"]}]}),
            ),
            rule(
                "dayPattern",
                json!({"stance": "unwanted", "steps": [{"withTags": ["support"]}, {"withTags": ["late"]}]}),
            ),
            rule(
                "multiDayPattern",
                json!({"stance": "barred", "days": [
                    {"work": true, "withTags": ["late"]},
                    {"work": true, "withTags": ["early"]},
                ]}),
            ),
            rule(
                "dailyPairing",
                json!({"stance": "unwanted", "withTags": ["night"], "dayOffset": 1, "paired": {"withTags": ["early"]}}),
            ),
            rule(
                "disruption",
                json!({"withTags": ["promised"], "withinMinutes": 60}),
            ),
            rule(
                "distinctTags",
                json!({"period": "day", "tagTypes": ["ward"], "max": 1}),
            ),
            rule(
                "nearDayOff",
                json!({"stance": "barred", "before": {"withTags": ["night"]}}),
            ),
            rule(
                "nearDayOff",
                json!({"stance": "unwanted", "after": {"withTags": ["night"]}}),
            ),
            rule(
                "travel",
                json!({"maxMeters": 40000, "minChangeMinutes": 60}),
            ),
            rule(
                "locationsWorked",
                json!({"period": "week", "level": "preferred", "max": 1}),
            ),
            rule(
                "disruption",
                json!({"level": "preferred", "withinMinutes": 120, "keepTags": ["ward-a", "ward-b", "ward-c"]}),
            ),
            rule(
                "dailyPairing",
                json!({"stance": "barred", "withTags": ["early"], "paired": {"withTags": ["late"]}}),
            ),
            rule(
                "multiDayPattern",
                json!({"stance": "unwanted", "days": [
                    {"work": true, "withTags": ["night"]},
                    {"work": false},
                    {"work": true, "withTags": ["early"]},
                ]}),
            ),
            rule(
                "dayPattern",
                json!({"stance": "preferred", "steps": [{"withTags": ["support"]}, {"withTags": ["late"]}]}),
            ),
            rule(
                "shiftTypes",
                json!({"period": "week", "level": "preferred", "categories": ["night"], "max": 2}),
            ),
            rule(
                "distinctTags",
                json!({"period": "week", "level": "preferred", "tagTypes": ["ward"], "max": 2}),
            ),
            rule(
                "distinctTags",
                json!({"period": "week", "tagTypes": ["ward"], "max": 3}),
            ),
            rule(
                "startSpread",
                json!({"period": "week", "level": "preferred", "max": 480}),
            ),
            rule("splitShifts", json!({"level": "preferred", "max": 240})),
            rule(
                "rotation",
                json!({"level": "preferred", "cycle": "weekly", "groups": [{"withTags": ["early", "late"]}, {"withTags": ["night"]}]}),
            ),
            rule("overlapAllowed", json!({"withTags": ["support"]})),
        ]
    };
    let mut full = shared();
    full.extend([
        rule("minutesWorked", json!({"period": "week", "max": 2400})),
        rule("minutesWorked", json!({"period": "week", "level": "preferred", "min": 1200})),
        rule("minutesWorked", json!({"rollingDays": 14, "max": 4800})),
        rule("minutesLogged", json!({"period": "month", "level": "preferred", "max": 9600})),
        rule("daysWorked", json!({"period": "week", "max": 5})),
        rule("daysWorked", json!({"rollingDays": 7, "level": "preferred", "max": 5})),
        rule("daysWorked", json!({"rollingDays": 7, "max": 6})),
        rule("daysWorked", json!({"period": "week", "level": "preferred", "min": 3, "max": 5})),
        rule("shiftsWorked", json!({"rollingDays": 7, "level": "preferred", "max": 5})),
        rule("minutesWorked", json!({"rollingDays": 14, "level": "preferred", "max": 4000})),
        rule("minutesLogged", json!({"period": "month", "max": 12000})),
        rule("consecutiveWeekendsWorked", json!({"period": "schedule", "max": 3})),
        rule("consecutiveDaysOff", json!({"period": "week", "min": 1})),
        rule("daysOff", json!({"period": "week", "level": "preferred", "min": 2, "max": 4})),
        rule("dailyPairing", json!({"stance": "preferred", "withTags": ["late"], "dayOffset": 1, "paired": {"withTags": ["late"]}})),
        rule("shiftsWorked", json!({"period": "fortnight", "level": "preferred", "min": 4})),
        rule("weekendsWorked", json!({"period": "month", "max": 3})),
        rule("weekendsWorked", json!({"rollingDays": 14, "level": "preferred", "max": 1})),
        rule("weekendMinutes", json!({"period": "week", "level": "preferred", "max": 960})),
        rule("consecutiveWeekendsWorked", json!({"period": "schedule", "level": "preferred", "max": 2})),
        rule("consecutiveDaysWorked", json!({"max": 5})),
        rule("consecutiveDaysWorked", json!({"level": "preferred", "max": 4, "categories": ["early", "late", "night"], "startDays": ["monday", "thursday"], "minGapMinutes": 2880, "compact": true})),
        rule("daysOff", json!({"period": "week", "min": 2})),
        rule("consecutiveDaysOff", json!({"period": "week", "level": "preferred", "min": 2})),
        rule("cost", json!({"period": "week", "level": "preferred", "max": 1250, "hourly": 30.0, "baseMinutes": 2400, "overtime": [{"hourly": 45.0, "minutes": 240}, {"hourly": 60.0}]})),
    ]);
    let mut part = shared();
    part.extend([
        rule("minutesWorked", json!({"period": "week", "max": 1440})),
        rule("minutesWorked", json!({"period": "week", "level": "preferred", "min": 480})),
        rule("shiftsWorked", json!({"rollingDays": 7, "max": 4})),
        rule("consecutiveMinutesOff", json!({"rollingDays": 7, "min": 2880})),
        rule("consecutiveMinutesOff", json!({"rollingDays": 7, "level": "preferred", "min": 3600})),
        rule("consecutiveDaysOff", json!({"rollingDays": 7, "level": "preferred", "min": 2})),
        rule("consecutiveWeekendsOff", json!({"period": "schedule", "level": "preferred", "min": 1})),
        rule("weekendsWorked", json!({"rollingDays": 14, "max": 2})),
        rule("cost", json!({"period": "week", "max": 700, "hourly": 28.0})),
        rule("consecutiveDaysOff", json!({"rollingDays": 7, "min": 1})),
        rule("consecutiveDaysWorked", json!({"max": 4, "categories": ["early", "late", "night"], "minGapMinutes": 1440})),
        rule("weekendMinutes", json!({"period": "week", "max": 960})),
        rule("weekendsWorked", json!({"period": "month", "level": "preferred", "max": 2})),
        rule("multiDayPattern", json!({"stance": "preferred", "days": [
            {"work": true, "withTags": ["early"]},
            {"work": true, "withTags": ["early"]},
        ]})),
        rule("rotation", json!({"cycle": "blocks", "max": 7, "groups": [{"withTags": ["early", "late"]}, {"withTags": ["night"]}]})),
        rule("rotation", json!({"cycle": "blocks", "level": "preferred", "min": 2, "max": 5, "groups": [{"withTags": ["early", "late"]}, {"withTags": ["night"]}]})),
    ]);
    let agency = vec![
        rule("restBetween", json!({"min": 600, "withinMinutes": 1440})),
        rule("consecutiveShifts", json!({"max": 1, "linkMinutes": 600})),
        rule("minutesWorked", json!({"period": "schedule", "max": 4800})),
        rule("daysWorked", json!({"period": "week", "max": 4})),
        rule("weekendsWorked", json!({"period": "month", "max": 2})),
        rule(
            "consecutiveWeekendsOff",
            json!({"period": "schedule", "min": 1}),
        ),
        rule(
            "shiftTypes",
            json!({"period": "schedule", "categories": ["night"], "max": 0}),
        ),
        rule("travel", json!({"maxMeters": 4000})),
        // Agency staff work no nights; were one to, a free day follows,
        // then a return to work.
        rule(
            "multiDayPattern",
            json!({"stance": "required", "days": [
                {"work": true, "withTags": ["night"]},
                {"work": false},
                {"work": true},
            ]}),
        ),
        rule("overlapAllowed", json!({"withTags": ["support"]})),
        rule("shiftsWorked", json!({"period": "week", "max": 5})),
        rule("startSpread", json!({"period": "week", "max": 600})),
        rule("locationsWorked", json!({"period": "week", "max": 1})),
        rule(
            "consecutiveDaysWorked",
            json!({"max": 4, "startDays": ["monday", "tuesday", "wednesday", "thursday", "friday"]}),
        ),
        rule(
            "rotation",
            json!({"cycle": "weekly", "groups": [{"withTags": ["early"]}, {"withTags": ["late"]}]}),
        ),
        // Agency staff who take a support shift run the afternoon clinic too.
        rule(
            "dayPattern",
            json!({"stance": "required", "steps": [{"withTags": ["support"]}, {"withTags": ["clinic"]}]}),
        ),
        rule(
            "dailyPairing",
            json!({"stance": "required", "withTags": ["support"], "paired": {"withTags": ["clinic"]}}),
        ),
    ];
    json!([
        {"id": "full-time", "rules": full},
        {"id": "part-time", "rules": part},
        {"id": "agency", "rules": agency},
    ])
}
