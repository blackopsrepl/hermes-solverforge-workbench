//! A task's slot: when and where it runs, and what each rule costs for it.
//!
//! The slot is the task's cascading list shadow. SolverForge recomputes it
//! after every move for the tasks whose inputs changed: the task before it
//! on its resource and the tasks it depends on, wherever those run. A task
//! starts as early as its resource, its changeover, its earliest start and
//! its dependencies allow; work pauses over unavailable spans, or waits for
//! a long enough gap when the task must not be interrupted. A frozen task
//! keeps the start it was pinned at.

use crate::constraints::{Rule, RULES};
use crate::derived::{Facts, ResourceFacts, TaskFacts};
use crate::domain::Plan;
use crate::time::overlap;

/// The setup before a task on a machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Changeover {
    pub start: i64,
    pub end: i64,
    /// Setup minutes from the changeover table (0 when only the type changes).
    pub minutes: i64,
    /// Task type before the changeover: the previous task's or the machine's
    /// initial one.
    pub from_type: usize,
}

/// Where and when a task runs, and its cost under every per-task rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Slot {
    /// The machine or employee running the task; `None` in the queue.
    pub resource: Option<usize>,
    /// Position among the resource's tasks, frozen ones first.
    pub rank: usize,
    pub start: i64,
    pub end: i64,
    pub changeover: Option<Changeover>,
    /// Machine idle minutes right before the task.
    pub idle: i64,
    /// Minutes from the resource's first availability to the task's end.
    pub span: i64,
    /// Positive amount per rule, by [`Rule::index`].
    pub amounts: [i64; RULES.len()],
}

impl Slot {
    /// A task waiting in the unscheduled queue.
    fn queued(priority: i64) -> Self {
        let mut amounts = [0; RULES.len()];
        amounts[Rule::Unassigned.index()] = priority;
        Self {
            resource: None,
            rank: 0,
            start: 0,
            end: 0,
            changeover: None,
            idle: 0,
            span: 0,
            amounts,
        }
    }

    /// Runs on a machine or an employee.
    pub fn placed(&self) -> bool {
        self.resource.is_some()
    }

    pub fn amount(&self, rule: Rule) -> i64 {
        self.amounts[rule.index()]
    }
}

/// When a task can begin after `ready` on resource `rf`, the type before
/// it being `previous_type`: the changeover, if any, and the earliest start.
pub fn begin(
    facts: &Facts,
    rf: &ResourceFacts,
    tf: &TaskFacts,
    ready: i64,
    previous_type: Option<usize>,
) -> (Option<Changeover>, i64) {
    let mut earliest = ready;
    let mut changeover = None;
    if let (true, Some(machine_type), Some(from), Some(to)) =
        (rf.machine, rf.machine_type, previous_type, tf.task_type)
    {
        let minutes = facts.changeover(machine_type, from, to).unwrap_or(0);
        if from != to || minutes > 0 {
            let end = rf.calendar.finish(ready, minutes);
            changeover = Some(Changeover {
                start: ready,
                end,
                minutes,
                from_type: from,
            });
            earliest = end;
        }
    }
    if let Some(min_start) = tf.min_start {
        earliest = earliest.max(min_start);
    }
    (changeover, earliest)
}

/// Start and end of `minutes` of work on `rf` from `earliest`.
pub fn run(rf: &ResourceFacts, tf: &TaskFacts, earliest: i64, minutes: i64) -> (i64, i64) {
    if tf.interruptible {
        let start = rf.calendar.available_from(earliest);
        (start, rf.calendar.finish(start, minutes))
    } else {
        let start = rf.calendar.uninterrupted_from(earliest, minutes);
        (start, start + minutes)
    }
}

/// The end of frozen task `t`, which keeps its pinned start.
pub fn frozen_end(plan: &Plan, t: usize) -> i64 {
    let facts = &*plan.facts;
    let task = &plan.tasks[t];
    let (Some(r), Some(start)) = (task.frozen_on, task.pinned_start) else {
        return 0;
    };
    let tf = &facts.tasks[t];
    let minutes = tf.duration[r];
    if tf.interruptible {
        facts.resources[r].calendar.finish(start, minutes)
    } else {
        start + minutes
    }
}

