//! Lays each shift's itinerary into its list, and pins what the dataset
//! pins.

use std::collections::HashMap;
use std::sync::Arc;

use solverforge_workbench_host::Issue;

use crate::derived::Kind;
use crate::domain::Plan;
use crate::time::parse_date_time;
use crate::wire::ItemKind;

pub fn place(plan: &mut Plan, issues: &mut Vec<Issue>) {
    let by_id: HashMap<String, usize> = plan
        .facts
        .visits
        .iter()
        .enumerate()
        .map(|(i, v)| (v.id.clone(), i))
        .collect();
    let mut placed = vec![false; plan.visits.len()];
    let mut pins = Vec::new();
    let mut lists = Vec::new();
    let mut pinned_shifts = Vec::new();
    for shift in plan.drivers.iter().flat_map(|d| &d.shifts) {
        let s = lists.len();
        let mut list = Vec::new();
        let mut last_pin = i64::MIN;
        for item in &shift.itinerary {
            let what = format!("shift {:?} itinerary", shift.id);
            let Some(&v) = by_id.get(&item.id) else {
                issues.push(Issue::error(
                    "unknown_itinerary_item",
                    format!("{what}: no stop or break {:?}", item.id),
                ));
                continue;
            };
            match (&plan.facts.visits[v].kind, item.kind) {
                (Kind::Stop { .. }, ItemKind::Stop) => {}
                (Kind::Break(b), ItemKind::Break) if b.shift == s => {}
                (Kind::Break(_), ItemKind::Break) => {
                    issues.push(Issue::error(
                        "foreign_break",
                        format!("{what}: break {:?} belongs to another shift", item.id),
                    ));
                    continue;
                }
                _ => {
                    issues.push(Issue::error(
                        "wrong_item_kind",
                        format!("{what}: {:?} is not a {:?}", item.id, item.kind),
                    ));
                    continue;
                }
            }
            if std::mem::replace(&mut placed[v], true) {
                issues.push(Issue::error(
                    "assigned_twice",
                    format!("{what}: {:?} is already in an itinerary", item.id),
                ));
                continue;
            }
            if item.pin {
                match item.start_service_time.as_deref().and_then(parse_date_time) {
                    Some(start) if item.kind == ItemKind::Stop => {
                        if start < last_pin {
                            issues.push(Issue::warning(
                                "pinned_stops_out_of_order",
                                format!("{what}: pinned stop {:?} starts before the one pinned ahead of it", item.id),
                            ));
                        }
                        last_pin = start;
                        pins.push((v, s, start));
                    }
                    Some(_) => {}
                    None => issues.push(Issue::error(
                        "pinned_stop_without_start",
                        format!("{what}: pinned {:?} needs a startServiceTime", item.id),
                    )),
                }
            }
            list.push(v);
        }
        lists.push(list);
        pinned_shifts.push(shift.pinned);
    }
    // A pinned shift keeps its list, so its breaks must already be in it.
    for (v, fact) in plan.facts.visits.iter().enumerate() {
        if let Kind::Break(b) = &fact.kind {
            if pinned_shifts.get(b.shift).copied().unwrap_or(false) && !placed[v] {
                issues.push(Issue::error(
                    "pinned_shift_misses_break",
                    format!(
                        "shift {:?} is pinned but its itinerary leaves out break {:?}",
                        plan.facts.shifts[b.shift].id, fact.id
                    ),
                ));
            }
        }
    }
    if !pins.is_empty() {
        let facts = Arc::make_mut(&mut plan.facts);
        for &(v, s, start) in &pins {
            facts.visits[v].pin = Some((s, start));
        }
    }
    for (v, s, _) in pins {
        plan.visits[v].pin_shift = Some(s);
    }
    // A job the dataset plans only in part keeps its other stops on the
    // bench, so the broken job shows; jobs planned nowhere stay out of
    // every list until construction places them whole.
    let mut benched = Vec::new();
    for job in &plan.facts.jobs {
        if job.stops.iter().any(|&v| placed[v]) {
            benched.extend(job.stops.iter().copied().filter(|&v| !placed[v]));
        }
    }
    for (s, list) in lists.into_iter().enumerate() {
        plan.shifts[s].visits = list;
        plan.shifts[s].pinned = pinned_shifts[s];
    }
    let bench = plan.facts.bench();
    plan.shifts[bench].visits = benched;
}
