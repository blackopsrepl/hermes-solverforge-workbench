//! `fill`: the best places for what a plan leaves open, ranked by the model.
//!
//! Three kinds of item are open: an unscheduled work order (placed in a
//! furnace at a start, with the best free crew for its tasks), a task of a
//! scheduled order without an operator, and an undecided roster day. Every
//! option is scored by applying it through the incremental director and
//! undoing it again.

use std::collections::{BTreeSet, HashMap};

use solverforge_workbench_host::fill::{FillAnswer, FillRequest, Ranking};
use solverforge_workbench_host::{EngineError, EntityName, Model};

use crate::derived::{div_ceil, Derived, Task};
use crate::domain::Plan;
use crate::labels::{moment, operator};
use crate::model::FurnaceHeatTreatment;
use crate::prepare::{furnace_fits, start_fits};
use crate::time::{format_date_time, overlap};
use crate::wire::ShiftKind;

/// One option for one item: the values it sets.
#[derive(Clone)]
enum Change {
    /// `(order, slot, crew per task)`.
    Place(usize, Option<usize>, [Option<usize>; 4]),
    /// `(order, task, operator)`.
    Crew(usize, Task, Option<usize>),
    /// `(roster row, shift value)`.
    Roster(usize, Option<usize>),
}

struct Item {
    type_name: &'static str,
    name: EntityName,
    options: Vec<(EntityName, Change)>,
    undo: Change,
}

fn descriptor_index(type_name: &str) -> usize {
    Plan::descriptor()
        .entity_descriptors
        .iter()
        .position(|e| e.type_name.rsplit("::").next() == Some(type_name))
        .expect("the plan declares the entity")
}

/// Operator time already booked, by operator.
fn bookings(plan: &Plan, except: usize) -> HashMap<usize, Vec<(i64, i64)>> {
    let mut out: HashMap<usize, Vec<(i64, i64)>> = HashMap::new();
    for order in plan.work_orders.iter().filter(|o| o.index != except) {
        for task in Task::ALL {
            if let (Some(op), Some(window)) = (order.op(task), order.task_window(task)) {
                out.entry(op).or_default().push(window);
            }
        }
    }
    out
}

/// The free, rostered, skilled operator with the fewest tasks for a task
/// window, if anyone is.
fn best_operator(
    plan: &Plan,
    d: &Derived,
    order: usize,
    task: Task,
    window: (i64, i64),
    busy: &HashMap<usize, Vec<(i64, i64)>>,
) -> Option<usize> {
    let shift = d.owner_of(window)?;
    plan.work_orders[order]
        .op_candidates(task)
        .iter()
        .copied()
        .filter(|&op| {
            plan.roster
                .iter()
                .any(|r| r.operator_idx() == op && r.shift_id() == Some(shift))
        })
        .filter(|op| {
            busy.get(op).is_none_or(|spans| {
                spans
                    .iter()
                    .all(|&(a, b)| overlap(a, b, window.0, window.1) == 0)
            })
        })
        .min_by_key(|op| (busy.get(op).map_or(0, Vec::len), *op))
}

/// Starts worth trying for an order on a furnace: its release, right after
/// each load and its changeover, alongside each load of the same recipe, and
/// the first start of each shift that leaves room for load building.
fn starts(plan: &Plan, d: &Derived, order: usize, furnace: usize) -> BTreeSet<i64> {
    let grid = |minute: i64| div_ceil(minute.max(0), d.step) * d.step;
    let recipe = d.recipe_of(order);
    let mut out = BTreeSet::new();
    out.insert(grid(d.orders[order].release.unwrap_or(0)));
    for other in plan.work_orders.iter().filter(|o| o.index != order) {
        let Some(p) = other.placed().filter(|p| p.furnace == furnace) else {
            continue;
        };
        let theirs = d.recipe_of(other.index);
        let gap = d.changeover(furnace, (theirs.temperature, Some(theirs.process)), recipe);
        out.insert(grid(p.end + gap));
        if other.recipe_idx() == d.orders[order].recipe {
            out.insert(p.start);
        }
    }
    let lead = d.s.load_build + d.s.charge + d.s.program;
    for day in 0..d.days {
        for kind in [ShiftKind::Morning, ShiftKind::Afternoon, ShiftKind::Night] {
            let (from, _) = d.shift_bounds(crate::derived::ShiftId { day, kind });
            out.insert(grid(from + lead));
        }
    }
    out.into_iter()
        .filter(|&start| start < d.horizon && start_fits(d, furnace, order, start))
        .collect()
}

