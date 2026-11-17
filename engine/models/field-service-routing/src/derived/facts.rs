//! Facts derived from the dataset once, by `prepare`, and shared by every
//! clone of the plan: places and travel, what each visit and break needs,
//! what each shift offers, and how each visit fits each shift.

/// A visit or a break.
#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Visit,
    Break(BreakFact),
}

/// A break of one shift.
#[derive(Debug, Clone, PartialEq)]
pub struct BreakFact {
    pub shift: usize,
    pub fixed: bool,
    /// Fixed: the start. Floating: the earliest start.
    pub start: i64,
    /// Fixed: the end. Floating: the latest end, if any.
    pub end: Option<i64>,
    /// Seconds.
    pub duration: i64,
}

/// One time window, in seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub min_start: i64,
    pub max_start: Option<i64>,
    pub max_end: i64,
}

/// When, after a preceding visit's departure, a visit may start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delay {
    /// Seconds after the departure.
    After(i64),
    /// `days` days (working days when `working`) after the departure's day,
    /// at `time` seconds after midnight.
    At { days: i64, working: bool, time: i64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coordination {
    None,
    SameVehicle,
    SameShift,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pooling {
    All,
    Visits,
    Dependencies,
    None,
}

/// A visit's wait on a preceding visit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dependency {
    pub preceding: usize,
    pub min: Delay,
    pub max: Option<Delay>,
    pub coordination: Coordination,
    pub pooling: Pooling,
}

/// What a visit or break needs, independent of where it is planned.
#[derive(Debug, Clone, PartialEq)]
pub struct VisitFact {
    /// The wire id.
    pub id: String,
    pub label: String,
    pub kind: Kind,
    /// Index into [`Travel`]'s places; breaks without a place have none.
    pub location: Option<usize>,
    /// Service seconds before multipliers and adjustments.
    pub service: i64,
    /// Seconds of the service per skill.
    pub parts: Vec<(usize, i64)>,
    /// Seconds added to (or taken off) the service.
    pub adjustment: i64,
    /// Seconds added when the visit before it was elsewhere.
    pub first_on_location: i64,
    pub windows: Vec<Window>,
    pub preferred_windows: Vec<Window>,
    /// Required skill and minimum level.
    pub skills: Vec<(usize, Option<i64>)>,
    pub tags: Vec<usize>,
    /// Priority weight, and whether leaving it unplanned costs medium.
    pub weight: i64,
    pub mandatory: bool,
    /// Its priority is at least as important as the completion-risk one.
    pub at_risk: bool,
    pub sla: Option<i64>,
    pub consecutive: Option<usize>,
    pub exclusions: Vec<usize>,
    /// Earned when planned, in cents.
    pub profit: i64,
    pub min_start_travel: Option<i64>,
    /// The shift this element must stay on: a break's own shift.
    pub owner: Option<usize>,
    /// A pinned visit's shift and the start it keeps.
    pub pin: Option<(usize, i64)>,
    /// Where and when the input plan had it, for disruption; `i64::MIN`
    /// when it gave no time.
    pub original: Option<(usize, i64)>,
    /// Its windows span days or reach past the planning window.
    pub movable: bool,
    /// Only a linked shift reaches it: every single shift is too short for
    /// the round trip.
    pub remote: bool,
    /// Its same-shift dependency chain.
    pub chain: Option<usize>,
    /// Its place in list construction's order.
    pub order: i64,
    /// Day (since 1970) of its earliest window start, for movable visits.
    pub earliest_day: Option<i64>,
    pub dependencies: Vec<Dependency>,
    /// It is the preceding visit of some dependency.
    pub has_dependents: bool,
    /// The visit group it belongs to.
    pub group: Option<usize>,
}

impl VisitFact {
    pub fn is_break(&self) -> bool {
        matches!(self.kind, Kind::Break(_))
    }

    /// Takes part in a dependency, either side.
    pub fn in_dependency(&self) -> bool {
        self.has_dependents || !self.dependencies.is_empty()
    }
}

/// Visits several vehicles serve together.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupFact {
    pub id: String,
    /// Members end together rather than start together.
    pub align_end: bool,
    pub visits: Vec<usize>,
}

