//! The eighteen rules: names, levels and whether they reward.

use solverforge_workbench_host::Level;

/// One constraint of the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Rule {
    EarliestStart,
    Deadline,
    DependencyUnassigned,
    DependencyTiming,
    DependencySplit,
    DependencyInterrupted,
    TagMissing,
    ResourceNotAllowed,
    ForbiddenHandOver,
    PartsShort,
    Unassigned,
    Makespan,
    IdealEnd,
    Lateness,
    ChangeoverTime,
    ChangeoverCount,
    IdleTime,
    PreferredHandOver,
}

/// Every rule, in scoring order.
pub const RULES: [Rule; 18] = [
    Rule::EarliestStart,
    Rule::Deadline,
    Rule::DependencyUnassigned,
    Rule::DependencyTiming,
    Rule::DependencySplit,
    Rule::DependencyInterrupted,
    Rule::TagMissing,
    Rule::ResourceNotAllowed,
    Rule::ForbiddenHandOver,
    Rule::PartsShort,
    Rule::Unassigned,
    Rule::Makespan,
    Rule::IdealEnd,
    Rule::Lateness,
    Rule::ChangeoverTime,
    Rule::ChangeoverCount,
    Rule::IdleTime,
    Rule::PreferredHandOver,
];

impl Rule {
    /// The constraint's name, as weights and analysis know it.
    pub const fn name(self) -> &'static str {
        match self {
            Rule::EarliestStart => "Started before earliest start",
            Rule::Deadline => "Finished after deadline",
            Rule::DependencyUnassigned => "Dependency left unassigned",
            Rule::DependencyTiming => "Dependency offset broken",
            Rule::DependencySplit => "Dependency split across resources",
            Rule::DependencyInterrupted => "Dependency interrupted by other tasks",
            Rule::TagMissing => "Required tag missing",
            Rule::ResourceNotAllowed => "Resource not allowed",
            Rule::ForbiddenHandOver => "Forbidden resource hand-over",
            Rule::PartsShort => "Parts out of stock",
            Rule::Unassigned => "Task left unassigned",
            Rule::Makespan => "Makespan",
            Rule::IdealEnd => "Distance from ideal end",
            Rule::Lateness => "Lateness after due date",
            Rule::ChangeoverTime => "Changeover time",
            Rule::ChangeoverCount => "Changeover count",
            Rule::IdleTime => "Machine idle time",
            Rule::PreferredHandOver => "Preferred resource hand-over",
        }
    }

    pub const fn level(self) -> Level {
        match self {
            Rule::Unassigned => Level::Medium,
            Rule::Makespan
            | Rule::IdealEnd
            | Rule::Lateness
            | Rule::ChangeoverTime
            | Rule::ChangeoverCount
            | Rule::IdleTime
            | Rule::PreferredHandOver => Level::Soft,
            _ => Level::Hard,
        }
    }

    pub const fn index(self) -> usize {
        self as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_are_listed_in_index_order() {
        for (index, rule) in RULES.into_iter().enumerate() {
            assert_eq!(rule.index(), index);
            assert_eq!(
                RULES
                    .iter()
                    .filter(|other| other.name() == rule.name())
                    .count(),
                1
            );
        }
    }
}
