//! Facts derived from the dataset once, by `prepare`, and shared by every
//! clone of the plan: places and travel, what each stop and break needs,
//! and what each shift offers.

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
    /// Day (since 1970) of the earliest window start, for stops whose
    /// windows span more than one day.
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