/// A skill a shift holds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Held {
    pub skill: usize,
    pub level: i64,
    pub multiplier: Option<f64>,
}

/// What one shift costs once it serves a visit.
#[derive(Debug, Clone, PartialEq)]
pub struct CostFact {
    /// Cents.
    pub fixed: i64,
    /// Seconds the rate lasts (`None` to the end), activation cents, cents
    /// per second.
    pub rates: Vec<(Option<i64>, i64, f64)>,
}

impl CostFact {
    /// Cents a shift working `seconds` costs.
    pub fn cost(&self, seconds: i64) -> i64 {
        let mut total = self.fixed as f64;
        let mut from = 0;
        for &(length, activation, per_second) in &self.rates {
            if seconds <= from && from > 0 {
                break;
            }
            total += activation as f64;
            let until = length.map_or(seconds, |l| (from + l).min(seconds));
            total += per_second * (until - from).max(0) as f64;
            match length {
                Some(l) => from += l,
                None => break,
            }
        }
        total.round() as i64
    }
}

/// What a shift offers.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShiftFact {
    pub id: String,
    pub label: String,
    pub vehicle: usize,
    /// A linked shift: its component shifts, day by day.
    pub components: Vec<usize>,
    /// A component shift: the linked shifts it is part of.
    pub linked_into: Vec<usize>,
    pub start_location: usize,
    pub end_location: usize,
    /// Working spans `[start, end)`: one per day of a linked shift, the
    /// last one open-ended. Work and travel pause between spans.
    pub spans: Vec<(i64, i64)>,
    pub min_start: i64,
    pub min_first_arrival: Option<i64>,
    pub max_end: Option<i64>,
    pub max_soft_end: Option<i64>,
    pub max_last_departure: Option<i64>,
    pub max_soft_last_departure: Option<i64>,
    /// Seconds or counts.
    pub max_travel_per_visit: Option<i64>,
    pub max_travel: Option<i64>,
    pub max_soft_travel: Option<i64>,
    pub max_service: Option<i64>,
    pub max_soft_service: Option<i64>,
    pub max_visits: Option<i64>,
    pub max_soft_visits: Option<i64>,
    pub skills: Vec<Held>,
    /// `[start, end)` and the skills held then.
    pub temporary_skills: Vec<(i64, i64, Vec<Held>)>,
    pub tags: Vec<usize>,
    pub temporary_tags: Vec<(i64, i64, Vec<usize>)>,
    pub movable_ratio: f64,
    /// Seconds from the start limit to the end limit, less breaks.
    pub working_time: i64,
    pub cost: Option<CostFact>,
    /// Drive seconds as `multiplier * plain + extra`.
    pub travel_multiplier: f64,
    pub travel_extra: i64,
    pub colocated_radius: i64,
    /// The holding list of visits no shift serves.
    pub bench: bool,
}

impl ShiftFact {
    pub fn linked(&self) -> bool {
        !self.components.is_empty()
    }

    /// The working span holding moment `t`, or the next one after it.
    pub fn span(&self, t: i64) -> usize {
        self.spans
            .iter()
            .position(|&(_, end)| t < end)
            .unwrap_or(self.spans.len() - 1)
    }

    /// The moment `seconds` of work after `t` ends, pausing between
    /// working spans.
    pub fn advance(&self, t: i64, seconds: i64) -> i64 {
        let mut t = t;
        let mut left = seconds;
        let mut span = self.span(t);
        loop {
            let (start, end) = self.spans[span];
            t = t.max(start);
            if end == i64::MAX || t + left <= end {
                return t + left;
            }
            left -= end - t;
            span += 1;
        }
    }

    /// The earliest moment at or after `t` inside a working span.
    pub fn at_work(&self, t: i64) -> i64 {
        t.max(self.spans[self.span(t)].0)
    }

    /// Drive seconds after the shift's travel adjustment.
    pub fn drive(&self, plain: i64) -> i64 {
        (plain as f64 * self.travel_multiplier).round() as i64 + self.travel_extra
    }
}

