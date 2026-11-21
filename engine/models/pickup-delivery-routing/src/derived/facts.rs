//! Facts derived from the dataset once, by `prepare`, and shared by every
//! clone of the plan: places and travel, what each stop and break needs,
//! and what each shift offers.

use solverforge_workbench_host::traffic::{travel_seconds, BusyWindow};

/// A job's stop or a driver's break.
#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// Stop `seq` (0-based) of job `job`.
    Stop {
        job: usize,
        seq: usize,
    },
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
    /// No job may be on board during the break.
    pub reject_jobs: bool,
}

/// One time window of a stop, in seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub min_start: i64,
    pub max_start: Option<i64>,
    pub max_end: i64,
}

/// What a stop or break needs, independent of where it is planned.
#[derive(Debug, Clone, PartialEq)]
pub struct VisitFact {
    /// The wire id.
    pub id: String,
    pub label: String,
    pub kind: Kind,
    /// Index into [`Travel`]'s places; breaks without a place have none.
    pub location: Option<usize>,
    /// Service seconds before any skill multiplier.
    pub duration: i64,
    pub windows: Vec<Window>,
    /// Units picked up minus units delivered, per capacity type.
    pub load: Vec<i64>,
    pub sla: Option<i64>,
    pub min_start_travel: Option<i64>,
    /// Day (since 1970) a shift that may serve it first could, for stops
    /// whose windows span more than one day.
    pub earliest_day: Option<i64>,
    /// The shift this element must stay on: a break's own shift.
    pub owner: Option<usize>,
    /// A pinned stop's shift and the start of service it keeps.
    pub pin: Option<(usize, i64)>,
}

/// What a job asks of the shift that serves it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct JobFact {
    pub id: String,
    pub label: String,
    /// Visit indexes, in service order.
    pub stops: Vec<usize>,
    pub prohibited_jobs: Vec<usize>,
    pub no_pooling: bool,
    pub interruptible: bool,
    /// Seconds pooling may add; `None` is no limit.
    pub max_burden: Option<i64>,
    pub mandatory: bool,
    pub weight: i64,
}

/// What a shift offers.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShiftFact {
    pub id: String,
    pub label: String,
    pub driver: usize,
    pub start_location: usize,
    pub end_location: usize,
    pub min_start: i64,
    pub min_first_arrival: Option<i64>,
    pub max_end: Option<i64>,
    pub max_soft_end: Option<i64>,
    pub max_last_departure: Option<i64>,
    pub max_soft_last_departure: Option<i64>,
    pub capacity: Vec<i64>,
    /// The holding list of stops no shift serves.
    pub bench: bool,
}

/// How well one shift fits one job: the job's per-stop costs on it.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Fit {
    pub not_required: bool,
    pub prohibited: bool,
    pub not_preferred: bool,
    pub skills_missing: i64,
    pub tags_missing: i64,
    /// Skill levels above the minimum, summed over the job's skills.
    pub overqualified: i64,
    /// Service duration multiplier from the shift's matching skills.
    pub multiplier: f64,
}

/// Seconds and meters per ordered pair of places, `None` where no road goes.
pub type Legs = Vec<Option<(i64, i64)>>;

/// Travel between places: seconds and meters, `None` where no road goes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Travel {
    pub places: Vec<[f64; 2]>,
    legs: Vec<Option<(i64, i64)>>,
    /// Per busy period of the plan, its legs; `None` when it travels as usual.
    periods: Vec<Option<Legs>>,
    /// When each busy period runs, in time order.
    windows: Vec<BusyWindow>,
    /// Legs of each mode other than the car's, in the plan's order.
    modes: Vec<Legs>,
    /// Per vehicle, which travel it uses: 0 the car's, `m` the `m`-th mode's.
    vehicle_modes: Vec<usize>,
}

impl Travel {
    pub fn new(places: Vec<[f64; 2]>, legs: Vec<Option<(i64, i64)>>) -> Self {
        Self {
            places,
            legs,
            periods: Vec::new(),
            windows: Vec::new(),
            modes: Vec::new(),
            vehicle_modes: Vec::new(),
        }
    }

