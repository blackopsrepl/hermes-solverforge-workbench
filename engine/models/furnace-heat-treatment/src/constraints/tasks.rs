//! Manual tasks around each load: shift ownership, crew, skills, double
//! bookings and the monitoring crew.

use solverforge::prelude::*;
use solverforge::stream::collector::sum;
use solverforge::stream::joiner::equal;

use crate::derived::Task;
use crate::domain::{Plan, WorkOrder};
use crate::eval::{
    Booking, Bookings, Monitor, MonitorCollector, OrderShiftWork, OrderWatch, RosterShiftWork,
    RosterWatch, ShiftWork, WatchRow,
};

type Score = HardMediumSoftScore;

fn f() -> ConstraintFactory<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
}

/// A scheduled order's task that no single shift owns.
fn unowned(order: &WorkOrder, task: Task) -> bool {
    order
        .task_window(task)
        .is_some_and(|window| order.ctx.owner_of(window).is_none())
}

fn ownership(name: &'static str, task: Task) -> impl IncrementalConstraint<Plan, Score> {
    f().for_each(Plan::work_orders())
        .filter(move |o: &WorkOrder| unowned(o, task))
        .penalize(hard_weight(|_: &WorkOrder| Score::of_hard(900)))
        .named(name)
}

/// Operators given to tasks of an order that is not scheduled.
fn crew_without_load(order: &WorkOrder) -> i64 {
    if order.slot.is_some() {
        return 0;
    }
    Task::ALL
        .into_iter()
        .filter(|t| order.op(*t).is_some())
        .count() as i64
}

/// Tasks of a scheduled order that have no operator.
fn missing_crew(order: &WorkOrder) -> i64 {
    order.tasks().filter(|t| order.op(*t).is_none()).count() as i64
}

/// Tasks of an order given to an operator without the skills they need.
fn unskilled(order: &WorkOrder) -> i64 {
    let d = &order.ctx;
    order
        .tasks()
        .filter(|t| {
            order.op(*t).is_some_and(|op| {
                let needed = d.task_skills(order.index, *t);
                d.operators[op].skills & needed != needed
            })
        })
        .count() as i64
}

pub fn constraints() -> impl ConstraintSet<Plan, Score> {
    (
        ownership("Load building crosses a shift change", Task::LoadBuild),
        ownership("Programming crosses a shift change", Task::Program),
        ownership("Quench crosses a shift change", Task::Quench),
        ownership("Unloading crosses a shift change", Task::Unload),
        f().for_each(Plan::work_orders())
            .filter(|o: &WorkOrder| crew_without_load(o) > 0)
            .penalize(hard_weight(|o: &WorkOrder| {
                Score::of_hard(crew_without_load(o) * 900)
            }))
            .named("Crew on an unscheduled order"),
        f().for_each(Plan::work_orders())
            .filter(|o: &WorkOrder| missing_crew(o) > 0)
            .penalize(hard_weight(|o: &WorkOrder| {
                Score::of_hard(missing_crew(o) * 4_500)
            }))
            .named("Task without an operator"),
        f().for_each(Plan::work_orders())
            .filter(|o: &WorkOrder| unskilled(o) > 0)
            .penalize(hard_weight(|o: &WorkOrder| {
                Score::of_hard(unskilled(o) * 900)
            }))
            .named("Operator lacks the skill for a task"),
        f().for_each(Plan::work_orders())
            .project(OrderShiftWork)
            .merge(f().for_each(Plan::roster()).project(RosterShiftWork))
            .group_by(
                |w: &ShiftWork| (w.operator, w.shift),
                sum(|w: &ShiftWork| w.delta),
            )
            .penalize(hard_weight(|_: &(usize, usize), tasks: &i64| {
                Score::of_hard((*tasks).max(0) * 900)
            }))
            .named("Operator not rostered for a task's shift"),
        f().for_each(Plan::work_orders())
            .project(Bookings)
            .join(equal(|b: &Booking| b.operator))
            .filter(|a: &Booking, b: &Booking| !a.same_job(b) && a.overlap(b) > 0)
            .penalize(hard_weight(|a: &Booking, b: &Booking| {
                Score::of_hard(a.overlap(b) * 900)
            }))
            .named("Operator double-booked"),
        f().for_each(Plan::work_orders())
            .project(OrderWatch)
            .merge(f().for_each(Plan::roster()).project(RosterWatch))
            .group_by(|row: &WatchRow| row.bucket, MonitorCollector)
            .penalize(hard_weight(|_: &i64, m: &Monitor| {
                Score::of_hard(m.shortage() * 900)
            }))
            .named("Too few operators monitoring"),
    )
}