/// A technician and their vehicle.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VehicleFact {
    pub id: String,
    pub label: String,
    pub rating: f64,
    /// Seconds worked and available before this plan.
    pub historical_utilized: i64,
    pub historical_capacity: i64,
    /// Seconds of working time over its shifts in this plan.
    pub capacity: i64,
}

/// How one visit fits one shift, apart from the time it is served.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Fit {
    pub not_required: bool,
    pub prohibited: bool,
    /// Weight of a preferred-vehicle match.
    pub preferred: i64,
    pub outside_required: bool,
    pub outside_preferred: bool,
    /// The round trip from the shift's start to the visit and to its end
    /// takes longer than the shift's working time.
    pub far: bool,
}

/// Travel between places: seconds and meters, `None` where no road goes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Travel {
    pub places: Vec<[f64; 2]>,
    legs: Vec<Option<(i64, i64)>>,
}

impl Travel {
    pub fn new(places: Vec<[f64; 2]>, legs: Vec<Option<(i64, i64)>>) -> Self {
        Self { places, legs }
    }

    /// Seconds and meters from `from` to `to`.
    pub fn leg(&self, from: usize, to: usize) -> Option<(i64, i64)> {
        self.legs[from * self.places.len() + to]
    }
}

/// Straight-line meters between two `[latitude, longitude]` points.
pub fn meters(a: [f64; 2], b: [f64; 2]) -> f64 {
    const EARTH: f64 = 6_371_000.0;
    let (la1, la2) = (a[0].to_radians(), b[0].to_radians());
    let dla = la2 - la1;
    let dlo = (b[1] - a[1]).to_radians();
    let h = (dla / 2.0).sin().powi(2) + la1.cos() * la2.cos() * (dlo / 2.0).sin().powi(2);
    2.0 * EARTH * h.sqrt().asin()
}

/// Every fact the constraints and route timing read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Facts {
    /// The planning window, `[start, end)` in seconds.
    pub window: (i64, i64),
    pub travel: Travel,
    pub skills: Vec<String>,
    pub tags: Vec<String>,
    pub vehicles: Vec<VehicleFact>,
    pub visits: Vec<VisitFact>,
    /// Visits come first: elements from here on are breaks.
    pub first_break: usize,
    pub groups: Vec<GroupFact>,
    /// Visits that depend on each other on the same shift, in service
    /// order: SolverForge keeps each chain on one shift and moves, ruins
    /// and places it as one unit.
    pub chains: Vec<Vec<usize>>,
    pub consecutive_groups: Vec<String>,
    pub shifts: Vec<ShiftFact>,
    /// `fits[visit * shifts + shift]`, for visits only.
    pub fits: Vec<Fit>,
    /// Ten times the highest mandatory priority weight: the medium cost of
    /// an overnight stay a technician could have spent at home.
    pub overnight_cost: i64,
    /// Seconds a pinned visit may start late at a soft cost.
    pub pin_allowed_delay: i64,
    /// Seconds before a shift's end within which a visit risks completion.
    pub risk_margin: i64,
    /// Days (since 1970) that are no working day besides weekends.
    pub holidays: Vec<i64>,
}

impl Facts {
    pub fn fit(&self, visit: usize, shift: usize) -> &Fit {
        &self.fits[visit * self.shifts.len() + shift]
    }

    /// The shift holding visits no shift serves.
    pub fn bench(&self) -> usize {
        self.shifts.len() - 1
    }

    pub fn is_visit(&self, element: usize) -> bool {
        element < self.first_break
    }

    /// The earliest moment a visit waiting `delay` on a departure at
    /// `departure` may start.
    pub fn after(&self, departure: i64, delay: Delay) -> i64 {
        match delay {
            Delay::After(seconds) => departure + seconds,
            Delay::At {
                days,
                working,
                time,
            } => {
                let mut day = departure.div_euclid(crate::time::DAY);
                let mut left = days;
                while left > 0 {
                    day += 1;
                    if !working || self.working_day(day) {
                        left -= 1;
                    }
                }
                day * crate::time::DAY + time
            }
        }
    }

    fn working_day(&self, day: i64) -> bool {
        crate::time::weekday(day) < 5 && !self.holidays.contains(&day)
    }
}
