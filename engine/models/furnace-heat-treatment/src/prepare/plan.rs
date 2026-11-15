//! `prepare`: validate a dataset, resolve its ids and derive its facts.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use solverforge_workbench_host::Issue;

use super::candidates::{order_candidates, task_candidates};
use crate::derived::{
    Ctx, Derived, FurnaceInfo, OperatorInfo, OrderInfo, RecipeInfo, RosterInfo, Settings, ShiftId,
    Task, Values,
};
use crate::domain::{Plan, RosterDay};
use crate::time::{format_date, parse_date, parse_date_time, DAY};
use crate::wire::{Parameters, ShiftKind};

/// Longest cycle a recipe may have, in minutes: seven days.
pub const MAX_CYCLE: i64 = 7 * DAY;
/// Longest schedule, in days.
pub const MAX_DAYS: i64 = 31;

struct Issues(Vec<Issue>);

impl Issues {
    fn error(&mut self, code: &'static str, message: String) {
        self.0.push(Issue::error(code, message));
    }
    fn warning(&mut self, code: &'static str, message: String) {
        self.0.push(Issue::warning(code, message));
    }
    fn has_errors(&self) -> bool {
        self.0
            .iter()
            .any(|issue| issue.severity == solverforge_workbench_host::Severity::Error)
    }
}

