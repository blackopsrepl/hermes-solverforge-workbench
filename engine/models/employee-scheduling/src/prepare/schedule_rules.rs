//! Compiles schedule-wide rules into windows and concurrency limits.

use super::{Builder, Calendar};
use crate::derived::{Concurrency, GlobalKind, GlobalWindow, Rate, ShiftInfo, Slot, TagMatch};
use crate::time::DAY;
use crate::wire::{Level, ScheduleRules, TagFilter};

/// Most schedule-wide windows one shift may count toward.
pub const MAX_WINDOWS_PER_SHIFT: usize = 32;

#[allow(clippy::too_many_arguments)]
fn add_windows(
    b: &mut Builder,
    calendar: &Calendar,
    shifts: &[ShiftInfo],
    out: &mut (Vec<GlobalWindow>, Vec<Vec<u32>>),
    what: &str,
    period: &str,
    filter: &TagFilter,
    template: GlobalWindow,
) {
    let Some(periods) = calendar.period(period) else {
        b.error(
            "unknown_period",
            format!("{what} names unknown period {period:?}"),
        );
        return;
    };
    if let (Some(min), Some(max)) = (template.min, template.max) {
        if min > max {
            b.error(
                "rule_min_above_max",
                format!("{what} has min {min} above max {max}"),
            );
            return;
        }
    }
    if template.min.is_none() && template.max.is_none() && template.kind != GlobalKind::CostTotal {
        b.error("rule_limit_missing", format!("{what} needs a min or a max"));
        return;
    }
    let filter: TagMatch = b.tag_match(filter);
    for (start, end) in periods {
        let index = out.0.len() as u32;
        out.0.push(GlobalWindow {
            start,
            end,
            ..template.clone()
        });
        for (s, info) in shifts.iter().enumerate() {
            if info.start >= start && info.start < end && filter.accepts(&info.tags) {
                out.1[s].push(index);
            }
        }
    }
}

/// Compiled schedule-wide rules.
pub struct Compiled {
    pub windows: Vec<GlobalWindow>,
    pub shift_windows: Vec<Vec<u32>>,
    pub concurrency: Vec<Concurrency>,
    pub slots: Vec<Slot>,
    pub slots_by_day: Vec<Vec<u32>>,
}

