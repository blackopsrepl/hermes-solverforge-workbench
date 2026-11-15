//! Fleet readiness on one day, from the units the plan keeps down that day.

use super::UnitSet;

/// Which readiness level a gap is measured against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Floor {
    Minimum,
    Target,
}

/// Ready units missing on `day` below every rule's minimum or target,
/// summed over rules. Units away (not yet available) are never ready.
pub fn readiness_gap(day: usize, down: &UnitSet, floor: Floor) -> i64 {
    let Some(d) = down.derived() else {
        return 0;
    };
    let day = day as i64;
    let mut gap = 0;
    for rule in d.rules.iter().filter(|rule| rule.applies(day)) {
        let wanted = match floor {
            Floor::Minimum => rule.minimum,
            Floor::Target => rule.target,
        };
        if wanted <= 0 {
            continue;
        }
        let away = d
            .units
            .iter()
            .enumerate()
            .filter(|(u, info)| rule.counts(info.fleet) && d.away(*u, day))
            .count() as i64;
        let held = down
            .units()
            .filter(|&u| rule.counts(d.units[u].fleet) && !d.away(u, day))
            .count() as i64;
        gap += (wanted - (rule.scope - away - held)).max(0);
    }
    gap
}

/// What the units the plan keeps down on `day` cost in lost operations;
/// units away anyway cost nothing more.
pub fn downtime_cost(day: usize, down: &UnitSet) -> i64 {
    let Some(d) = down.derived() else {
        return 0;
    };
    down.units()
        .filter(|&u| !d.away(u, day as i64))
        .map(|u| d.units[u].downtime_cost)
        .sum()
}
