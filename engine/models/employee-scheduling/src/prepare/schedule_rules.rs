//! Compiles schedule-wide rules into windows and concurrency limits.

use super::{Builder, Calendar};
use crate::derived::{Concurrency, GlobalKind, GlobalWindow, ShiftInfo, TagMatch};
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
    if template.min.is_none() && template.max.is_none() {
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

/// Windows, per-shift window lists and concurrency limits of the schedule rules.
pub fn compile_schedule_rules(
    b: &mut Builder,
    calendar: &Calendar,
    rules: &ScheduleRules,
    shifts: &[ShiftInfo],
) -> (Vec<GlobalWindow>, Vec<Vec<u32>>, Vec<Concurrency>) {
    let mut out = (Vec::new(), vec![Vec::new(); shifts.len()]);
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
    (out.0, out.1, concurrency)
}
