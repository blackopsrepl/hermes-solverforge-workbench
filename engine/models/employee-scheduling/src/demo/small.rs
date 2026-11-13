//! `SMALL_WEEK`: one hospital ward, ten nurses, one week.

use super::rng::Rng;
use crate::domain::{Employee, Plan, Shift};
use crate::time::{format_date, format_date_time, parse_date, DAY};
use crate::wire::{Schedule, SkillRef, TimeSpan, Weekday};

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
            if n < 3 {
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
        employees,
        shifts,
        weights: Default::default(),
        score: None,
    }
}
