//! Facts `prepare` derives from a dataset: ids resolved to indexes, times to
//! minutes from the schedule start, recipes resolved per order. Shared
//! read-only by every entity of one plan.

use crate::wire::{Priority, Process, Role, ShiftKind};

/// Minutes in one shift.
pub const SHIFT: i64 = 480;
/// Minutes in one monitoring bucket.
pub const BUCKET: i64 = 15;
/// Ambient temperature a cold furnace starts from, °C.
pub const AMBIENT: i64 = 20;

/// Settings resolved from the dataset's parameters.
#[derive(Debug, Clone, Default)]
pub struct Settings {
    pub shift_start: i64,
    pub load_build: i64,
    pub charge: i64,
    pub program: i64,
    pub quench: i64,
    pub unload: i64,
    pub heavy_load_kg: i64,
    pub purge: i64,
    pub minimum_rest: i64,
    pub max_shifts_per_week: i64,
    pub target_shifts_per_week: i64,
    pub max_consecutive_nights: i64,
    pub peak_start: i64,
    pub peak_end: i64,
    pub peak_price: f64,
    pub off_peak_price: f64,
    pub weekend_off_peak: bool,
}

#[derive(Debug, Clone, Default)]
pub struct FurnaceInfo {
    pub processes: u16,
    pub max_temperature: i64,
    pub max_load_kg: i64,
    pub heating_rate: i64,
    pub cooling_rate: i64,
    pub power_kw: i64,
    pub initial_temperature: Option<i64>,
    /// Downtime windows, minutes from the schedule start, sorted.
    pub downtime: Vec<(i64, i64)>,
}

#[derive(Debug, Clone)]
pub struct RecipeInfo {
    pub process: Process,
    pub temperature: i64,
    pub cycle: i64,
    pub quench: bool,
}

impl RecipeInfo {
    /// Minutes at the start of the cycle the furnace ramps and needs closer watch.
    pub fn ramp(&self) -> i64 {
        self.cycle.clamp(15, 60).min(self.cycle - 15).max(0)
    }
}

#[derive(Debug, Clone)]
pub struct OrderInfo {
    pub recipe: usize,
    pub load_kg: i64,
    pub release: Option<i64>,
    pub due: i64,
    pub priority: Priority,
}

#[derive(Debug, Clone)]
pub struct OperatorInfo {
    pub skills: u16,
    pub role: Role,
    pub day_only: bool,
    /// Roster days the operator cannot work.
    pub unavailable: Vec<i64>,
}

#[derive(Debug, Clone)]
pub struct RosterInfo {
    pub operator: usize,
    /// Roster day: -1 is the evening before the schedule starts.
    pub day: i64,
}

/// A shift of one roster day, numbered from the carry-in night.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShiftId {
    pub day: i64,
    pub kind: ShiftKind,
}

impl ShiftId {
    /// Dense number: `(day + 1) * 3 + kind`.
    pub fn number(self) -> usize {
        ((self.day + 1) * 3) as usize + self.kind.value()
    }
}

/// A manual task around a furnace load.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Task {
    LoadBuild,
    Program,
    Quench,
    Unload,
}

impl Task {
    pub const ALL: [Task; 4] = [Task::LoadBuild, Task::Program, Task::Quench, Task::Unload];

    pub const fn label(self) -> &'static str {
        match self {
            Task::LoadBuild => "load building",
            Task::Program => "programming",
            Task::Quench => "quench",
            Task::Unload => "unloading",
        }
    }

    /// The wire key of the task, also used in fill item ids.
    pub const fn key(self) -> &'static str {
        match self {
            Task::LoadBuild => "loadBuild",
            Task::Program => "program",
            Task::Quench => "quench",
            Task::Unload => "unload",
        }
    }

    /// Whether one crew does the task once for every order of a shared load.
    pub const fn per_load(self) -> bool {
        !matches!(self, Task::LoadBuild)
    }
}

/// Everything derived from one dataset.
#[derive(Debug, Default)]
pub struct Derived {
    /// Minutes since 1970-01-01 of the schedule start (a midnight).
    pub origin: i64,
    pub days: i64,
    pub horizon: i64,
    pub step: i64,
    pub slots: usize,
    pub s: Settings,
    pub furnaces: Vec<FurnaceInfo>,
    pub recipes: Vec<RecipeInfo>,
    pub orders: Vec<OrderInfo>,
    pub operators: Vec<OperatorInfo>,
    pub roster: Vec<RosterInfo>,
}

