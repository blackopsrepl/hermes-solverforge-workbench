//! Period and rolling windows in plan minutes.

use std::collections::HashMap;

use crate::time::{civil_from_days, days_from_civil, DAY};

/// Calendar facts needed to lay out rule windows.
pub struct Calendar {
    /// Calendar day of plan day 0.
    pub origin: i64,
    /// Plan days covered by the data.
    pub days: i64,
    pub horizon_start: i64,
    pub horizon_end: i64,
    pub week_start: u8,
    /// Custom periods: windows in plan minutes.
    pub custom: HashMap<String, Vec<(i64, i64)>>,
}

impl Calendar {
    fn first_day(&self) -> i64 {
        self.horizon_start.div_euclid(DAY)
    }

    fn end_day(&self) -> i64 {
        (self.horizon_end - 1).div_euclid(DAY) + 1
    }

    /// Windows of a named period that overlap the planning horizon, or `None`
    /// for an unknown period name.
    pub fn period(&self, name: &str) -> Option<Vec<(i64, i64)>> {
        let (first, end) = (self.first_day(), self.end_day());
        let windows = match name {
            "day" => (first..end).map(|d| (d * DAY, (d + 1) * DAY)).collect(),
            "week" => {
                let weekday = crate::time::weekday(self.origin + first) as i64;
                let mut start = first - (weekday - self.week_start as i64).rem_euclid(7);
                let mut out = Vec::new();
                while start < end {
                    out.push((start * DAY, (start + 7) * DAY));
                    start += 7;
                }
                out
            }
            "month" | "year" => {
                let monthly = name == "month";
                let (mut year, mut month, _) = civil_from_days(self.origin + first);
                if !monthly {
                    month = 1;
                }
                let mut out = Vec::new();
                loop {
                    let start = days_from_civil(year, month, 1) - self.origin;
                    if start >= end {
                        break;
                    }
                    let (next_year, next_month) = if monthly {
                        if month == 12 {
                            (year + 1, 1)
                        } else {
                            (year, month + 1)
                        }
                    } else {
                        (year + 1, 1)
                    };
                    let stop = days_from_civil(next_year, next_month, 1) - self.origin;
                    out.push((start * DAY, stop * DAY));
                    (year, month) = (next_year, next_month);
                }
                out
            }
            "schedule" => vec![(self.horizon_start, self.horizon_end)],
            custom => self
                .custom
                .get(custom)?
                .iter()
                .copied()
                .filter(|&(s, e)| s < self.horizon_end && e > self.horizon_start)
                .collect(),
        };
        Some(windows)
    }

    /// Windows of `size` days starting on every day, each ending inside the
    /// data and touching the horizon.
    pub fn rolling(&self, size: i64) -> Vec<(i64, i64)> {
        let (first, end) = (self.first_day(), self.end_day());
        let from = (first - size + 1).max(0);
        let to = end.min(self.days.max(end)) - size;
        let windows: Vec<_> = (from..=to).map(|d| (d * DAY, (d + size) * DAY)).collect();
        if windows.is_empty() {
            vec![(first * DAY, (first + size) * DAY)]
        } else {
            windows
        }
    }
}
