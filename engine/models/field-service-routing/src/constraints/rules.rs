//! The sixty-four rules: names, groups, levels, whether they reward, and
//! one sentence on what each costs.
//!
//! Rules up to [`VISIT_RULES`] are scored per visit from the amounts its
//! slot carries; the rest read a shift's last visit, a pair of visits, a
//! group or a vehicle.

use solverforge_workbench_host::Level;

/// One constraint of the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Rule {
    // Scored per visit, from its slot.
    DependencyDelay,
    DependencySameVehicle,
    DependencySameShift,
    DependencyPrerequisite,
    DependencyPooling,
    FixedBreakDelayed,
    FloatingBreakLate,
    FixedBreakConflict,
    LocatedBreakConflict,
    EarliestDay,
    CompletionRisk,
    OptionalUnplanned,
    MandatoryUnplanned,
    Profit,
    PinStart,
    PinShift,
    PinStartDelay,
    Disruption,
    MaxTravelPerVisit,
    Waiting,
    Colocated,
    Consecutive,
    TowardsEnd,
    ReachableVisit,
    SlaEnd,
    OptionalSla,
    SlaEarlier,
    MandatorySla,
    UnnecessarySkills,
    SkillsMissing,
    OutsidePreferredArea,
    OutsideArea,
    Rating,
    PreferredWindow,
    WindowEnd,
    WindowStart,
    PreferredVehicle,
    TagsMissing,
    NotRequiredVehicle,
    ProhibitedVehicle,
    PartsMissing,
    // Scored on other rows.
    Exclusion,
    DependentUnplanned,
    Fairness,
    MovableBalance,
    Loops,
    SemiAssigned,
    MaxServiceHard,
    MaxServiceSoft,
    MaxTravelHard,
    MaxTravelSoft,
    MaxVisitsHard,
    MaxVisitsSoft,
    TravelDistance,
    TravelTime,
    InactiveDistance,
    InactiveTime,
    ReachableItinerary,
    Overnight,
    ExcludesShifts,
    LastDepartureHard,
    LastDepartureSoft,
    EndHard,
    EndSoft,
    ShiftCost,
}

/// Rules scored per visit: the length of a slot's amounts.
pub const VISIT_RULES: usize = Rule::Exclusion as usize;

/// Every rule, in scoring order.
pub const RULES: [Rule; 65] = {
    use Rule::*;
    [
        DependencyDelay,
        DependencySameVehicle,
        DependencySameShift,
        DependencyPrerequisite,
        DependencyPooling,
        FixedBreakDelayed,
        FloatingBreakLate,
        FixedBreakConflict,
        LocatedBreakConflict,
        EarliestDay,
        CompletionRisk,
        OptionalUnplanned,
        MandatoryUnplanned,
        Profit,
        PinStart,
        PinShift,
        PinStartDelay,
        Disruption,
        MaxTravelPerVisit,
        Waiting,
        Colocated,
        Consecutive,
        TowardsEnd,
        ReachableVisit,
        SlaEnd,
        OptionalSla,
        SlaEarlier,
        MandatorySla,
        UnnecessarySkills,
        SkillsMissing,
        OutsidePreferredArea,
        OutsideArea,
        Rating,
        PreferredWindow,
        WindowEnd,
        WindowStart,
        PreferredVehicle,
        TagsMissing,
        NotRequiredVehicle,
        ProhibitedVehicle,
        PartsMissing,
        Exclusion,
        DependentUnplanned,
        Fairness,
        MovableBalance,
        Loops,
        SemiAssigned,
        MaxServiceHard,
        MaxServiceSoft,
        MaxTravelHard,
        MaxTravelSoft,
        MaxVisitsHard,
        MaxVisitsSoft,
        TravelDistance,
        TravelTime,
        InactiveDistance,
        InactiveTime,
        ReachableItinerary,
        Overnight,
        ExcludesShifts,
        LastDepartureHard,
        LastDepartureSoft,
        EndHard,
        EndSoft,
        ShiftCost,
    ]
};