/// One order placed on a furnace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placed {
    pub furnace: usize,
    pub start: i64,
    pub end: i64,
}

impl Derived {
    pub fn recipe_of(&self, order: usize) -> &RecipeInfo {
        &self.recipes[self.orders[order].recipe]
    }

    /// The slot value of `furnace` starting at slot `slot`.
    pub fn value(&self, furnace: usize, slot: usize) -> usize {
        furnace * self.slots + slot
    }

    /// Where and when an order runs with slot value `value`.
    pub fn place(&self, order: usize, value: usize) -> Placed {
        let start = (value % self.slots) as i64 * self.step;
        Placed {
            furnace: value / self.slots,
            start,
            end: start + self.recipe_of(order).cycle,
        }
    }

    /// Start and end of a shift.
    pub fn shift_bounds(&self, shift: ShiftId) -> (i64, i64) {
        let start =
            shift.day * crate::time::DAY + self.s.shift_start + SHIFT * shift.kind.value() as i64;
        (start, start + SHIFT)
    }

    /// The rostered shift a minute belongs to, if the roster has one.
    pub fn owner(&self, minute: i64) -> Option<ShiftId> {
        let rel = minute - self.s.shift_start;
        let day = rel.div_euclid(crate::time::DAY);
        let kind = ShiftKind::from_value((rel.rem_euclid(crate::time::DAY) / SHIFT) as usize)?;
        let valid = (day == -1 && kind == ShiftKind::Night) || (0..self.days).contains(&day);
        valid.then_some(ShiftId { day, kind })
    }

    /// The one shift that owns all of `[start, end)`, if there is one.
    pub fn owner_of(&self, (start, end): (i64, i64)) -> Option<ShiftId> {
        let first = self.owner(start)?;
        (self.owner(end - 1)? == first).then_some(first)
    }

    /// Number of shift ids, carry-in night included.
    pub fn shift_count(&self) -> usize {
        ((self.days + 1) * 3) as usize
    }

    /// The window of a task for an order running `[start, end)`, if the
    /// order has the task.
    pub fn task_window(
        &self,
        order: usize,
        task: Task,
        start: i64,
        end: i64,
    ) -> Option<(i64, i64)> {
        let s = &self.s;
        match task {
            Task::LoadBuild => {
                let to = start - s.program - s.charge;
                Some((to - s.load_build, to))
            }
            Task::Program => Some((start - s.program, start)),
            Task::Quench => self
                .recipe_of(order)
                .quench
                .then_some((end, end + s.quench)),
            Task::Unload => Some((end + s.quench, end + s.quench + s.unload)),
        }
    }

    /// Skills a task needs, as a mask.
    pub fn task_skills(&self, order: usize, task: Task) -> u16 {
        use crate::wire::Skill::*;
        match task {
            Task::LoadBuild => LoadBuild.bit(),
            Task::Program => FurnaceProgram.bit(),
            Task::Quench => QuenchOperate.bit(),
            Task::Unload if self.orders[order].load_kg > self.s.heavy_load_kg => {
                LoadBuild.bit() | CraneForklift.bit()
            }
            Task::Unload => LoadBuild.bit(),
        }
    }

    /// Minutes a furnace needs to go from one load to the next.
    pub fn changeover(&self, furnace: usize, from: (i64, Option<Process>), to: &RecipeInfo) -> i64 {
        let f = &self.furnaces[furnace];
        let family = from
            .1
            .map_or(0, |process| Process::family_minutes(process, to.process));
        let delta = to.temperature - from.0;
        if delta == 0 && family == 0 {
            return 0;
        }
        let ramp = if delta > 0 {
            div_ceil(delta, f.heating_rate.max(1))
        } else {
            div_ceil(-delta, f.cooling_rate.max(1))
        };
        ramp + self.s.purge + family
    }

    /// Whether a minute (from the schedule start) is billed at the peak price.
    pub fn is_peak(&self, minute: i64) -> bool {
        let day = self.origin.div_euclid(crate::time::DAY) + minute.div_euclid(crate::time::DAY);
        if self.s.weekend_off_peak && crate::time::weekday(day) >= 5 {
            return false;
        }
        let of_day = minute.rem_euclid(crate::time::DAY);
        let (from, to) = (self.s.peak_start, self.s.peak_end);
        if from <= to {
            (from..to).contains(&of_day)
        } else {
            of_day >= from || of_day < to
        }
    }