/// Validates `plan`, resolves its ids and fills every derived field.
pub fn prepare(plan: &mut Plan) -> Vec<Issue> {
    let mut b = Issues(Vec::new());
    let Some((origin, days)) = horizon(plan, &mut b) else {
        return b.0;
    };
    let Some((settings, step)) = settings(&plan.parameters, &mut b) else {
        return b.0;
    };
    let horizon = days * DAY;
    let mut d = Derived {
        origin,
        days,
        horizon,
        step,
        slots: (horizon / step) as usize,
        s: settings,
        ..Derived::default()
    };
    let origin_day = origin / DAY;
    let roster_day = |text: &str| parse_date(text).map(|date| date - origin_day);

    // Recipes.
    let mut recipe_ids = HashMap::new();
    for (index, recipe) in plan.recipes.iter().enumerate() {
        if recipe_ids.insert(recipe.id.clone(), index).is_some() {
            b.error(
                "duplicate_recipe",
                format!("recipe id {:?} is used more than once", recipe.id),
            );
        }
        if recipe.temperature <= 0 {
            b.error(
                "invalid_recipe",
                format!(
                    "recipe {:?} has setpoint {} °C; it must be positive",
                    recipe.id, recipe.temperature
                ),
            );
        }
        if !(1..=MAX_CYCLE).contains(&recipe.cycle_minutes) {
            b.error(
                "invalid_recipe",
                format!(
                    "recipe {:?} has a cycle of {} minutes; it must be 1 to {MAX_CYCLE}",
                    recipe.id, recipe.cycle_minutes
                ),
            );
        }
        d.recipes.push(RecipeInfo {
            process: recipe.process,
            temperature: recipe.temperature,
            cycle: recipe.cycle_minutes.clamp(1, MAX_CYCLE),
            quench: recipe.quench.unwrap_or(recipe.process.quenches()),
        });
    }

    // Furnaces.
    let mut furnace_ids = HashMap::new();
    for (index, furnace) in plan.furnaces.iter_mut().enumerate() {
        furnace.index = index;
        if furnace_ids.insert(furnace.id.clone(), index).is_some() {
            b.error(
                "duplicate_furnace",
                format!("furnace id {:?} is used more than once", furnace.id),
            );
        }
        if furnace.heating_rate < 1 || furnace.cooling_rate < 1 {
            b.error(
                "invalid_furnace",
                format!(
                    "furnace {:?} must heat and cool at least 1 °C per minute",
                    furnace.id
                ),
            );
        }
        if furnace.max_temperature <= 0 || furnace.max_load_kg <= 0 || furnace.power_kw < 0 {
            b.error("invalid_furnace", format!("furnace {:?} needs a positive maximum temperature and load and a power that is not negative", furnace.id));
        }
        if furnace.processes.is_empty() {
            b.warning(
                "idle_furnace",
                format!(
                    "furnace {:?} runs no process, so no load can go in it",
                    furnace.id
                ),
            );
        }
        let mut downtime = Vec::new();
        for window in &furnace.downtime {
            match (parse_date_time(&window.from), parse_date_time(&window.to)) {
                (Some(from), Some(to)) if to > from => downtime.push((from - origin, to - origin)),
                _ => b.error("invalid_downtime", format!("furnace {:?} has downtime {:?} to {:?}; it needs two date-times, the second after the first", furnace.id, window.from, window.to)),
            }
        }
        downtime.sort_unstable();
        d.furnaces.push(FurnaceInfo {
            processes: furnace.processes.iter().fold(0, |mask, p| mask | p.bit()),
            max_temperature: furnace.max_temperature,
            max_load_kg: furnace.max_load_kg,
            heating_rate: furnace.heating_rate.max(1),
            cooling_rate: furnace.cooling_rate.max(1),
            power_kw: furnace.power_kw.max(0),
            initial_temperature: furnace.initial_temperature,
            downtime,
        });
    }
    if plan.furnaces.is_empty() {
        b.warning("no_furnaces", "the dataset has no furnaces".into());
    }

    // Operators.
    let mut operator_ids = HashMap::new();
    for (index, operator) in plan.operators.iter_mut().enumerate() {
        operator.index = index;
        if operator_ids.insert(operator.id.clone(), index).is_some() {
            b.error(
                "duplicate_operator",
                format!("operator id {:?} is used more than once", operator.id),
            );
        }
        let mut unavailable = Vec::new();
        for date in &operator.unavailable {
            match roster_day(date) {
                Some(day) => unavailable.push(day),
                None => b.error(
                    "invalid_date",
                    format!(
                        "operator {:?} is unavailable on {date:?}, which is not a date YYYY-MM-DD",
                        operator.id
                    ),
                ),
            }
        }
        d.operators.push(OperatorInfo {
            skills: operator
                .skills
                .iter()
                .fold(operator.role.base_skills(), |m, s| m | s.bit()),
            role: operator.role,
            day_only: operator.day_only,
            unavailable,
        });
    }

    // Roster rows: one per operator and roster day, missing ones added open.
    let mut seen: HashSet<(usize, i64)> = HashSet::new();
    let mut roster_ids = HashSet::new();
    for row in &plan.roster {
        let Some(&operator) = operator_ids.get(&row.operator) else {
            b.error(
                "unknown_operator",
                format!(
                    "roster row {:?} names operator {:?}, which is not in the dataset",
                    row.id, row.operator
                ),
            );
            continue;
        };
        let Some(day) = roster_day(&row.day).filter(|day| (-1..days).contains(day)) else {
            b.error("invalid_roster_day", format!("roster row for {:?} on {:?} is not a date from the day before the schedule to its last day", row.operator, row.day));
            continue;
        };
        if !seen.insert((operator, day)) {
            b.error(
                "duplicate_roster_day",
                format!(
                    "operator {:?} has more than one roster row on {}",
                    row.operator, row.day
                ),
            );
        }
    }
    for (operator_idx, operator) in plan.operators.iter().enumerate() {
        for day in -1..days {
            if !seen.contains(&(operator_idx, day)) {
                plan.roster.push(RosterDay {
                    index: 0,
                    id: String::new(),
                    operator: operator.id.clone(),
                    day: format_date(origin_day + day),
                    shift: None,
                    pinned: false,
                    allowed: Values::default(),
                    shift_value: None,
                    ctx: Ctx::default(),
                });
            }
        }
    }
    for (index, row) in plan.roster.iter_mut().enumerate() {
        row.index = index;
        if row.id.is_empty() {
            row.id = format!("{}@{}", row.operator, row.day);
        }
        if !roster_ids.insert(row.id.clone()) {
            b.error(
                "duplicate_roster_id",
                format!("roster id {:?} is used more than once", row.id),
            );
        }
        d.roster.push(RosterInfo {
            operator: operator_ids.get(&row.operator).copied().unwrap_or(0),
            day: roster_day(&row.day).unwrap_or(-1),
        });
    }
    if row_shifts_invalid(plan, &mut b) || b.has_errors() {
        return b.0;
    }

    // Coverage.
    for (index, demand) in plan.coverage.iter_mut().enumerate() {
        demand.index = index;
        if demand.id.is_empty() {
            demand.id = format!("{}/{}/{:?}", demand.day, demand.shift.label(), demand.role);
        }
        let day = roster_day(&demand.day).filter(|day| (-1..days).contains(day));
        match (day, demand.shift) {
            (_, ShiftKind::Off) => b.error("invalid_coverage", format!("coverage {:?} asks for operators on a day off", demand.id)),
            (Some(-1), kind) if kind != ShiftKind::Night => b.error("invalid_coverage", format!("coverage {:?} is on the day before the schedule, which only has its night shift", demand.id)),
            (Some(day), kind) => demand.shift_number = ShiftId { day, kind }.number(),
            (None, _) => b.error("invalid_coverage", format!("coverage {:?} is not on a roster day of the schedule", demand.id)),
        }
        if demand.count < 0 {
            b.error(
                "invalid_coverage",
                format!(
                    "coverage {:?} asks for {} operators",
                    demand.id, demand.count
                ),
            );
        }
    }

    // Work orders.
    let mut order_ids = HashSet::new();
    let mut placements = Vec::with_capacity(plan.work_orders.len());
    for (index, order) in plan.work_orders.iter_mut().enumerate() {
        order.index = index;
        if !order_ids.insert(order.id.clone()) {
            b.error(
                "duplicate_order",
                format!("work order id {:?} is used more than once", order.id),
            );
        }
        let recipe = recipe_ids.get(&order.recipe).copied();
        if recipe.is_none() {
            b.error(
                "unknown_recipe",
                format!(
                    "work order {:?} needs recipe {:?}, which is not in the dataset",
                    order.id, order.recipe
                ),
            );
        }
        if order.load_kg <= 0 {
            b.error(
                "invalid_load",
                format!(
                    "work order {:?} weighs {} kg; it must be positive",
                    order.id, order.load_kg
                ),
            );
        }
        let due = parse_date_time(&order.due).map(|due| due - origin);
        if due.is_none() {
            b.error(
                "invalid_due",
                format!(
                    "work order {:?} is due {:?}, which is not a date-time YYYY-MM-DDTHH:MM",
                    order.id, order.due
                ),
            );
        }
        let release = match &order.release {
            None => None,
            Some(text) => match parse_date_time(text) {
                Some(at) => Some(at - origin),
                None => {
                    b.error("invalid_release", format!("work order {:?} is released {text:?}, which is not a date-time YYYY-MM-DDTHH:MM", order.id));
                    None
                }
            },
        };
        let placement = match (&order.furnace, &order.start) {
            (None, None) => None,
            (Some(furnace), Some(start)) => {
                let furnace_idx = furnace_ids.get(furnace).copied();
                let minute = parse_date_time(start).map(|at| at - origin);
                match (furnace_idx, minute) {
                    (None, _) => {
                        b.error("unknown_furnace", format!("work order {:?} runs in furnace {furnace:?}, which is not in the dataset", order.id));
                        None
                    }
                    (_, None) => {
                        b.error("invalid_start", format!("work order {:?} starts {start:?}, which is not a date-time YYYY-MM-DDTHH:MM", order.id));
                        None
                    }
                    (Some(_), Some(m)) if m < 0 || m >= horizon || m % step != 0 => {
                        b.error("invalid_start", format!("work order {:?} starts {start:?}; starts lie inside the schedule on its {step}-minute grid", order.id));
                        None
                    }
                    (Some(f), Some(m)) => Some(f * d.slots + (m / step) as usize),
                }
            }
            _ => {
                b.error(
                    "half_placed",
                    format!(
                        "work order {:?} names a furnace or a start but not both",
                        order.id
                    ),
                );
                None
            }
        };
        let mut ops = [None; 4];
        for (n, task) in Task::ALL.into_iter().enumerate() {
            if let Some(id) = order.op_wire(task) {
                match operator_ids.get(id) {
                    Some(&op) => ops[n] = Some(op),
                    None => b.error("unknown_operator", format!("work order {:?} gives its {} to operator {id:?}, who is not in the dataset", order.id, task.label())),
                }
            }
        }
        placements.push((placement, ops));
        d.orders.push(OrderInfo {
            recipe: recipe.unwrap_or(0),
            load_kg: order.load_kg,
            release,
            due: due.unwrap_or(0),
            priority: order.priority,
        });
    }
    if plan.work_orders.is_empty() {
        b.warning("no_orders", "the dataset has no work orders".into());
    }
    if b.has_errors() {
        return b.0;
    }

    // Warnings that need the whole dataset.
    for (index, order) in plan.work_orders.iter().enumerate() {
        let info = &d.orders[index];
        let recipe = &d.recipes[info.recipe];
        let fits = d.furnaces.iter().any(|f| {
            f.processes & recipe.process.bit() != 0
                && recipe.temperature <= f.max_temperature
                && info.load_kg <= f.max_load_kg
        });
        if !fits {
            b.warning(
                "no_furnace",
                format!(
                    "no furnace can run work order {:?}: {} at {} °C, {} kg",
                    order.id,
                    recipe.process.label(),
                    recipe.temperature,
                    info.load_kg
                ),
            );
        }
        if info
            .release
            .is_some_and(|release| release + recipe.cycle > info.due)
        {
            b.warning("due_before_release", format!("work order {:?} cannot finish by its due time even if it starts the moment it is released", order.id));
        }
    }

    let derived = Arc::new(d);
    let ctx = Ctx(derived.clone());
    for row in &mut plan.roster {
        row.ctx = ctx.clone();
        row.allowed = allowed_shifts(&derived, row.index);
        row.shift_value = row.shift.map(ShiftKind::value);
    }
    for (order, (placement, ops)) in plan.work_orders.iter_mut().zip(placements) {
        order.ctx = ctx.clone();
        order.candidates = Values::new(order_candidates(&derived, order.index));
        for task in Task::ALL {
            let values = task_candidates(&derived, order.index, task);
            match task {
                Task::LoadBuild => order.load_build_candidates = values,
                Task::Program => order.program_candidates = values,
                Task::Quench => order.quench_candidates = values,
                Task::Unload => order.unload_candidates = values,
            }
        }
        order.slot = placement;
        for (n, task) in Task::ALL.into_iter().enumerate() {
            *order.op_mut(task) = ops[n];
        }
    }
    b.0
}

