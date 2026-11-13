//! Static facts about one shift worked by one employee.

use super::{Derived, Need, Span};
use crate::time::overlap;

impl Derived {
    fn holds(&self, employee: usize, need: &Need, at: i64) -> bool {
        self.employees[employee].skills.iter().any(|grant| {
            grant.skill == need.skill
                && grant.level >= need.level
                && grant.from <= at
                && at < grant.to
        })
    }

    /// Required skills the employee lacks at the shift start; with "any skill"
    /// shifts, 1 when the employee holds none of them.
    pub fn required_skills_missing(&self, shift: usize, employee: usize) -> i64 {
        let info = &self.shifts[shift];
        let missing = info
            .required_skills
            .iter()
            .filter(|need| !self.holds(employee, need, info.start))
            .count() as i64;
        if info.any_required_skill && !info.required_skills.is_empty() {
            i64::from(missing == info.required_skills.len() as i64)
        } else {
            missing
        }
    }

    /// Preferred skills the employee lacks at the shift start.
    pub fn preferred_skills_missing(&self, shift: usize, employee: usize) -> i64 {
        let info = &self.shifts[shift];
        info.preferred_skills
            .iter()
            .filter(|need| !self.holds(employee, need, info.start))
            .count() as i64
    }

    /// Shift risk factors the employee must avoid.
    pub fn risks_hit(&self, shift: usize, employee: usize) -> i64 {
        let avoid = &self.employees[employee].avoid_risks;
        self.shifts[shift]
            .risks
            .iter()
            .filter(|risk| avoid.binary_search(risk).is_ok())
            .count() as i64
    }

    fn span_minutes(&self, spans: &[Span], shift: usize, weighted: bool) -> i64 {
        let info = &self.shifts[shift];
        spans
            .iter()
            .filter(|span| span.shifts.accepts(&info.tags))
            .map(|span| {
                overlap(span.start, span.end, info.start, info.end)
                    * if weighted { span.weight } else { 1 }
            })
            .sum()
    }

    /// Minutes of the shift inside the employee's unavailable times.
    pub fn unavailable_minutes(&self, shift: usize, employee: usize) -> i64 {
        self.span_minutes(&self.employees[employee].unavailable, shift, false)
    }

    /// Minutes of the shift outside the employee's available-only times, when
    /// any of them applies to this shift.
    pub fn outside_available_minutes(&self, shift: usize, employee: usize) -> i64 {
        let info = &self.shifts[shift];
        let mut spans: Vec<(i64, i64)> = self.employees[employee]
            .available_only
            .iter()
            .filter(|span| span.shifts.accepts(&info.tags))
            .map(|span| (span.start, span.end))
            .collect();
        if spans.is_empty() {
            return 0;
        }
        spans.sort_unstable();
        let mut covered = 0;
        let mut cursor = info.start;
        for (start, end) in spans {
            let (from, to) = (start.max(cursor), end.min(info.end));
            if to > from {
                covered += to - from;
                cursor = to;
            }
        }
        (info.end - info.start) - covered
    }

    fn listed(list: &[u32], employee: usize) -> i64 {
        i64::from(list.binary_search(&(employee as u32)).is_ok())
    }

    /// 1 when the shift lists the employee as preferred.
    pub fn preferred_employee(&self, shift: usize, employee: usize) -> i64 {
        Self::listed(&self.shifts[shift].preferred_employees, employee)
    }

    /// 1 when the shift lists the employee as unwanted.
    pub fn unwanted_employee(&self, shift: usize, employee: usize) -> i64 {
        Self::listed(&self.shifts[shift].unwanted_employees, employee)
    }

    /// 1 when the shift bars the employee.
    pub fn barred_employee(&self, shift: usize, employee: usize) -> i64 {
        Self::listed(&self.shifts[shift].barred_employees, employee)
    }

    /// 1 when the employee only works certain shifts and this is not one.
    pub fn outside_required_shifts(&self, shift: usize, employee: usize) -> i64 {
        let tags = &self.shifts[shift].tags;
        self.employees[employee]
            .required_shifts
            .as_ref()
            .map_or(0, |filter| i64::from(!filter.accepts(tags)))
    }

    /// 1 when the shift is one the employee likes.
    pub fn inside_preferred_shifts(&self, shift: usize, employee: usize) -> i64 {
        let tags = &self.shifts[shift].tags;
        self.employees[employee]
            .preferred_shifts
            .as_ref()
            .map_or(0, |filter| i64::from(filter.accepts(tags)))
    }

    /// Metres from the employee's home to the shift, when both are placed.
    pub fn travel_metres(&self, shift: usize, employee: usize) -> Option<f64> {
        let (lat, lon) = self.employees[employee].home?;
        let (to_lat, to_lon) = self.shifts[shift].location?;
        let from = crate::wire::Place { lat, lon };
        Some(from.metres_to(&crate::wire::Place {
            lat: to_lat,
            lon: to_lon,
        }))
    }

    /// Kilometres (rounded up) beyond each matching travel rule's maximum.
    pub fn travel_over_limit(&self, shift: usize, employee: usize) -> i64 {
        let Some(metres) = self.travel_metres(shift, employee) else {
            return 0;
        };
        let tags = &self.shifts[shift].tags;
        self.employees[employee]
            .travel
            .iter()
            .filter(|rule| rule.shifts.accepts(tags))
            .filter_map(|rule| rule.max_meters)
            .map(|max| ((metres - max as f64).max(0.0) / 1000.0).ceil() as i64)
            .sum()
    }

    /// Kilometres from home to the shift, rounded.
    pub fn travel_km(&self, shift: usize, employee: usize) -> i64 {
        self.travel_metres(shift, employee)
            .map_or(0, |metres| (metres / 1000.0).round() as i64)
    }

    /// Minutes of the shift inside preferred times, times each span's weight.
    pub fn preferred_minutes(&self, shift: usize, employee: usize) -> i64 {
        self.span_minutes(&self.employees[employee].preferred_times, shift, true)
    }

    /// Minutes of the shift inside unwanted times, times each span's weight.
    pub fn unwanted_minutes(&self, shift: usize, employee: usize) -> i64 {
        self.span_minutes(&self.employees[employee].unwanted_times, shift, true)
    }
}
