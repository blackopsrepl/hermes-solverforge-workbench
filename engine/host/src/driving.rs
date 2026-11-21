//! Rests the law asks of drivers after so long at the wheel.

use serde::{Deserialize, Serialize};

use crate::data::{col, Column};

/// A rest of `minutes` after every `afterMinutes` of driving. A stop at
/// least as long, a break or waiting at a visit, counts as the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DrivingRest {
    pub after_minutes: i64,
    pub minutes: i64,
}

impl DrivingRest {
    /// `(limit, pause)` in seconds, or why the rest cannot be read.
    pub fn seconds(&self, what: &str) -> Result<(i64, i64), String> {
        if self.after_minutes <= 0 || self.minutes < 0 {
            return Err(format!(
                "{what}: drivingRest needs afterMinutes above zero and minutes not below zero"
            ));
        }
        Ok((self.after_minutes * 60, self.minutes * 60))
    }
}

/// The rests a drive of `travel` seconds takes after `driven` seconds at the
/// wheel since the last rest, under `rest` (`(limit, pause)` seconds): the
/// seconds resting, and the seconds driven since the last rest on arrival.
/// Without a rule the driver never rests on the road.
pub fn on_the_road(rest: Option<(i64, i64)>, driven: i64, travel: i64) -> (i64, i64) {
    match rest {
        Some((limit, pause)) if limit > 0 => {
            let total = driven + travel;
            let stops = if total > limit {
                (total - 1) / limit
            } else {
                0
            };
            (stops * pause, total - stops * limit)
        }
        _ => (0, driven + travel),
    }
}

/// Seconds driven since the last rest after a stop of `idle` seconds: a
/// stop as long as the rest counts as one.
pub fn after_stop(rest: Option<(i64, i64)>, driven: i64, idle: i64) -> i64 {
    match rest {
        Some((_, pause)) if idle >= pause => 0,
        _ => driven,
    }
}

/// The driving rest of a shift, for the page's data editor.
pub fn column() -> Column {
    col("drivingRest", "Rest after driving", "object")
        .fields(vec![
            col("afterMinutes", "After driving", "minutes").unit("minutes"),
            col("minutes", "Rest", "minutes").unit("minutes"),
        ])
        .optional()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_driver_rests_each_time_the_limit_is_passed_and_not_at_it() {
        let rest = Some((7200, 1800));
        assert_eq!(on_the_road(rest, 0, 7200), (0, 7200));
        assert_eq!(on_the_road(rest, 0, 7201), (1800, 1));
        assert_eq!(on_the_road(rest, 3600, 18_000), (2 * 1800, 7200));
        assert_eq!(on_the_road(None, 3600, 18_000), (0, 21_600));
        assert_eq!(after_stop(rest, 5000, 1800), 0);
        assert_eq!(after_stop(rest, 5000, 1799), 5000);
    }
}