    /// Peak minutes in `[start, end)`.
    pub fn peak_minutes(&self, start: i64, end: i64) -> i64 {
        let mut minutes = 0;
        let mut at = start;
        while at < end {
            let day_end = (at.div_euclid(crate::time::DAY) + 1) * crate::time::DAY;
            let stop = day_end.min(end);
            if self.s.weekend_off_peak
                && crate::time::weekday(
                    self.origin.div_euclid(crate::time::DAY) + at.div_euclid(crate::time::DAY),
                ) >= 5
            {
                at = stop;
                continue;
            }
            let base = at.div_euclid(crate::time::DAY) * crate::time::DAY;
            let (from, to) = (self.s.peak_start, self.s.peak_end);
            let windows: &[(i64, i64)] = if from <= to {
                &[(from, to)]
            } else {
                &[(0, to), (from, crate::time::DAY)]
            };
            for &(a, b) in windows {
                minutes += crate::time::overlap(at, stop, base + a, base + b);
            }
            at = stop;
        }
        minutes
    }

    /// Energy cost of running a furnace over `[start, end)`, in cents.
    pub fn energy_cents(&self, furnace: usize, start: i64, end: i64) -> i64 {
        let kw = self.furnaces[furnace].power_kw as f64;
        let peak = self.peak_minutes(start, end) as f64;
        let off = (end - start) as f64 - peak;
        (kw * (peak * self.s.peak_price + off * self.s.off_peak_price) / 60.0).round() as i64
    }
}

pub fn div_ceil(a: i64, b: i64) -> i64 {
    (a + b - 1).div_euclid(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn derived() -> Derived {
        Derived {
            origin: crate::time::days_from_civil(2027, 3, 1) * crate::time::DAY,
            days: 7,
            horizon: 7 * crate::time::DAY,
            step: 15,
            slots: 672,
            s: Settings {
                shift_start: 360,
                peak_start: 480,
                peak_end: 1200,
                peak_price: 20.0,
                off_peak_price: 10.0,
                weekend_off_peak: true,
                purge: 20,
                ..Settings::default()
            },
            furnaces: vec![FurnaceInfo {
                heating_rate: 10,
                cooling_rate: 4,
                power_kw: 60,
                ..FurnaceInfo::default()
            }],
            ..Derived::default()
        }
    }

    #[test]
    fn minutes_belong_to_their_shift() {
        let d = derived();
        let night = |day| ShiftId {
            day,
            kind: ShiftKind::Night,
        };
        assert_eq!(d.owner(15), Some(night(-1)));
        assert_eq!(d.owner(1440 + 75), Some(night(0)));
        assert_eq!(d.owner(7 * 1440 + 15), Some(night(6)));
        assert_eq!(d.owner(7 * 1440 + 400), None);
        assert_eq!(d.owner(-200), None);
        assert_eq!(
            d.owner_of((830, 845)),
            None,
            "a task across the 14:00 change has no single owner"
        );
        assert_eq!(night(-1).number(), 2);
    }

    #[test]
    fn heating_is_faster_than_cooling() {
        let d = derived();
        let hot = RecipeInfo {
            process: Process::Quenching,
            temperature: 900,
            cycle: 60,
            quench: true,
        };
        let cool = RecipeInfo {
            process: Process::Quenching,
            temperature: 200,
            cycle: 60,
            quench: true,
        };
        assert_eq!(d.changeover(0, (900, Some(Process::Quenching)), &hot), 0);
        assert_eq!(
            d.changeover(0, (200, Some(Process::Quenching)), &hot),
            70 + 20
        );
        assert_eq!(
            d.changeover(0, (900, Some(Process::Quenching)), &cool),
            175 + 20
        );
    }

    #[test]
    fn the_tariff_bills_weekday_daytime_at_peak() {
        let d = derived();
        // Monday 06:00–10:00: two peak hours.
        assert_eq!(d.peak_minutes(360, 600), 120);
        // Saturday is off peak.
        assert_eq!(d.peak_minutes(5 * 1440 + 480, 5 * 1440 + 600), 0);
        // 60 kW for 4 h, 2 at 20 c and 2 at 10 c.
        assert_eq!(d.energy_cents(0, 360, 600), 60 * 20 * 2 + 60 * 10 * 2);
        assert!(d.is_peak(600) && !d.is_peak(1300));
    }
}