/// Schedule start and length in days.
fn horizon(plan: &Plan, b: &mut Issues) -> Option<(i64, i64)> {
    let (Some(start), Some(end)) = (
        parse_date_time(&plan.schedule.start),
        parse_date_time(&plan.schedule.end),
    ) else {
        b.error(
            "invalid_schedule",
            format!(
                "the schedule runs from {:?} to {:?}; both must be YYYY-MM-DDTHH:MM",
                plan.schedule.start, plan.schedule.end
            ),
        );
        return None;
    };
    if start % DAY != 0 || end % DAY != 0 {
        b.error(
            "invalid_schedule",
            "the schedule starts and ends at midnight".into(),
        );
        return None;
    }
    let days = (end - start) / DAY;
    if !(1..=MAX_DAYS).contains(&days) {
        b.error(
            "invalid_schedule",
            format!("the schedule lasts {days} days; it must be 1 to {MAX_DAYS}"),
        );
        return None;
    }
    Some((start, days))
}

/// Resolves and checks the parameters.
fn settings(p: &Parameters, b: &mut Issues) -> Option<(Settings, i64)> {
    for key in p.unknown.keys() {
        b.warning(
            "unknown_parameter",
            format!("parameter {key:?} is not a setting of this model and is ignored"),
        );
    }
    let mut ok = true;
    let mut check = |good: bool, name: &str, rule: &str| {
        if !good {
            b.error(
                "invalid_parameter",
                format!("parameter {name} must be {rule}"),
            );
            ok = false;
        }
    };
    check(
        (1..=60).contains(&p.slot_minutes) && DAY % p.slot_minutes == 0,
        "slotMinutes",
        "1 to 60 and divide a day",
    );
    check(
        (0..DAY).contains(&p.shift_start),
        "shiftStart",
        "a minute of the day, 0 to 1439",
    );
    for (value, name) in [
        (p.load_build_minutes, "loadBuildMinutes"),
        (p.program_minutes, "programMinutes"),
        (p.quench_minutes, "quenchMinutes"),
        (p.unload_minutes, "unloadMinutes"),
    ] {
        check((1..=240).contains(&value), name, "1 to 240 minutes");
    }
    check(
        (0..=240).contains(&p.charge_minutes),
        "chargeMinutes",
        "0 to 240 minutes",
    );
    check(p.heavy_load_kg >= 0, "heavyLoadKg", "zero or more");
    check(
        (0..=600).contains(&p.purge_minutes),
        "purgeMinutes",
        "0 to 600 minutes",
    );
    check(
        (0..=2 * DAY).contains(&p.minimum_rest),
        "minimumRest",
        "0 to 2880 minutes",
    );
    check(
        (0..=7).contains(&p.max_shifts_per_week),
        "maxShiftsPerWeek",
        "0 to 7",
    );
    check(
        (0..=7).contains(&p.target_shifts_per_week),
        "targetShiftsPerWeek",
        "0 to 7",
    );
    check(
        (1..=7).contains(&p.max_consecutive_nights),
        "maxConsecutiveNights",
        "1 to 7",
    );
    check(
        (0..=DAY).contains(&p.peak_start) && (0..=DAY).contains(&p.peak_end),
        "peakStart and peakEnd",
        "minutes of the day, 0 to 1440",
    );
    check(
        p.peak_price >= 0.0 && p.off_peak_price >= 0.0,
        "peakPrice and offPeakPrice",
        "zero or more",
    );
    ok.then_some((
        Settings {
            shift_start: p.shift_start,
            load_build: p.load_build_minutes,
            charge: p.charge_minutes,
            program: p.program_minutes,
            quench: p.quench_minutes,
            unload: p.unload_minutes,
            heavy_load_kg: p.heavy_load_kg,
            purge: p.purge_minutes,
            minimum_rest: p.minimum_rest,
            max_shifts_per_week: p.max_shifts_per_week,
            target_shifts_per_week: p.target_shifts_per_week,
            max_consecutive_nights: p.max_consecutive_nights,
            peak_start: p.peak_start,
            peak_end: p.peak_end,
            peak_price: p.peak_price,
            off_peak_price: p.off_peak_price,
            weekend_off_peak: p.weekend_off_peak,
        },
        p.slot_minutes,
    ))
}

