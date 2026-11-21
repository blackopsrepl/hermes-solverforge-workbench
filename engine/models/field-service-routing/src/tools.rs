//! Shared tools: equipment several technicians use in turn, such as a lift
//! or a test rig.
//!
//! The plan lists its `tools` and how many of each it owns; a visit names the
//! tools it needs. A tool serves one visit at a time and is carried from one
//! visit's site to the next, so the next visit that needs it can start only
//! once it has been brought over.

use std::sync::Arc;

/// The plan's tools and the drive between its places.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Tools {
    /// How many units of each tool the plan owns.
    pub counts: Vec<usize>,
    /// Places in the travel matrix.
    pub places: usize,
    /// Seconds to carry a tool from one place to another: `from * places + to`.
    pub seconds: Vec<i64>,
}

/// The tools one visit needs, where it is, and the plan's tools.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ToolUse {
    pub tools: Vec<usize>,
    pub place: usize,
    pub shared: Arc<Tools>,
}

/// A planned visit's start, end and tools.
pub type Used = (i64, i64, Arc<ToolUse>);

/// Seconds visits wait past their start for a tool to be free and brought
/// over. Each tool's visits take its units in start order, each the unit
/// that reaches it first.
pub fn waiting<'a>(visits: impl Iterator<Item = &'a Used>) -> i64 {
    let visits: Vec<&Used> = visits.collect();
    let Some((_, _, first)) = visits.first() else {
        return 0;
    };
    let shared = Arc::clone(&first.shared);
    let carry = |from: usize, to: usize| shared.seconds[from * shared.places + to];
    let mut late = 0;
    for (tool, &count) in shared.counts.iter().enumerate() {
        let mut uses: Vec<(i64, i64, usize)> = visits
            .iter()
            .filter(|(_, _, u)| u.tools.contains(&tool))
            .map(|(start, end, u)| (*start, *end, u.place))
            .collect();
        if uses.len() < 2 {
            continue;
        }
        uses.sort_unstable();
        // Each unit: when and where it is free; unused units are free anywhere.
        let mut units: Vec<Option<(i64, usize)>> = vec![None; count.max(1)];
        for (start, end, place) in uses {
            let (unit, ready) = units
                .iter()
                .enumerate()
                .map(|(i, free)| match free {
                    None => (i, i64::MIN),
                    Some((at, from)) => (i, at + carry(*from, place)),
                })
                .min_by_key(|&(_, ready)| ready)
                .expect("a tool has at least one unit");
            let wait = ready.saturating_sub(start).max(0);
            late += wait;
            units[unit] = Some((end + wait, place));
        }
    }
    late
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shared(count: usize) -> Arc<Tools> {
        // Two places 600 s apart.
        Arc::new(Tools {
            counts: vec![count],
            places: 2,
            seconds: vec![0, 600, 600, 0],
        })
    }

    fn at(start: i64, end: i64, place: usize, shared: &Arc<Tools>) -> Used {
        (
            start,
            end,
            Arc::new(ToolUse {
                tools: vec![0],
                place,
                shared: Arc::clone(shared),
            }),
        )
    }

    #[test]
    fn a_tool_serves_one_visit_at_a_time_and_is_carried_between_sites() {
        let one = shared(1);
        // 0-3600 at place 0, then at place 1 from 3600: it arrives at 4200.
        let tight = [at(0, 3600, 0, &one), at(3600, 7200, 1, &one)];
        assert_eq!(waiting(tight.iter()), 600);
        let room = [at(0, 3600, 0, &one), at(4200, 7200, 1, &one)];
        assert_eq!(waiting(room.iter()), 0);
        // Same site: overlap only.
        let overlap = [at(0, 3600, 0, &one), at(1800, 5400, 0, &one)];
        assert_eq!(waiting(overlap.iter()), 1800);
    }

    #[test]
    fn a_second_unit_serves_a_visit_at_the_same_time() {
        let two = shared(2);
        let overlap = [at(0, 3600, 0, &two), at(1800, 5400, 1, &two)];
        assert_eq!(waiting(overlap.iter()), 0);
    }
}
