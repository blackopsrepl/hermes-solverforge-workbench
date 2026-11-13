//! Compiles contract rules into per-employee rule tables.

use super::{Builder, Calendar};
use crate::derived::{DayRunRule, Measure, NearDayOffRule, ShiftRunRule, WindowRule};
use crate::wire::{Contract, Level, Rule, RuleKind, Stance, Weekday};

/// One contract's rules, compiled, with weights not yet scaled by priority.
#[derive(Default, Clone)]
pub struct Compiled {
    pub windows: Vec<WindowRule>,
    pub day_runs: Vec<DayRunRule>,
    pub shift_runs: Vec<ShiftRunRule>,
    pub near_day_off: Vec<NearDayOffRule>,
}

impl Compiled {
    /// Adds `other`'s rules with weights times `priority`.
    pub fn extend_scaled(&mut self, other: &Compiled, priority: i64) {
        self.windows
            .extend(other.windows.iter().cloned().map(|mut r| {
                r.weight *= priority;
                r
            }));
        self.day_runs
            .extend(other.day_runs.iter().cloned().map(|mut r| {
                r.weight *= priority;
                r
            }));
        self.shift_runs
            .extend(other.shift_runs.iter().cloned().map(|mut r| {
                r.weight *= priority;
                r
            }));
        self.near_day_off
            .extend(other.near_day_off.iter().cloned().map(|mut r| {
                r.weight *= priority;
                r
            }));
    }
}

pub fn weekday_mask(days: &[Weekday]) -> u8 {
    days.iter().fold(0, |mask, day| mask | 1 << day.number())
}

#[derive(Clone, Copy, PartialEq)]
enum Window {
    Period,
    Rolling,
    Either,
}

fn window_kind(kind: RuleKind) -> Option<(Measure, Window)> {
    use RuleKind as K;
    Some(match kind {
        K::MinutesWorked => (Measure::Minutes, Window::Either),
        K::MinutesLogged => (Measure::Logged, Window::Period),
        K::DaysWorked => (Measure::Days, Window::Either),
        K::ShiftsWorked => (Measure::Shifts, Window::Either),
        K::WeekendsWorked => (Measure::Weekends, Window::Either),
        K::WeekendMinutes => (Measure::WeekendMinutes, Window::Period),
        K::ConsecutiveWeekendsWorked => (Measure::ConsecutiveWeekends, Window::Period),
        K::DaysOff => (Measure::DaysOff, Window::Period),
        K::ConsecutiveDaysOff => (Measure::ConsecutiveDaysOff, Window::Either),
        K::ConsecutiveMinutesOff => (Measure::MinutesOff, Window::Rolling),
        K::ConsecutiveWeekendsOff => (Measure::ConsecutiveWeekendsOff, Window::Period),
        _ => return None,
    })
}

/// Compiles every rule of `contract`, reporting problems by contract and rule number.
pub fn compile(b: &mut Builder, calendar: &Calendar, contract: &Contract) -> Compiled {
    let mut out = Compiled::default();
    for (n, rule) in contract.rules.iter().enumerate() {
        let what = format!("contract {} rule #{} ({:?})", contract.id, n + 1, rule.kind);
        compile_rule(b, calendar, rule, &what, &mut out);
    }
    out
}

fn limits_ok(b: &mut Builder, rule: &Rule, what: &str) -> bool {
    if let (Some(min), Some(max)) = (rule.min, rule.max) {
        if min > max {
            b.error(
                "rule_min_above_max",
                format!("{what} has min {min} above max {max}"),
            );
            return false;
        }
    }
    if rule.min.is_some_and(|v| v < 0) || rule.max.is_some_and(|v| v < 0) {
        b.error(
            "rule_negative_limit",
            format!("{what} has a negative limit"),
        );
        return false;
    }
    true
}

