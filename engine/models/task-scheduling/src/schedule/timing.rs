//! Timing: tasks start as early as their resource, their changeover, their
//! earliest start and their dependencies allow.
//!
//! Each resource runs its frozen tasks, then its planned ones, in order. A
//! task waits for the task before it on its resource and the changeover
//! between them, for its earliest start, and for every assigned task it
//! depends on (its anchor plus the minimum offset). Work pauses over
//! unavailable spans, or waits for a long enough gap when the task must not
//! be interrupted. A pinned task keeps the start it was pinned at.
//!
//! Tasks are timed in dependency order across resources. When dependencies
//! and resource orders form a cycle no timing satisfies, the earliest such
//! task on the lowest resource is timed without its untimed dependencies;
//! the dependency rule then scores the broken offsets.

use crate::domain::Plan;

/// Marks a task no machine or employee runs.
pub const UNASSIGNED: usize = usize::MAX;

/// The setup before a task on a machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Changeover {
    pub start: i64,
    pub end: i64,
    /// Setup minutes from the changeover table (0 when only the type changes).
    pub minutes: i64,
    /// Task type before the changeover: the previous task's or the machine's
    /// initial one.
    pub from_type: usize,
}

/// The timing of every task of a plan. Reused across evaluations so the
/// solver's hot path does not allocate.
#[derive(Debug, Clone, Default)]
pub struct Timing {
    /// Resource index per task, or [`UNASSIGNED`].
    pub resource: Vec<usize>,
    /// Position on its resource, frozen tasks first.
    pub position: Vec<usize>,
    pub start: Vec<i64>,
    pub end: Vec<i64>,
    pub changeover: Vec<Option<Changeover>>,
    /// Machine idle minutes right before the task.
    pub idle: Vec<i64>,
    /// Per resource: its tasks in processing order.
    pub order: Vec<Vec<usize>>,
    /// Tasks in the unscheduled queue.
    pub queued: Vec<usize>,
    // Scratch.
    pending: Vec<u32>,
    ready: Vec<usize>,
    next: Vec<usize>,
    done: Vec<bool>,
}

impl Timing {
    /// The timing of `plan`.
    pub fn of(plan: &Plan) -> Self {
        let mut timing = Self::default();
        timing.compute(plan);
        timing
    }

    /// Whether task `t` runs on a machine or an employee.
    pub fn assigned(&self, t: usize) -> bool {
        self.resource.get(t).is_some_and(|&r| r != UNASSIGNED)
    }

    /// Recomputes the timing of `plan` in place.
    pub fn compute(&mut self, plan: &Plan) {
        let facts = &*plan.facts;
        let n = plan.tasks.len();
        let resources = facts.resources.len();
        self.resource.clear();
        self.resource.resize(n, UNASSIGNED);
        self.position.clear();
        self.position.resize(n, 0);
        self.start.clear();
        self.start.resize(n, 0);
        self.end.clear();
        self.end.resize(n, 0);
        self.changeover.clear();
        self.changeover.resize(n, None);
        self.idle.clear();
        self.idle.resize(n, 0);
        self.order.resize_with(resources, Vec::new);
        self.order.truncate(resources);
        self.queued.clear();

        // Place tasks; a task listed twice (a move in flight) counts once.
        for (r, resource) in plan.resources.iter().enumerate() {
            if resource.unscheduled || r >= resources {
                for &t in &resource.sequence {
                    if t < n && self.resource[t] == UNASSIGNED {
                        self.queued.push(t);
                    }
                }
                continue;
            }
            let order = &mut self.order[r];
            order.clear();
            for &t in resource.frozen.iter().chain(&resource.sequence) {
                if t < n && self.resource[t] == UNASSIGNED {
                    self.resource[t] = r;
                    self.position[t] = order.len();
                    order.push(t);
                }
            }
        }
        // A queued task that also sits on a resource is on the resource.
        let resource = &self.resource;
        self.queued.retain(|&t| resource[t] == UNASSIGNED);

        // Dependency-ordered timing (Kahn), resource order as edges too.
        self.pending.clear();
        self.pending.resize(n, 0);
        let mut assigned = 0;
        for t in 0..n {
            if self.resource[t] == UNASSIGNED {
                continue;
            }
            assigned += 1;
            let mut count = u32::from(self.position[t] > 0);
            count += facts.tasks[t]
                .deps
                .iter()
                .filter(|dep| self.resource[dep.task] != UNASSIGNED)
                .count() as u32;
            self.pending[t] = count;
        }
        self.ready.clear();
        for t in 0..n {
            if self.resource[t] != UNASSIGNED && self.pending[t] == 0 {
                self.ready.push(t);
            }
        }
        self.next.clear();
        self.next.resize(resources, 0);
        let mut timed = 0;
        let mut done = std::mem::take(&mut self.done);
        done.clear();
        done.resize(n, false);
        while timed < assigned {
            let t = match self.ready.pop() {
                Some(t) => t,
                None => {
                    // A cycle: time the first untimed task of the lowest
                    // resource that has one.
                    let Some(r) = (0..resources).find(|&r| self.next[r] < self.order[r].len())
                    else {
                        break;
                    };
                    self.order[r][self.next[r]]
                }
            };
            if done[t] {
                continue;
            }
            self.time(plan, t, &done);
            done[t] = true;
            timed += 1;
            let r = self.resource[t];
            self.next[r] = self.position[t] + 1;
            if let Some(&after) = self.order[r].get(self.position[t] + 1) {
                self.release(after);
            }
            for &(dependent, _) in &facts.dependents[t] {
                if self.resource[dependent] != UNASSIGNED && !done[dependent] {
                    self.release(dependent);
                }
            }
        }
        self.done = done;
    }