    /// The same travel, slower or faster while busy periods run.
    pub fn with_traffic(mut self, periods: Vec<Option<Legs>>, windows: Vec<BusyWindow>) -> Self {
        self.periods = periods;
        self.windows = windows;
        self
    }

    /// Seconds and meters from `from` to `to` at usual speeds.
    pub fn leg(&self, from: usize, to: usize) -> Option<(i64, i64)> {
        self.legs[from * self.places.len() + to]
    }

    /// The same travel, with the vehicles that go by bicycle or on foot
    /// travelling on their mode's legs: `vehicle_modes[v]` is 0 for a car,
    /// `m` for the `m`-th of `modes`.
    pub fn with_modes(mut self, modes: Vec<Legs>, vehicle_modes: Vec<usize>) -> Self {
        self.modes = modes;
        self.vehicle_modes = vehicle_modes;
        self
    }

    /// Which travel `vehicle` uses: 0 the car's, `m` the `m`-th mode's.
    fn mode(&self, vehicle: usize) -> usize {
        self.vehicle_modes.get(vehicle).copied().unwrap_or(0)
    }

    /// Seconds and meters from `from` to `to` for `vehicle` at usual speeds.
    pub fn leg_of(&self, vehicle: usize, from: usize, to: usize) -> Option<(i64, i64)> {
        match self.mode(vehicle) {
            0 => self.leg(from, to),
            m => self.modes[m - 1][from * self.places.len() + to],
        }
    }

    /// Seconds and meters from `from` to `to` for `vehicle` leaving at
    /// `depart`: a car drives through the busy periods it crosses; a bicycle
    /// or a walker is not slowed by them.
    pub fn trip(&self, vehicle: usize, from: usize, to: usize, depart: i64) -> Option<(i64, i64)> {
        match self.mode(vehicle) {
            0 => self.leg_at(from, to, depart),
            _ => self.leg_of(vehicle, from, to),
        }
    }

    /// Seconds and meters from `from` to `to` leaving at `depart`: driven
    /// through the busy periods it crosses, over the route of the moment it leaves.
    pub fn leg_at(&self, from: usize, to: usize, depart: i64) -> Option<(i64, i64)> {
        let (seconds, meters) = self.leg(from, to)?;
        if self.windows.is_empty() {
            return Some((seconds, meters));
        }
        let cell = from * self.places.len() + to;
        let during = |period: usize| {
            self.periods
                .get(period)
                .and_then(|p| p.as_ref())
                .and_then(|legs| legs[cell])
        };
        let travel = travel_seconds(depart, seconds, &self.windows, |p| {
            during(p).map_or(seconds, |l| l.0)
        });
        let leaving = self
            .windows
            .iter()
            .find(|w| w.start <= depart && depart < w.end)
            .and_then(|w| during(w.period));
        Some((travel, leaving.map_or(meters, |l| l.1)))
    }
}

/// Every fact the constraints and route timing read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Facts {
    /// The planning window, `[start, end)` in seconds.
    pub window: (i64, i64),
    pub travel: Travel,
    pub capacity_types: Vec<String>,
    pub drivers: Vec<String>,
    pub visits: Vec<VisitFact>,
    pub jobs: Vec<JobFact>,
    pub shifts: Vec<ShiftFact>,
    /// `fits[job * shifts + shift]`.
    pub fits: Vec<Fit>,
    /// Part names, in the order the dataset first names them.
    pub parts: Vec<String>,
    /// Per element, the parts it uses up: `(part, quantity)`.
    pub visit_parts: Vec<Vec<(usize, i64)>>,
    /// Per shift, the units of each part it loads.
    pub shift_stock: Vec<Vec<i64>>,
}

impl Facts {
    pub fn fit(&self, job: usize, shift: usize) -> &Fit {
        &self.fits[job * self.shifts.len() + shift]
    }

    /// The shift holding stops no shift serves.
    pub fn bench(&self) -> usize {
        self.shifts.len() - 1
    }

    /// The job and position in it of a stop.
    pub fn stop(&self, visit: usize) -> Option<(usize, usize)> {
        match self.visits[visit].kind {
            Kind::Stop { job, seq } => Some((job, seq)),
            Kind::Break(_) => None,
        }
    }
}
