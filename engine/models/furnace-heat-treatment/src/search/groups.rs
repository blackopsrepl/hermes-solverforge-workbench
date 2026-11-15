//! Grouped moves for the solver: a work order placed together with its crew
//! (rostering a free operator onto the shift when no rostered one is free),
//! and a scheduled order re-crewed.
//!
//! Placing an order alone leaves its four tasks without operators, which
//! costs far more than leaving it unscheduled, so single-variable moves
//! never schedule anything; these candidates change everything a placement
//! needs at once and leave the scoring to the director.

use std::collections::HashMap;

use solverforge::prelude::*;

use crate::derived::{div_ceil, Derived, ShiftId, Task};
use crate::domain::Plan;
use crate::prepare::{furnace_fits, start_fits};
use crate::time::overlap;
use crate::wire::ShiftKind;

/// One load on a furnace.
#[derive(Clone)]
struct Batch {
    start: i64,
    end: i64,
    recipe: usize,
    kg: i64,
    orders: Vec<usize>,
}

/// What the plan looks like right now, indexed for fast candidate checks.
struct Board<'a> {
    plan: &'a Plan,
    d: &'a Derived,
    /// Loads per furnace, sorted by start.
    loads: Vec<Vec<Batch>>,
    /// Booked task windows per operator: `(start, end, order)`.
    busy: HashMap<usize, Vec<(i64, i64, usize)>>,
    /// Roster row of `(operator, day)`.
    rows: HashMap<(usize, i64), usize>,
}

impl<'a> Board<'a> {
    fn new(plan: &'a Plan, d: &'a Derived) -> Self {
        let mut loads = vec![Vec::new(); plan.furnaces.len()];
        let mut busy: HashMap<usize, Vec<(i64, i64, usize)>> = HashMap::new();
        for order in &plan.work_orders {
            let Some(p) = order.placed() else { continue };
            let list: &mut Vec<Batch> = &mut loads[p.furnace];
            match list
                .iter_mut()
                .find(|l| l.start == p.start && l.recipe == order.recipe_idx())
            {
                Some(load) => {
                    load.kg += order.load_kg;
                    load.orders.push(order.index);
                }
                None => list.push(Batch {
                    start: p.start,
                    end: p.end,
                    recipe: order.recipe_idx(),
                    kg: order.load_kg,
                    orders: vec![order.index],
                }),
            }
            for task in Task::ALL {
                if let (Some(op), Some((s, e))) = (order.op(task), order.task_window(task)) {
                    busy.entry(op).or_default().push((s, e, order.index));
                }
            }
        }
        for list in &mut loads {
            list.sort_by_key(|l| (l.start, l.recipe));
        }
        let rows = plan
            .roster
            .iter()
            .map(|r| ((r.operator_idx(), r.day_index()), r.index))
            .collect();
        Self {
            plan,
            d,
            loads,
            busy,
            rows,
        }
    }

    /// Whether order `o` fits on `furnace` at `start` without overlapping
    /// another load, overfilling a shared one or cutting a changeover short.
    fn furnace_clean(&self, o: usize, furnace: usize, start: i64) -> bool {
        let d = self.d;
        let recipe = d.recipe_of(o);
        let end = start + recipe.cycle;
        let mine = d.orders[o].recipe;
        let kg = d.orders[o].load_kg;
        let mut before: Option<(i64, usize)> = d.furnaces[furnace]
            .initial_temperature
            .map(|_| (0, usize::MAX));
        let mut after: Option<(i64, usize)> = None;
        for Batch {
            start: s,
            end: e,
            recipe: r,
            kg: load_kg,
            orders,
        } in &self.loads[furnace]
        {
            let others: Vec<usize> = orders.iter().copied().filter(|&x| x != o).collect();
            if others.is_empty() {
                continue;
            }
            let other_kg = load_kg - if others.len() < orders.len() { kg } else { 0 };
            if *s == start && *r == mine {
                if other_kg + kg > d.furnaces[furnace].max_load_kg {
                    return false;
                }
                continue;
            }
            if overlap(start, end, *s, *e) > 0 {
                return false;
            }
            if *e <= start && before.is_none_or(|(b, _)| *e >= b) {
                before = Some((*e, *r));
            }
            if *s >= end && after.is_none_or(|(a, _)| *s < a) {
                after = Some((*s, *r));
            }
        }
        if let Some((free, r)) = before {
            let from = if r == usize::MAX {
                (d.furnaces[furnace].initial_temperature.unwrap_or(0), None)
            } else {
                (d.recipes[r].temperature, Some(d.recipes[r].process))
            };
            if start - free < d.changeover(furnace, from, recipe) {
                return false;
            }
        }
        if let Some((next, r)) = after {
            let gap = d.changeover(
                furnace,
                (recipe.temperature, Some(recipe.process)),
                &d.recipes[r],
            );
            if next - end < gap {
                return false;
            }
        }
        true
    }