/// Windows, per-shift window lists, concurrency limits and demand slots of
/// the schedule rules.
pub fn compile_schedule_rules(
    b: &mut Builder,
    calendar: &Calendar,
    rules: &ScheduleRules,
    shifts: &[ShiftInfo],
) -> Compiled {
    let mut out = (Vec::new(), vec![Vec::new(); shifts.len()]);
    for (n, rule) in rules.costs.iter().enumerate() {
        let what = format!("schedule cost rule #{}", n + 1);
        let employees = rule.employees.as_ref().map(|f| b.tag_match(f));
        let rates = rule
            .rates
            .iter()
            .map(|rate| Rate {
                employee_group: rate.employee_group.as_ref().map(|g| b.intern(g)),
                shift_group: rate.shift_group.as_ref().map(|g| b.intern(g)),
                cents: super::contracts::cents(rate.hourly),
            })
            .collect();
        let preferred = rule.level == Level::Preferred;
        let total = rule.min.is_none() && rule.max.is_none();
        if total && !preferred {
            b.error(
                "rule_limit_missing",
                format!("{what} is required, so it needs a min or a max"),
            );
            continue;
        }
        let template = GlobalWindow {
            kind: if total {
                GlobalKind::CostTotal
            } else {
                GlobalKind::Cost
            },
            preferred: rule.level == Level::Preferred,
            weight: rule.weight.max(0),
            start: 0,
            end: 0,
            employees,
            tags: Vec::new(),
            rates,
            // Limits are in currency units, costs in cents.
            min: rule.min.map(|v| v * 100),
            max: rule.max.map(|v| v * 100),
        };
        add_windows(
            b,
            calendar,
            shifts,
            &mut out,
            &what,
            &rule.period,
            &rule.shifts,
            template,
        );
    }
    for (n, rule) in rules.shift_counts.iter().enumerate() {
        let what = format!("schedule shift count rule #{}", n + 1);
        let employees = rule.employees.as_ref().map(|f| b.tag_match(f));
        let template = GlobalWindow {
            kind: GlobalKind::Shifts,
            preferred: rule.level == Level::Preferred,
            weight: rule.weight.max(0),
            start: 0,
            end: 0,
            employees,
            tags: Vec::new(),
            rates: Vec::new(),
            min: rule.min,
            max: rule.max,
        };
        add_windows(
            b,
            calendar,
            shifts,
            &mut out,
            &what,
            &rule.period,
            &rule.shifts,
            template,
        );
    }
    for (n, rule) in rules.distinct_tags.iter().enumerate() {
        let what = format!("schedule distinct tags rule #{}", n + 1);
        let tags = b.tags_of_types(&rule.tag_types, &what);
        if tags.is_empty() {
            b.error(
                "rule_needs_categories",
                format!("{what} needs tag types with tags"),
            );
            continue;
        }
        let template = GlobalWindow {
            kind: GlobalKind::DistinctTags,
            preferred: rule.level == Level::Preferred,
            weight: rule.weight.max(0),
            start: 0,
            end: 0,
            employees: None,
            tags,
            rates: Vec::new(),
            min: rule.min,
            max: rule.max,
        };
        add_windows(
            b,
            calendar,
            shifts,
            &mut out,
            &what,
            &rule.period,
            &rule.shifts,
            template,
        );
    }
    if out.1.iter().any(|list| list.len() > MAX_WINDOWS_PER_SHIFT) {
        b.error(
            "too_many_windows",
            format!(
                "a shift counts toward more than {MAX_WINDOWS_PER_SHIFT} schedule-wide periods"
            ),
        );
        for list in &mut out.1 {
            list.truncate(MAX_WINDOWS_PER_SHIFT);
        }
    }
    let mut concurrency = Vec::new();
    for (n, rule) in rules.concurrency.iter().enumerate() {
        let what = format!("concurrency rule #{}", n + 1);
        if rule.max < 1 {
            b.error(
                "rule_negative_limit",
                format!("{what} must allow at least one shift"),
            );
            continue;
        }
        let from = match &rule.from {
            Some(text) => b.minutes(text, || what.clone()).unwrap_or(i64::MIN),
            None => i64::MIN,
        };
        let to = match &rule.to {
            Some(text) => b.minutes(text, || what.clone()).unwrap_or(i64::MAX),
            None => i64::MAX,
        };
        let shifts = b.tag_match(&rule.shifts);
        concurrency.push(Concurrency {
            max: rule.max,
            shifts,
            from,
            to,
        });
    }
    let mut slots = Vec::new();
    let mut slots_by_day = vec![Vec::new(); calendar.days.max(0) as usize];
    for (n, rule) in rules.demand.iter().enumerate() {
        let what = format!("demand rule #{}", n + 1);
        let shifts = b.tag_match(&rule.shifts);
        let employees = rule.employees.as_ref().map(|f| b.tag_match(f));
        for slot in &rule.slots {
            let start = b.minutes(&slot.from, || what.clone());
            let end = b.minutes(&slot.to, || what.clone());
            let (Some(start), Some(end)) = (start, end) else {
                continue;
            };
            if end <= start || end - start > DAY {
                b.error(
                    "demand_slot_invalid",
                    format!(
                        "{what} has a slot from {} that is empty or longer than a day",
                        slot.from
                    ),
                );
                continue;
            }
            if slot.min.is_none() && slot.max.is_none() {
                b.error(
                    "rule_limit_missing",
                    format!("{what} has a slot without min or max"),
                );
                continue;
            }
            let day = start.div_euclid(DAY);
            if day < 0 || day >= calendar.days {
                b.warning(
                    "demand_outside_schedule",
                    format!(
                        "{what} has a slot from {} outside the data, ignored",
                        slot.from
                    ),
                );
                continue;
            }
            slots_by_day[day as usize].push(slots.len() as u32);
            slots.push(Slot {
                preferred: rule.level == Level::Preferred,
                start,
                end,
                shifts: shifts.clone(),
                employees: employees.clone(),
                min: slot.min,
                max: slot.max,
            });
        }
    }
    Compiled {
        windows: out.0,
        shift_windows: out.1,
        concurrency,
        slots,
        slots_by_day,
    }
}
