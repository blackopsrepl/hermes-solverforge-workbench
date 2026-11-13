//! `SMALL_WEEK`: one hospital ward, ten nurses, one week.

use super::rng::Rng;
use crate::domain::{Employee, Plan, Shift};
use crate::time::{format_date, format_date_time, parse_date, DAY};
use crate::wire::{
    Contract, Level, Rule, RuleKind, Schedule, SkillRef, Stance, TagFilter, TimeSpan, Weekday,
};

const FIRST_DAY: &str = "2027-03-01";
const NAMES: [&str; 10] = [
    "Amina", "Bruno", "Chiara", "Dmitri", "Elif", "Farah", "Goran", "Hana", "Ivo", "Jun",
];

fn at(day: i64, minute: i64) -> String {
    format_date_time(day * DAY + minute)
}

fn span(day: i64, from: i64, to: i64) -> TimeSpan {
    TimeSpan {
        from: at(day, from),
        to: at(day, to),
        shifts: Default::default(),
        weight: 1,
    }
}

/// The week's dataset, nothing assigned except one pinned shift.
pub fn small_week() -> Plan {
    let first = parse_date(FIRST_DAY).expect("demo date");
    let mut rng = Rng::new(7);

    let employees: Vec<Employee> = NAMES
        .iter()
        .enumerate()
        .map(|(n, name)| {
            let mut employee = Employee::new(name.to_lowercase());
            employee.name = Some((*name).to_owned());
            employee.skills.push(SkillRef::Id("nurse".into()));
            employee
                .contracts
                .push(if n < 7 { "full-time" } else { "part-time" }.into());
            if n == 5 {
                employee.days_off.push(format_date(first + 3));
            }
            if n < 4 {
                employee.skills.push(SkillRef::Id("senior".into()));
            }
            if n % 3 == 1 {
                employee.skills.push(SkillRef::Id("wound-care".into()));
            }
            if n == 4 {
                employee.avoid_risks.push("radiation".into());
            }
            let off = first + rng.below(7) as i64;
            employee.unavailable.push(span(off, 0, DAY));
            if n % 2 == 0 {
                let day = first + rng.below(7) as i64;
                employee.preferred_times.push(span(day, 7 * 60, 15 * 60));
            } else {
                let day = first + rng.below(7) as i64;
                employee.unwanted_times.push(span(day, 15 * 60, 23 * 60));
            }
            employee
        })
        .collect();

    let mut shifts = Vec::new();
    for offset in 0..7 {
        let day = first + offset;
        let weekend = offset >= 5;
        let date = format_date(day);
        let add = |shifts: &mut Vec<Shift>, kind: &str, slot: usize, from: i64, to: i64| {
            let mut shift = Shift::new(format!("{date}-{kind}-{slot}"), at(day, from), at(day, to));
            shift.tags = vec![kind.to_owned(), "ward-a".to_owned()];
            shift.required_skills.push(SkillRef::Id("nurse".into()));
            shifts.push(shift);
            shifts.len() - 1
        };
        let early = if weekend { 1 } else { 2 };
        for slot in 0..early {
            let i = add(&mut shifts, "early", slot, 7 * 60, 15 * 60);
            if slot == 0 {
                shifts[i]
                    .required_skills
                    .push(SkillRef::Id("senior".into()));
            }
            shifts[i]
                .preferred_skills
                .push(SkillRef::Id("wound-care".into()));
        }
        for slot in 0..early {
            add(&mut shifts, "late", slot, 15 * 60, 23 * 60);
        }
        let night = add(&mut shifts, "night", 0, 23 * 60, DAY + 7 * 60);
        shifts[night].risks.push("radiation".into());
        if !weekend {
            let extra = add(&mut shifts, "support", 0, 9 * 60, 13 * 60);
            shifts[extra].optional = true;
            shifts[extra].importance = 2;
        }
    }
    // The first early shift is already promised to Amina.
    shifts[0].employee = Some("amina".into());
    shifts[0].pinned = true;

    Plan {
        schedule: Schedule {
            start: at(first, 0),
            end: at(first + 7, 0),
            week_start: Weekday::Monday,
            weekend: vec![Weekday::Saturday, Weekday::Sunday],
            periods: Vec::new(),
        },
        tags: Vec::new(),
        contracts: contracts(),
        schedule_rules: Default::default(),
        teams: Vec::new(),
        cost_groups: Vec::new(),
        alternatives: Vec::new(),
        employees,
        shifts,
        days: Vec::new(),
        windows: Vec::new(),
        choices: Vec::new(),
        wholes: Vec::new(),
        weights: Default::default(),
        score: None,
    }
}

fn rule(kind: RuleKind) -> Rule {
    Rule::new(kind)
}

/// Full-time and part-time contracts with rest and weekly hour limits.
fn contracts() -> Vec<Contract> {
    let rest = Rule {
        max: Some(1),
        link_minutes: Some(600),
        ..rule(RuleKind::ConsecutiveShifts)
    };
    let streak = Rule {
        max: Some(5),
        ..rule(RuleKind::ConsecutiveDaysWorked)
    };
    let near_off = Rule {
        stance: Some(Stance::Unwanted),
        before: Some(TagFilter {
            with_tags: vec!["night".into()],
            ..Default::default()
        }),
        ..rule(RuleKind::NearDayOff)
    };
    let weekly = |min: i64, max: i64| {
        vec![
            Rule {
                period: Some("week".into()),
                max: Some(max),
                ..rule(RuleKind::MinutesWorked)
            },
            Rule {
                period: Some("week".into()),
                level: Level::Preferred,
                min: Some(min),
                ..rule(RuleKind::MinutesWorked)
            },
        ]
    };
    let mut full = weekly(1440, 2400);
    full.extend([rest.clone(), streak.clone(), near_off.clone()]);
    let mut part = weekly(480, 1440);
    part.extend([rest, streak, near_off]);
    vec![
        Contract {
            id: "full-time".into(),
            rules: full,
        },
        Contract {
            id: "part-time".into(),
            rules: part,
        },
    ]
}