    fn rostered(&self, op: usize, shift: ShiftId) -> bool {
        self.rows
            .get(&(op, shift.day))
            .is_some_and(|&r| self.plan.roster[r].shift_id() == Some(shift))
    }

    /// Whether `op` could be rostered onto `shift`: the day is open to it.
    fn rosterable(&self, op: usize, shift: ShiftId) -> Option<usize> {
        let &row = self.rows.get(&(op, shift.day))?;
        let r = &self.plan.roster[row];
        (!r.pinned && !r.is_working() && r.allowed.contains(&shift.kind.value())).then_some(row)
    }

    fn free(
        &self,
        op: usize,
        (s, e): (i64, i64),
        order: usize,
        extra: &[(usize, (i64, i64))],
    ) -> bool {
        self.busy.get(&op).is_none_or(|spans| {
            spans
                .iter()
                .all(|&(a, b, o)| o == order || overlap(a, b, s, e) == 0)
        }) && extra
            .iter()
            .all(|&(x, (a, b))| x != op || overlap(a, b, s, e) == 0)
    }

    fn load_count(&self, op: usize) -> usize {
        self.busy.get(&op).map_or(0, Vec::len)
    }

    /// The crew for order `o` running `[start, end)`: per task the free
    /// rostered skilled operator with the fewest tasks, else a free skilled
    /// operator who can be rostered onto the shift (returned in `roster`),
    /// else none. Returns the crew and how many tasks stay uncovered.
    fn crew(
        &self,
        o: usize,
        start: i64,
        end: i64,
    ) -> ([Option<usize>; 4], Vec<(usize, usize)>, i64) {
        let order = &self.plan.work_orders[o];
        let mut crew = [None; 4];
        let mut roster: Vec<(usize, usize)> = Vec::new();
        let mut taken: Vec<(usize, (i64, i64))> = Vec::new();
        let mut missing = 0;
        for (n, task) in Task::ALL.into_iter().enumerate() {
            let Some(window) = self.d.task_window(o, task, start, end) else {
                continue;
            };
            let Some(shift) = self.d.owner_of(window) else {
                missing += 1;
                continue;
            };
            let candidates = order.op_candidates(task);
            let on_shift = |op: usize| {
                self.rostered(op, shift)
                    || roster.iter().any(|&(row, value)| {
                        let r = &self.plan.roster[row];
                        r.operator_idx() == op
                            && r.day_index() == shift.day
                            && value == shift.kind.value()
                    })
            };
            let pick = candidates
                .iter()
                .copied()
                .filter(|&op| on_shift(op) && self.free(op, window, o, &taken))
                .min_by_key(|&op| (self.load_count(op), op))
                .or_else(|| {
                    let op = candidates
                        .iter()
                        .copied()
                        .filter(|&op| {
                            self.rosterable(op, shift).is_some() && self.free(op, window, o, &taken)
                        })
                        .min_by_key(|&op| (self.load_count(op), op))?;
                    roster.push((self.rosterable(op, shift)?, shift.kind.value()));
                    Some(op)
                });
            match pick {
                Some(op) => {
                    crew[n] = Some(op);
                    taken.push((op, window));
                }
                None => missing += 1,
            }
        }
        (crew, roster, missing)
    }