/// A roster row may not claim a shift its day does not have.
fn row_shifts_invalid(plan: &Plan, b: &mut Issues) -> bool {
    let mut bad = false;
    for row in &plan.roster {
        if row.day_index_unchecked(plan) == Some(-1)
            && matches!(row.shift, Some(ShiftKind::Morning | ShiftKind::Afternoon))
        {
            b.error("invalid_roster_shift", format!("roster row {:?} is on the day before the schedule, which only has its night shift", row.id));
            bad = true;
        }
    }
    bad
}

impl RosterDay {
    fn day_index_unchecked(&self, plan: &Plan) -> Option<i64> {
        let origin = parse_date_time(&plan.schedule.start)? / DAY;
        parse_date(&self.day).map(|day| day - origin)
    }
}

/// The shift values a roster row may take: off, plus the shifts of its day
/// the operator can work. An unavailable operator can only be off.
fn allowed_shifts(d: &Derived, row: usize) -> Values {
    let info = &d.roster[row];
    let op = &d.operators[info.operator];
    let mut values = vec![ShiftKind::Off.value()];
    if op.unavailable.contains(&info.day) {
        return Values::new(values);
    }
    if info.day >= 0 {
        values.push(ShiftKind::Morning.value());
        values.push(ShiftKind::Afternoon.value());
    }
    if !op.day_only && op.role.works_nights() {
        values.push(ShiftKind::Night.value());
    }
    Values::new(values)
}