pub const DEPENDENCIES: &str = "Dependencies between visits";
pub const FAIRNESS: &str = "Fairness";
pub const BREAKS: &str = "Lunch breaks and personal appointments";
pub const MULTI_DAY: &str = "Multi-day schedules and movable visits";
pub const MULTI_VEHICLE: &str = "Multi-vehicle visits";
pub const PRIORITY: &str = "Priority visits and optional visits";
pub const PROFIT: &str = "Profit maximization";
pub const REPLANNING: &str = "Replanning";
pub const ROUTES: &str = "Route optimization";
pub const SLA: &str = "Service level agreements";
pub const HOURS: &str = "Shift hours and overtime";
pub const SKILLS: &str = "Skills";
pub const COSTS: &str = "Technician costs";
pub const AREAS: &str = "Technician coverage area";
pub const RATING: &str = "Technician rating";
pub const WINDOWS: &str = "Time windows and opening hours";
pub const REQUIREMENTS: &str = "Visit requirements and tags";

/// Hard seconds one broken count-based rule costs (a missing tag or skill,
/// a vehicle the visit rules out, an unreachable leg), so it weighs like an
/// hour of lateness rather than a second of it.
pub const HOUR: i64 = 3_600;

/// Hard seconds added to every broken link (a dependency or group split, a
/// visit waiting on what comes after it), so it costs more than the
/// lateness it might save.
pub const BROKEN_LINK: i64 = 3_600;

/// Hard cost of a pinned visit left off its shift.
pub const PIN_SHIFT_COST: i64 = 10_000_000_000;

/// Hard cost per second a pinned visit starts after its allowed delay.
pub const PIN_START_WEIGHT: i64 = 100_000;

/// Soft cost per day a movable visit is served after its earliest day.
pub const DAY_COST: i64 = 1_000;

/// Soft cost of a replan moving a visit to another shift, and again to
/// another vehicle.
pub const MOVED: i64 = 100;