    /// A cheap guess of what placing `o` at `start` on `furnace` costs.
    fn value_key(
        &self,
        o: usize,
        furnace: usize,
        start: i64,
        missing: i64,
        rostered: usize,
    ) -> i64 {
        let d = self.d;
        let info = &d.orders[o];
        let recipe = d.recipe_of(o);
        let end = start + recipe.cycle;
        let late = (end - info.due).max(0) * info.priority.lateness_per_minute();
        let early = (info.due - end).max(0) / 40;
        let night = if !recipe.process.runs_overnight()
            && d.owner(start).is_some_and(|s| s.kind == ShiftKind::Night)
        {
            info.priority.night_start_cost()
        } else {
            0
        };
        let shares = self.loads[furnace]
            .iter()
            .any(|l| l.start == start && l.recipe == info.recipe && !l.orders.contains(&o));
        let energy = if shares {
            0
        } else {
            d.energy_cents(furnace, start, end)
        };
        missing * 4_500_000 + rostered as i64 * 2_000 + late + early + night + energy
    }

    /// Starts worth trying on a furnace: after each load and its changeover,
    /// alongside loads of the same recipe, at the release, and the first
    /// start of each shift that leaves room for load building.
    fn starts(&self, o: usize, furnace: usize) -> Vec<i64> {
        let d = self.d;
        let grid = |minute: i64| div_ceil(minute.max(0), d.step) * d.step;
        let recipe = d.recipe_of(o);
        let mut out = vec![grid(d.orders[o].release.unwrap_or(0))];
        if let Some(t) = d.furnaces[furnace].initial_temperature {
            out.push(grid(d.changeover(furnace, (t, None), recipe)));
        }
        for Batch {
            start: s,
            end: e,
            recipe: r,
            orders,
            ..
        } in &self.loads[furnace]
        {
            if orders.iter().all(|&x| x == o) {
                continue;
            }
            let theirs = &d.recipes[*r];
            out.push(grid(
                e + d.changeover(furnace, (theirs.temperature, Some(theirs.process)), recipe),
            ));
            if *r == d.orders[o].recipe {
                out.push(*s);
            }
            // Just before the load, leaving its changeover.
            let back = s
                - d.changeover(furnace, (recipe.temperature, Some(recipe.process)), theirs)
                - recipe.cycle;
            out.push(back.div_euclid(d.step) * d.step);
        }
        let lead = d.s.load_build + d.s.charge + d.s.program;
        for day in 0..d.days {
            for kind in [ShiftKind::Morning, ShiftKind::Afternoon, ShiftKind::Night] {
                out.push(grid(d.shift_bounds(ShiftId { day, kind }).0 + lead));
            }
        }
        out.sort_unstable();
        out.dedup();
        out.retain(|&s| {
            s >= 0
                && s < d.horizon
                && start_fits(d, furnace, o, s)
                && self.furnace_clean(o, furnace, s)
        });
        out
    }

    /// The `limit` best placements of order `o`, best first.
    fn placements(&self, o: usize, limit: usize) -> Vec<(i64, ScalarCandidate<Plan>)> {
        let d = self.d;
        let mut options = Vec::new();
        for furnace in 0..self.plan.furnaces.len() {
            if !furnace_fits(d, furnace, o) {
                continue;
            }
            for start in self.starts(o, furnace) {
                let end = start + d.recipe_of(o).cycle;
                let (crew, roster, missing) = self.crew(o, start, end);
                let key = self.value_key(o, furnace, start, missing, roster.len());
                options.push((key, furnace, start, crew, roster));
            }
        }
        options.sort_by_key(|option| option.0);
        options.truncate(limit);
        let order = &self.plan.work_orders[o];
        options
            .into_iter()
            .filter(|(_, furnace, start, crew, _)| {
                order.slot != Some(d.value(*furnace, (*start / d.step) as usize))
                    || Task::ALL
                        .into_iter()
                        .enumerate()
                        .any(|(n, t)| order.op(t) != crew[n])
            })
            .map(|(key, furnace, start, crew, roster)| {
                let slot = d.value(furnace, (start / d.step) as usize);
                let mut edits = vec![Plan::work_orders().scalar("slot").set(o, Some(slot))];
                for (n, task) in Task::ALL.into_iter().enumerate() {
                    if order.op(task) != crew[n] {
                        edits.push(
                            Plan::work_orders()
                                .scalar(op_variable(task))
                                .set(o, crew[n]),
                        );
                    }
                }
                for (row, value) in roster {
                    edits.push(Plan::roster().scalar("shift_value").set(row, Some(value)));
                }
                (key, ScalarCandidate::new("placement", edits))
            })
            .collect()
    }
}