fn place_options(plan: &Plan, d: &Derived, order: usize) -> Vec<(EntityName, Change)> {
    let busy = bookings(plan, order);
    let mut options = Vec::new();
    for (furnace, f) in plan.furnaces.iter().enumerate() {
        if !furnace_fits(d, furnace, order) {
            continue;
        }
        for start in starts(plan, d, order, furnace) {
            let slot = d.value(furnace, (start / d.step) as usize);
            let end = start + d.recipe_of(order).cycle;
            let mut crew = [None; 4];
            let mut taken = busy.clone();
            for (n, task) in Task::ALL.into_iter().enumerate() {
                if let Some(window) = d.task_window(order, task, start, end) {
                    crew[n] = best_operator(plan, d, order, task, window, &taken);
                    if let Some(op) = crew[n] {
                        taken.entry(op).or_default().push(window);
                    }
                }
            }
            let sharing: Vec<&str> = plan
                .work_orders
                .iter()
                .filter(|o| {
                    o.index != order
                        && o.slot == Some(slot)
                        && o.recipe_idx() == d.orders[order].recipe
                })
                .map(|o| o.id.as_str())
                .collect();
            let mut label = format!("{}, {}", f.label(), moment(d, start));
            if !sharing.is_empty() {
                label.push_str(&format!(", with {}", sharing.join(", ")));
            }
            options.push((
                EntityName {
                    id: Some(format!("{}@{}", f.id, format_date_time(d.origin + start))),
                    label,
                },
                Change::Place(order, Some(slot), crew),
            ));
        }
    }
    options
}

fn open_items(plan: &Plan, request: &FillRequest) -> Vec<Item> {
    let Some(d) = plan.work_orders.first().map(|o| o.ctx.clone()) else {
        return Vec::new();
    };
    let wanted = |id: &str, open: bool| {
        if let Some(choice) = &request.apply {
            return choice.item == id;
        }
        if request.items.is_empty() {
            open
        } else {
            request.items.iter().any(|item| item == id)
        }
    };
    let mut items = Vec::new();
    for order in &plan.work_orders {
        let i = order.index;
        let crew = Task::ALL.map(|t| order.op(t));
        if !order.pinned && wanted(&order.id, order.slot.is_none()) {
            items.push(Item {
                type_name: "WorkOrder",
                name: crate::names::name_entity(plan, "WorkOrder", &i.to_string()).expect("order"),
                options: place_options(plan, &d, i),
                undo: Change::Place(i, order.slot, crew),
            });
        }
        if order.pinned {
            continue;
        }
        for task in order.tasks() {
            let id = format!("{}/{}", order.id, task.key());
            if !wanted(&id, order.op(task).is_none()) {
                continue;
            }
            let options = order
                .op_candidates(task)
                .iter()
                .map(|&op| {
                    (
                        EntityName {
                            id: Some(plan.operators[op].id.clone()),
                            label: operator(plan, op),
                        },
                        Change::Crew(i, task, Some(op)),
                    )
                })
                .collect();
            items.push(Item {
                type_name: "Task",
                name: EntityName {
                    id: Some(id),
                    label: format!("{} {}", order.id, task.label()),
                },
                options,
                undo: Change::Crew(i, task, order.op(task)),
            });
        }
    }
    for row in plan.roster.iter().filter(|r| !r.pinned) {
        if !wanted(&row.id, row.shift_value.is_none()) {
            continue;
        }
        let options = row
            .allowed
            .iter()
            .filter_map(|&value| {
                let kind = ShiftKind::from_value(value)?;
                Some((
                    EntityName {
                        id: Some(kind.label().to_string()),
                        label: match kind {
                            ShiftKind::Off => "Day off".to_string(),
                            kind => format!("{} shift", kind.label()),
                        },
                    },
                    Change::Roster(row.index, Some(value)),
                ))
            })
            .collect();
        items.push(Item {
            type_name: "RosterDay",
            name: crate::names::name_entity(plan, "RosterDay", &row.index.to_string())
                .expect("row"),
            options,
            undo: Change::Roster(row.index, row.shift_value),
        });
    }
    items
}