fn compile_rule(b: &mut Builder, calendar: &Calendar, rule: &Rule, what: &str, out: &mut Compiled) {
    if rule.weight < 0 {
        b.error("negative_weight", format!("{what} has a negative weight"));
        return;
    }
    if !limits_ok(b, rule, what) {
        return;
    }
    let preferred = rule.level == Level::Preferred;
    let shifts = b.tag_match(&rule.shifts);
    if let Some((measure, allowed)) = window_kind(rule.kind) {
        let windows = match (&rule.period, rule.rolling_days) {
            (Some(_), Some(_)) => {
                b.error(
                    "rule_window_conflict",
                    format!("{what} names both a period and rolling days"),
                );
                return;
            }
            (Some(period), None) if allowed != Window::Rolling => match calendar.period(period) {
                Some(windows) => (windows, false),
                None => {
                    b.error(
                        "unknown_period",
                        format!("{what} names unknown period {period:?}"),
                    );
                    return;
                }
            },
            (None, Some(days)) if allowed != Window::Period => {
                if days < 1 {
                    b.error(
                        "invalid_rolling_days",
                        format!("{what} has rolling days {days}; it must be at least 1"),
                    );
                    return;
                }
                (calendar.rolling(days), true)
            }
            _ => {
                let expected = match allowed {
                    Window::Period => "a period",
                    Window::Rolling => "rolling days",
                    Window::Either => "a period or rolling days",
                };
                b.error("rule_window_missing", format!("{what} needs {expected}"));
                return;
            }
        };
        let needs_min = matches!(
            measure,
            Measure::ConsecutiveDaysOff | Measure::MinutesOff | Measure::ConsecutiveWeekendsOff
        );
        if (needs_min && rule.min.is_none()) || (rule.min.is_none() && rule.max.is_none()) {
            let needed = if needs_min { "a min" } else { "a min or a max" };
            b.error("rule_limit_missing", format!("{what} needs {needed}"));
            return;
        }
        out.windows.push(WindowRule {
            measure,
            preferred,
            rolling: windows.1,
            weight: rule.weight,
            shifts,
            windows: windows.0,
            min: rule.min,
            max: if needs_min { None } else { rule.max },
            whole_shift: rule.whole_shift,
            weekdays: if rule.weekdays.is_empty() {
                0x7f
            } else {
                weekday_mask(&rule.weekdays)
            },
        });
        return;
    }
    match rule.kind {
        RuleKind::ConsecutiveDaysWorked => {
            if rule.min.is_none()
                && rule.max.is_none()
                && rule.start_days.is_empty()
                && rule.min_gap_minutes.is_none()
                && !rule.compact
            {
                b.error("rule_limit_missing", format!("{what} limits nothing"));
                return;
            }
            let categories = b.tag_set(&rule.categories);
            if (rule.min_gap_minutes.is_some() || rule.compact) && categories.is_empty() {
                b.error(
                    "rule_needs_categories",
                    format!("{what} uses gaps between runs or compact runs, which need categories"),
                );
                return;
            }
            out.day_runs.push(DayRunRule {
                preferred,
                weight: rule.weight,
                shifts,
                min: rule.min,
                max: rule.max,
                categories,
                start_days: weekday_mask(&rule.start_days),
                min_gap: rule.min_gap_minutes,
                compact: rule.compact,
            });
        }
        RuleKind::ConsecutiveShifts => {
            if rule.min.is_none() && rule.max.is_none() && rule.rest_after_minutes.is_none() {
                b.error("rule_limit_missing", format!("{what} limits nothing"));
                return;
            }
            out.shift_runs.push(ShiftRunRule {
                preferred,
                weight: rule.weight,
                shifts,
                min: rule.min,
                max: rule.max,
                link: rule.link_minutes.unwrap_or(720).max(0),
                rest_after: rule.rest_after_minutes,
                rest_after_runs_of: rule.rest_after_runs_of.unwrap_or(1).max(1),
            });
        }
        RuleKind::NearDayOff => {
            let barred = match rule.stance {
                Some(Stance::Barred) => true,
                Some(Stance::Unwanted) => false,
                _ => {
                    b.error(
                        "rule_stance_invalid",
                        format!("{what} needs stance \"barred\" or \"unwanted\""),
                    );
                    return;
                }
            };
            if rule.before.is_none() && rule.after.is_none() {
                b.error(
                    "rule_limit_missing",
                    format!("{what} needs before or after shifts"),
                );
                return;
            }
            let before = rule.before.as_ref().map(|f| b.tag_match(f));
            let after = rule.after.as_ref().map(|f| b.tag_match(f));
            out.near_day_off.push(NearDayOffRule {
                barred,
                weight: rule.weight,
                before,
                after,
            });
        }
        _ => b.error("unsupported_rule", format!("{what} is not supported")),
    }
}