pub fn op_variable(task: Task) -> &'static str {
    match task {
        Task::LoadBuild => "load_build_op",
        Task::Program => "program_op",
        Task::Quench => "quench_op",
        Task::Unload => "unload_op",
    }
}

/// A position that changes as the plan changes, so local search pulls start
/// at a different order each step.
fn rotation(plan: &Plan) -> usize {
    plan.work_orders
        .iter()
        .map(|o| o.slot.unwrap_or(7).wrapping_mul(o.index + 1))
        .chain(
            plan.roster
                .iter()
                .map(|r| r.shift_value.unwrap_or(5) * (r.index + 3)),
        )
        .fold(0usize, |acc, x| acc.wrapping_mul(31).wrapping_add(x))
}

/// Construction places every unscheduled order (most urgent and least
/// flexible first); local search re-places orders from a rotating start.
pub fn placement_candidates(plan: &Plan, limits: ScalarGroupLimits) -> Vec<ScalarCandidate<Plan>> {
    let Some(d) = plan.work_orders.first().map(|o| o.ctx.clone()) else {
        return Vec::new();
    };
    let board = Board::new(plan, &d);
    let construction = limits.max_moves_per_step.is_none();
    let per_order = limits.value_candidate_limit.unwrap_or(16);
    let total = limits
        .group_candidate_limit
        .or(limits.max_moves_per_step)
        .unwrap_or(usize::MAX);
    let n = plan.work_orders.len();
    let first = if construction {
        0
    } else {
        rotation(plan) % n.max(1)
    };
    let mut out = Vec::new();
    for k in 0..n {
        let o = (first + k) % n;
        let order = &plan.work_orders[o];
        if order.pinned || (construction && order.slot.is_some()) {
            continue;
        }
        let info = &d.orders[o];
        let entity_key =
            info.due * 10 - info.priority.weight() * 2_000 + order.candidates.len() as i64 / 64;
        for (value_key, candidate) in board.placements(o, per_order) {
            if out.len() >= total {
                return out;
            }
            out.push(
                candidate
                    .with_construction_slot_key(o)
                    .with_construction_entity_order_key(entity_key)
                    .with_construction_value_order_key(value_key),
            );
        }
    }
    out
}

/// Re-crews scheduled orders whose crew is incomplete, off its shift or double-booked.
pub fn crew_candidates(plan: &Plan, limits: ScalarGroupLimits) -> Vec<ScalarCandidate<Plan>> {
    let Some(d) = plan.work_orders.first().map(|o| o.ctx.clone()) else {
        return Vec::new();
    };
    let board = Board::new(plan, &d);
    let total = limits.max_moves_per_step.unwrap_or(usize::MAX);
    let n = plan.work_orders.len();
    let first = rotation(plan) % n.max(1);
    let mut out = Vec::new();
    for k in 0..n {
        if out.len() >= total {
            break;
        }
        let o = (first + k) % n;
        let order = &plan.work_orders[o];
        let Some(p) = order.placed().filter(|_| !order.pinned) else {
            continue;
        };
        let needs = order
            .tasks()
            .any(|t| match (order.op(t), order.task_window(t)) {
                (None, _) => true,
                (Some(op), Some(w)) => {
                    d.owner_of(w).is_none_or(|s| !board.rostered(op, s))
                        || !board.free(op, w, o, &[])
                }
                _ => false,
            });
        if !needs {
            continue;
        }
        let (crew, roster, _) = board.crew(o, p.start, p.end);
        let mut edits = Vec::new();
        for (n, task) in Task::ALL.into_iter().enumerate() {
            if order.op(task) != crew[n] && crew[n].is_some() {
                edits.push(
                    Plan::work_orders()
                        .scalar(op_variable(task))
                        .set(o, crew[n]),
                );
            }
        }
        for (row, value) in roster {
            edits.push(Plan::roster().scalar("shift_value").set(row, Some(value)));
        }
        if !edits.is_empty() {
            out.push(ScalarCandidate::new("crew", edits).with_construction_slot_key(o));
        }
    }
    out
}
