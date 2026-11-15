//! A resource's unavailable spans and the time arithmetic over them.

/// Sorted, merged, half-open spans in which a resource cannot work.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Calendar {
    spans: Vec<(i64, i64)>,
}

impl Calendar {
    /// A calendar from spans in any order; overlapping and touching spans merge.
    pub fn new(mut spans: Vec<(i64, i64)>) -> Self {
        spans.retain(|(start, end)| end > start);
        spans.sort_unstable();
        let mut merged: Vec<(i64, i64)> = Vec::with_capacity(spans.len());
        for (start, end) in spans {
            match merged.last_mut() {
                Some(last) if start <= last.1 => last.1 = last.1.max(end),
                _ => merged.push((start, end)),
            }
        }
        Self { spans: merged }
    }

    pub fn spans(&self) -> &[(i64, i64)] {
        &self.spans
    }

    /// Index of the first span that ends after `t`.
    fn after(&self, t: i64) -> usize {
        self.spans.partition_point(|&(_, end)| end <= t)
    }

    /// The first minute at or after `t` the resource is available.
    pub fn available_from(&self, t: i64) -> i64 {
        match self.spans.get(self.after(t)) {
            Some(&(start, end)) if start <= t => end,
            _ => t,
        }
    }

    /// When work of `minutes` that starts at `t` is done, pausing over
    /// unavailable spans.
    pub fn finish(&self, t: i64, minutes: i64) -> i64 {
        let mut t = t;
        let mut left = minutes.max(0);
        let mut i = self.after(t);
        loop {
            if let Some(&(start, end)) = self.spans.get(i) {
                if start <= t {
                    t = end;
                    i += 1;
                    continue;
                }
            }
            if left == 0 {
                return t;
            }
            let free_until = self.spans.get(i).map_or(i64::MAX, |&(start, _)| start);
            if free_until - t >= left {
                return t + left;
            }
            left -= free_until - t;
            t = self.spans[i].1;
            i += 1;
        }
    }

    /// The first start at or after `t` from which `minutes` of work runs
    /// without meeting an unavailable span.
    pub fn uninterrupted_from(&self, t: i64, minutes: i64) -> i64 {
        let mut t = self.available_from(t);
        loop {
            match self.spans.get(self.after(t)) {
                Some(&(start, end)) if start < t + minutes.max(0) => {
                    t = end;
                }
                _ => return t,
            }
        }
    }

    /// Unavailable minutes inside `[from, to)`.
    pub fn unavailable_between(&self, from: i64, to: i64) -> i64 {
        if to <= from {
            return 0;
        }
        self.spans[self.after(from)..]
            .iter()
            .take_while(|&&(start, _)| start < to)
            .map(|&(start, end)| end.min(to) - start.max(from))
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lunch() -> Calendar {
        // 12:00–13:00 and 15:00–15:15, in minutes of the day.
        Calendar::new(vec![(900, 915), (720, 780)])
    }

    #[test]
    fn work_pauses_over_unavailable_spans() {
        let c = lunch();
        // A 6 h job from 10:00 ends 17:15 (the documented example).
        assert_eq!(c.finish(600, 360), 1035);
        assert_eq!(c.finish(600, 120), 720);
        assert_eq!(c.finish(730, 10), 790);
        assert_eq!(c.finish(730, 0), 780);
    }

    #[test]
    fn uninterrupted_work_waits_for_a_gap() {
        let c = lunch();
        assert_eq!(c.uninterrupted_from(600, 120), 600);
        assert_eq!(c.uninterrupted_from(650, 100), 780);
        assert_eq!(c.uninterrupted_from(600, 150), 915);
        assert_eq!(c.uninterrupted_from(780, 150), 915);
        assert_eq!(c.available_from(750), 780);
    }

    #[test]
    fn spans_merge_and_count() {
        let c = Calendar::new(vec![(10, 20), (15, 30), (30, 40), (50, 50)]);
        assert_eq!(c.spans(), &[(10, 40)]);
        assert_eq!(c.unavailable_between(0, 100), 30);
        assert_eq!(c.unavailable_between(20, 35), 15);
    }
}