fn apply_change(plan: &mut Plan, change: &Change) {
    match change {
        Change::Place(order, slot, crew) => {
            let order = &mut plan.work_orders[*order];
            order.slot = *slot;
            for (n, task) in Task::ALL.into_iter().enumerate() {
                *order.op_mut(task) = crew[n];
            }
        }
        Change::Crew(order, task, op) => *plan.work_orders[*order].op_mut(*task) = *op,
        Change::Roster(row, value) => plan.roster[*row].shift_value = *value,
    }
}

fn target(change: &Change, orders: usize, roster: usize) -> (usize, usize) {
    match change {
        Change::Place(i, ..) | Change::Crew(i, ..) => (orders, *i),
        Change::Roster(i, _) => (roster, *i),
    }
}

/// Ranked options for every open item, or the plan with one option applied.
pub fn fill(plan: Plan, request: &FillRequest) -> Result<FillAnswer<Plan>, EngineError> {
    let items = open_items(&plan, request);
    let (orders, roster) = (descriptor_index("WorkOrder"), descriptor_index("RosterDay"));
    let mut ranking =
        Ranking::<FurnaceHeatTreatment, _>::new(plan, FurnaceHeatTreatment::constraints());
    if let Some(choice) = &request.apply {
        let item = items
            .iter()
            .find(|item| item.name.id.as_deref() == Some(choice.item.as_str()))
            .ok_or_else(|| {
                EngineError::new(
                    "unknown_item",
                    format!("no open item {:?} to fill", choice.item),
                )
            })?;
        let (_, change) = item
            .options
            .iter()
            .find(|(name, _)| name.id.as_deref() == Some(choice.value.as_str()))
            .ok_or_else(|| {
                EngineError::new(
                    "unknown_value",
                    format!("{:?} is not a place for {:?}", choice.value, choice.item),
                )
            })?;
        let (descriptor, entity) = target(change, orders, roster);
        let change = change.clone();
        ranking
            .director()
            .do_change(descriptor, entity, move |plan: &mut Plan| {
                apply_change(plan, &change)
            });
        return Ok(FillAnswer::Applied(ranking.into_plan()));
    }
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let changes: Vec<Change> = item.options.iter().map(|(_, c)| c.clone()).collect();
        let names = item.options.into_iter().map(|(name, _)| name).collect();
        let undo = item.undo.clone();
        out.push(ranking.rank(
            item.type_name,
            item.name,
            names,
            request.top.max(1),
            &mut |director, n| {
                let change = changes[n].clone();
                let (descriptor, entity) = target(&change, orders, roster);
                director.do_change(descriptor, entity, move |plan: &mut Plan| {
                    apply_change(plan, &change)
                })
            },
            &mut |director, _| {
                let undo = undo.clone();
                let (descriptor, entity) = target(&undo, orders, roster);
                director.do_change(descriptor, entity, move |plan: &mut Plan| {
                    apply_change(plan, &undo)
                });
            },
        ));
    }
    Ok(FillAnswer::Options(out))
}