    fn release(&mut self, t: usize) {
        if self.pending[t] > 0 {
            self.pending[t] -= 1;
            if self.pending[t] == 0 {
                self.ready.push(t);
            }
        }
    }

    /// Times task `t`; every task it waits for that is timed is in `done`.
    fn time(&mut self, plan: &Plan, t: usize, done: &[bool]) {
        let facts = &*plan.facts;
        let r = self.resource[t];
        let rf = &facts.resources[r];
        let tf = &facts.tasks[t];
        let p = self.position[t];
        let (ready, prev_type) = if p == 0 {
            (rf.start, rf.initial_type)
        } else {
            let prev = self.order[r][p - 1];
            (self.end[prev], facts.tasks[prev].task_type)
        };
        let mut setup_minutes = 0;
        let mut earliest = ready;
        if let (true, Some(machine_type), Some(from), Some(to)) =
            (rf.machine, rf.machine_type, prev_type, tf.task_type)
        {
            let minutes = facts.changeover(machine_type, from, to).unwrap_or(0);
            if from != to || minutes > 0 {
                let end = rf.calendar.finish(ready, minutes);
                self.changeover[t] = Some(Changeover {
                    start: ready,
                    end,
                    minutes,
                    from_type: from,
                });
                setup_minutes = minutes;
                earliest = end;
            }
        }
        if let Some(min_start) = tf.min_start {
            earliest = earliest.max(min_start);
        }
        for dep in &tf.deps {
            if self.resource[dep.task] == UNASSIGNED || !done[dep.task] {
                continue;
            }
            let anchor = if dep.from_end {
                self.end[dep.task]
            } else {
                self.start[dep.task]
            };
            earliest = earliest.max(anchor + dep.min_offset);
        }
        let minutes = tf.duration[r];
        let (start, end) = match plan.tasks[t].pinned_start {
            Some(start) if tf.interruptible => (start, rf.calendar.finish(start, minutes)),
            Some(start) => (start, start + minutes),
            None if tf.interruptible => {
                let start = rf.calendar.available_from(earliest);
                (start, rf.calendar.finish(start, minutes))
            }
            None => {
                let start = rf.calendar.uninterrupted_from(earliest, minutes);
                (start, start + minutes)
            }
        };
        self.start[t] = start;
        self.end[t] = end;
        if rf.machine {
            let waited = start - ready - rf.calendar.unavailable_between(ready, start);
            self.idle[t] = (waited - setup_minutes).max(0);
        }
    }
}