/// The slot of task `t`: `None` while it is in no list, or when the task
/// before it on its resource has none (a dependency cycle through it).
pub fn compute(plan: &Plan, t: usize) -> Option<Slot> {
    let facts = &*plan.facts;
    let task = &plan.tasks[t];
    let lane = task.lane?;
    let tf = facts.tasks.get(t)?;
    if lane == facts.queue {
        return Some(Slot::queued(tf.priority));
    }
    // Where the task runs, when its resource is ready for it, the task type
    // before it and its rank.
    let (r, ready, previous_type, rank) = match task.frozen_on {
        Some(r) => match task.frozen_previous {
            Some(p) => (
                r,
                frozen_end(plan, p),
                facts.tasks[p].task_type,
                task.frozen_rank,
            ),
            None => (
                r,
                facts.resources[r].start,
                facts.resources[r].initial_type,
                0,
            ),
        },
        None => {
            let r = lane;
            let rf = facts.resources.get(r)?;
            match task.previous {
                Some(p) => {
                    let before = plan.tasks[p].slot?;
                    (r, before.end, facts.tasks[p].task_type, before.rank + 1)
                }
                None => {
                    let frozen = &plan.resources[r].frozen;
                    match frozen.last() {
                        Some(&p) => (
                            r,
                            frozen_end(plan, p),
                            facts.tasks[p].task_type,
                            frozen.len(),
                        ),
                        None => (r, rf.start, rf.initial_type, 0),
                    }
                }
            }
        }
    };
    let rf = &facts.resources[r];
    let (changeover, mut earliest) = begin(facts, rf, tf, ready, previous_type);
    for dep in &tf.deps {
        if let Some(before) = plan.tasks[dep.task].slot.filter(Slot::placed) {
            let anchor = if dep.from_end {
                before.end
            } else {
                before.start
            };
            earliest = earliest.max(anchor + dep.min_offset);
        }
    }
    let minutes = tf.duration[r];
    let (start, end) = match task.frozen_on.and(task.pinned_start) {
        Some(start) => (start, frozen_end(plan, t)),
        None => run(rf, tf, earliest, minutes),
    };
    let setup = changeover.map_or(0, |c| c.minutes);
    let idle = if rf.machine {
        let waited = start - ready - rf.calendar.unavailable_between(ready, start);
        (waited - setup).max(0)
    } else {
        0
    };
    let mut slot = Slot {
        resource: Some(r),
        rank,
        start,
        end,
        changeover,
        idle,
        span: end - rf.start,
        amounts: [0; RULES.len()],
    };
    score(plan, t, &mut slot);
    Some(slot)
}

/// Fills in what every per-task rule costs for task `t` in `slot`.
fn score(plan: &Plan, t: usize, slot: &mut Slot) {
    let facts = &*plan.facts;
    let tf = &facts.tasks[t];
    let r = slot.resource.unwrap_or_default();
    let rf = &facts.resources[r];
    let (start, end) = (slot.start, slot.end);
    let link = facts.broken_link;
    let mut add = |rule: Rule, amount: i64| slot.amounts[rule.index()] += amount;
    if let Some(min_start) = tf.min_start {
        add(Rule::EarliestStart, (min_start - start).max(0));
    }
    if let Some(max_end) = tf.max_end {
        add(Rule::Deadline, (end - max_end).max(0));
    }
    let missing: i64 = tf
        .required_tags
        .iter()
        .map(|&tag| match rf.tags[tag] {
            None => (end - start).max(1),
            Some((from, until)) => {
                let uncovered = (end - start) - overlap(start, end, from, until);
                if uncovered > 0 || (end == start && !(from..until).contains(&start)) {
                    uncovered.max(1)
                } else {
                    0
                }
            }
        })
        .sum();
    add(Rule::TagMissing, missing);
    if tf.allowed.as_ref().is_some_and(|allowed| !allowed[r]) {
        add(Rule::ResourceNotAllowed, tf.priority);
    }
    if let Some(ideal) = tf.ideal_end {
        add(Rule::IdealEnd, (end - ideal).abs() * tf.priority);
    }
    if let Some(due) = tf.due {
        add(Rule::Lateness, (end - due).max(0));
    }
    if let Some(changeover) = slot.changeover {
        add(Rule::ChangeoverCount, 1);
        add(Rule::ChangeoverTime, changeover.minutes);
    }
    add(Rule::IdleTime, slot.idle);
    for dep in &tf.deps {
        // Only the predecessor's slot counts: it is what SolverForge
        // recomputes this slot on. Without one (in no list, or on a
        // resource where it cannot be timed) or in the queue, the
        // predecessor is not scheduled.
        let Some(before) = plan.tasks[dep.task].slot.filter(Slot::placed) else {
            add(Rule::DependencyUnassigned, link);
            continue;
        };
        let rp = before.resource.unwrap_or_default();
        let anchor = if dep.from_end {
            before.end
        } else {
            before.start
        };
        let early = (anchor + dep.min_offset - start).max(0);
        let late = dep
            .max_offset
            .map_or(0, |max| (start - anchor - max).max(0));
        add(Rule::DependencyTiming, early + late);
        if dep.same_resource && rp != r {
            add(Rule::DependencySplit, link);
        }
        if dep.no_intermediate && rp == r {
            let between = slot.rank.abs_diff(before.rank) as i64 - 1;
            add(Rule::DependencyInterrupted, link * between.max(0));
        }
        let hand_over = facts.transition(rp, r);
        if hand_over.forbidden {
            add(Rule::ForbiddenHandOver, link);
        }
        add(Rule::PreferredHandOver, hand_over.reward);
    }
}