impl Rule {
    /// The constraint's name, as weights and analysis know it.
    pub const fn name(self) -> &'static str {
        use Rule::*;
        match self {
            DependencyDelay => "Require visit dependency delay",
            DependencySameVehicle => "Require the same vehicle for dependent visit",
            DependencySameShift => "Require the same vehicle shift for dependent visit",
            DependencyPrerequisite => "Require visit dependency prerequisite assigned",
            DependencyPooling => "Require visit dependency pooling type",
            FixedBreakDelayed => "Fixed break not delayed (hard)",
            FloatingBreakLate => "Max floating break end time (hard)",
            FixedBreakConflict => "No conflict with fixed break",
            LocatedBreakConflict => "No conflict with fixed location break",
            EarliestDay => "Prefer visits scheduled to the earliest day",
            CompletionRisk => "Minimize the visit completion risk",
            OptionalUnplanned => "Prefer scheduling optional visits",
            MandatoryUnplanned => "Require scheduling mandatory visits",
            Profit => "Maximize profit",
            PinStart => "Keep a pinned visit's start service time",
            PinShift => "Keep a pinned visit's vehicle shift",
            PinStartDelay => "Minimize a pinned visit's start service time allowed delay",
            Disruption => "Minimize disruption on replanning",
            MaxTravelPerVisit => "Max travel time per visit (hard)",
            Waiting => "Minimize waiting time",
            Colocated => "Prefer grouping co-located visits",
            Consecutive => "Prefer grouping visits in the same consecutive group",
            TowardsEnd => "Prefer moving towards end location",
            ReachableVisit => "Require reachable visits",
            SlaEnd => "Latest SLA end time",
            OptionalSla => "Prefer scheduling optional visits with tighter SLA",
            SlaEarlier => "Prefer scheduling visits with tight SLAs earlier",
            MandatorySla => "Require scheduling mandatory visits with tighter SLA",
            UnnecessarySkills => "Minimize scheduling vehicles with unnecessary skill levels",
            SkillsMissing => "Require skills",
            OutsidePreferredArea => "Minimize scheduling visits outside the preferred area",
            OutsideArea => "No visits outside coverage area",
            Rating => "Maximize technician rating",
            PreferredWindow => "Preferred visit time window",
            WindowEnd => "Require service max end time",
            WindowStart => "Require service max start time",
            PreferredVehicle => "Prefer visit vehicle match preferred vehicles",
            TagsMissing => "Require tags",
            NotRequiredVehicle => "Require visit vehicle match required vehicles",
            ProhibitedVehicle => "Require visit vehicle not match prohibited vehicles",
            PartsMissing => "Require parts in stock",
            Exclusion => "Require no mutually exclusive visits assigned",
            DependentUnplanned => "Require visit dependency dependent assigned",
            Fairness => "Balance time utilization",
            MovableBalance => "Balance movable and non-movable visits",
            Loops => "No loops allowed",
            SemiAssigned => "No semi-assigned visit groups",
            MaxServiceHard => "Max shift service duration (hard)",
            MaxServiceSoft => "Max shift service duration (soft)",
            MaxTravelHard => "Max shift travel time (hard)",
            MaxTravelSoft => "Max shift travel time (soft)",
            MaxVisitsHard => "Max visits per shift (hard)",
            MaxVisitsSoft => "Max visits per shift (soft)",
            TravelDistance => "Minimize travel distance",
            TravelTime => "Minimize travel time",
            InactiveDistance => "Penalize inactive shift travel distance",
            InactiveTime => "Penalize inactive shift travel time",
            ReachableItinerary => "Require reachable itineraries",
            Overnight => "Avoid overnight stay in local area",
            ExcludesShifts => "Excludes shifts",
            LastDepartureHard => "Max last visit departure time (hard)",
            LastDepartureSoft => "Max last visit departure time (soft)",
            EndHard => "Max shift end time (hard)",
            EndSoft => "Max shift end time (soft)",
            ShiftCost => "Minimize shift costs",
        }
    }

    pub const fn level(self) -> Level {
        use Rule::*;
        match self {
            MandatoryUnplanned | MandatorySla | Overnight => Level::Medium,
            DependencyDelay
            | DependencySameVehicle
            | DependencySameShift
            | DependencyPrerequisite
            | DependencyPooling
            | FixedBreakDelayed
            | FloatingBreakLate
            | FixedBreakConflict
            | LocatedBreakConflict
            | PinStart
            | PinShift
            | MaxTravelPerVisit
            | ReachableVisit
            | SkillsMissing
            | OutsideArea
            | WindowEnd
            | WindowStart
            | TagsMissing
            | NotRequiredVehicle
            | ProhibitedVehicle
            | PartsMissing
            | Exclusion
            | DependentUnplanned
            | MovableBalance
            | Loops
            | SemiAssigned
            | MaxServiceHard
            | MaxTravelHard
            | MaxVisitsHard
            | ReachableItinerary
            | ExcludesShifts
            | LastDepartureHard
            | EndHard => Level::Hard,
            _ => Level::Soft,
        }
    }

    /// The rule adds to the score rather than taking from it.
    pub const fn rewards(self) -> bool {
        use Rule::*;
        matches!(
            self,
            Profit | Colocated | Consecutive | Rating | PreferredVehicle
        )
    }

    /// Off unless a dataset or profile weighs it.
    pub const fn off_by_default(self) -> bool {
        use Rule::*;
        matches!(
            self,
            Disruption
                | Waiting
                | Colocated
                | Consecutive
                | TowardsEnd
                | OptionalSla
                | MandatorySla
                | Rating
                | TravelDistance
                | InactiveDistance
        )
    }

    pub const fn group(self) -> &'static str {
        use Rule::*;
        match self {
            DependencyDelay
            | DependencySameVehicle
            | DependencySameShift
            | DependencyPrerequisite
            | DependencyPooling
            | Exclusion
            | DependentUnplanned => DEPENDENCIES,
            Fairness => FAIRNESS,
            FixedBreakDelayed | FloatingBreakLate | FixedBreakConflict | LocatedBreakConflict => {
                BREAKS
            }
            EarliestDay | MovableBalance => MULTI_DAY,
            Loops | SemiAssigned => MULTI_VEHICLE,
            CompletionRisk | OptionalUnplanned | MandatoryUnplanned => PRIORITY,
            Profit => PROFIT,
            PinStart | PinShift | PinStartDelay | Disruption => REPLANNING,
            MaxTravelPerVisit | Waiting | Colocated | Consecutive | TowardsEnd | ReachableVisit
            | MaxServiceHard | MaxServiceSoft | MaxTravelHard | MaxTravelSoft | MaxVisitsHard
            | MaxVisitsSoft | TravelDistance | TravelTime | InactiveDistance | InactiveTime
            | ReachableItinerary => ROUTES,
            SlaEnd | OptionalSla | SlaEarlier | MandatorySla => SLA,
            Overnight | ExcludesShifts | LastDepartureHard | LastDepartureSoft | EndHard
            | EndSoft => HOURS,
            UnnecessarySkills | SkillsMissing => SKILLS,
            ShiftCost => COSTS,
            OutsidePreferredArea | OutsideArea => AREAS,
            Rating => RATING,
            PreferredWindow | WindowEnd | WindowStart => WINDOWS,
            PreferredVehicle | TagsMissing | NotRequiredVehicle | ProhibitedVehicle
            | PartsMissing => REQUIREMENTS,
        }
    }

    /// One sentence on what the rule costs.
    pub const fn description(self) -> &'static str {
        use Rule::*;
        match self {
            DependencyDelay => "A visit starts later than its dependency allows after the preceding visit departs; costs the seconds late.",
            DependencySameVehicle => "A visit is served by another vehicle than the visit it depends on, which must share it; costs its service seconds plus one hour.",
            DependencySameShift => "A visit is served on another shift than the visit it depends on, which must share it; costs its service seconds plus one hour.",
            DependencyPrerequisite => "A visit is planned while a visit it depends on is not; costs its service seconds plus one hour.",
            DependencyPooling => "The visit served right before a dependent visit is one its dependency's pooling type rules out; costs one hour.",
            FixedBreakDelayed => "The technician reaches a fixed break at a place of its own after it starts; costs the seconds late.",
            FloatingBreakLate => "A floating break ends after its latest end; costs the seconds late.",
            FixedBreakConflict => "Work runs into a fixed break, which starts late; costs the seconds of overlap.",
            LocatedBreakConflict => "Work runs into a fixed break at a place of its own; costs the seconds of overlap.",
            EarliestDay => "A movable visit is served after the first day a technician who may serve it works; costs 1000 per day later.",
            CompletionRisk => "A visit of the risk priority or higher ends within the risk margin of its shift's end; costs the seconds inside the margin.",
            OptionalUnplanned => "An optional visit is unplanned; costs its priority weight.",
            MandatoryUnplanned => "A mandatory visit is unplanned; costs its priority weight.",
            Profit => "A planned visit earns its profit; rewards the cents.",
            PinStart => "A pinned visit starts later than its kept start plus the allowed delay; costs 100 000 per second.",
            PinShift => "A pinned visit is not on the shift it is pinned to; costs 10 000 000 000.",
            PinStartDelay => "A pinned visit starts after its kept start, within the allowed delay; costs the seconds.",
            Disruption => "A replan moves a visit from the shift (100) or the vehicle (100) the input had it on, or shifts its start; costs those plus the seconds moved. Off unless weighted.",
            MaxTravelPerVisit => "The drive to a visit takes longer than its shift allows per visit; costs the seconds over.",
            Waiting => "A technician waits before starting a visit; costs the seconds. Off unless weighted.",
            Colocated => "Two consecutive items of a route are at the same place (within the co-location radius); rewards one per pair. Off unless weighted.",
            Consecutive => "Two consecutive visits of a route share a preferred consecutive group; rewards one per pair. Off unless weighted.",
            TowardsEnd => "A drive after the first visit takes the technician farther from the shift's end location; costs the meters farther. Off unless weighted.",
            ReachableVisit => "No road leads to a visit from the place before it; costs one hour per visit.",
            SlaEnd => "A visit ends after its latest SLA end; costs the seconds late.",
            OptionalSla => "An optional visit with an SLA is unplanned; costs more the tighter its SLA. Off unless weighted.",
            SlaEarlier => "A visit with an SLA ends late within the time its SLA leaves; costs the share of that time used, in thousandths.",
            MandatorySla => "A mandatory visit with an SLA is unplanned; costs more the tighter its SLA. Off unless weighted.",
            UnnecessarySkills => "The technician holds skills above the level a visit needs; costs its service seconds times the levels above.",
            SkillsMissing => "The technician lacks a skill a visit needs, or holds it below the level; costs one hour per skill and level short.",
            OutsidePreferredArea => "A visit lies outside the technician's preferred area; costs one per visit.",
            OutsideArea => "A visit lies outside the technician's coverage area; costs one hour per visit.",
            Rating => "A visit is served by a rated technician; rewards the rating times the service seconds. Off unless weighted.",
            PreferredWindow => "A visit's service falls outside its preferred time windows; costs the seconds outside.",
            WindowEnd => "A visit's service ends after its window's latest end; costs the seconds late.",
            WindowStart => "A visit's service starts after its window's latest start; costs the seconds late.",
            PreferredVehicle => "A visit is served by a vehicle it prefers; rewards the preference weight times the service seconds.",
            TagsMissing => "The shift serving a visit lacks tags it needs; costs one hour per tag.",
            NotRequiredVehicle => "A visit is served by a vehicle it does not list among its required vehicles; costs one hour.",
            ProhibitedVehicle => "A visit is served by a vehicle it prohibits; costs one hour.",
            PartsMissing => "A visit needs parts its shift's stock no longer holds when it is served, its earlier visits having used them; costs one hour per unit short.",
            Exclusion => "Two visits that exclude each other are both planned; costs one hour per pair.",
            DependentUnplanned => "A visit is planned while a visit that depends on it is not; costs the dependent's service seconds plus one hour.",
            Fairness => "Technicians' utilization (worked over available time, history included) differs; costs the squares of each technician's utilization in ten-thousandths, divided by ten thousand.",
            MovableBalance => "Movable visits take more of a shift's working time than its threshold allows; costs the seconds over.",
            Loops => "A visit cannot be timed: it waits on a visit planned after it, directly or through a joint visit; costs its service seconds plus one hour.",
            SemiAssigned => "Some visits of a multi-vehicle group are planned and others are not; costs the service seconds of each unplanned one plus one hour.",
            MaxServiceHard => "A shift's service exceeds its hard maximum; costs the seconds over.",
            MaxServiceSoft => "A shift's service exceeds its preferred maximum; costs the seconds over.",
            MaxTravelHard => "A shift's travel exceeds its hard maximum; costs the seconds over.",
            MaxTravelSoft => "A shift's travel exceeds its preferred maximum; costs the seconds over.",
            MaxVisitsHard => "A shift serves more visits than its hard maximum; costs one hour per visit over.",
            MaxVisitsSoft => "A shift serves more visits than its preferred maximum; costs one per visit over.",
            TravelDistance => "Driving distance of a shift that serves visits, start to end; costs the meters. Off unless weighted.",
            TravelTime => "Driving time of a shift that serves visits, start to end; costs the seconds.",
            InactiveDistance => "Driving distance of a shift that serves no visit, to its breaks and home; costs the meters. Off unless weighted.",
            InactiveTime => "Driving time of a shift that serves no visit, to its breaks and home; costs the seconds.",
            ReachableItinerary => "A shift's route has a leg no road connects, start to end; costs one hour per leg.",
            Overnight => "A linked shift keeps the technician out overnight between two visits both within reach of home; costs ten times the highest mandatory priority weight.",
            ExcludesShifts => "A linked shift and one of its component shifts both serve visits; costs one hour per visit on the component.",
            LastDepartureHard => "A shift leaves its last visit after the latest departure; costs the seconds late.",
            LastDepartureSoft => "A shift leaves its last visit after the preferred departure; costs the seconds late.",
            EndHard => "A shift reaches its end location after its latest end; costs the seconds late.",
            EndSoft => "A shift reaches its end location after its preferred end; costs the seconds of overtime.",
            ShiftCost => "A shift that serves visits costs its fixed cost and its rates over its working time; costs the cents.",
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
    fn rules_are_listed_in_index_order_with_unique_names() {
        for (index, rule) in RULES.into_iter().enumerate() {
            assert_eq!(rule.index(), index);
            assert_eq!(RULES.iter().filter(|r| r.name() == rule.name()).count(), 1);
        }
        assert_eq!(VISIT_RULES, 41);
    }
}
