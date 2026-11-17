//! Lays each shift's itinerary into its list, pins what the dataset pins,
//! and keeps where each visit was for replanning.

use std::collections::HashMap;
use std::sync::Arc;

use solverforge_workbench_host::Issue;

use crate::derived::Kind;
use crate::domain::Plan;
use crate::time::parse_date_time;
use crate::wire::{ItemKind, Limits};

pub fn place(plan: &mut Plan, issues: &mut Vec<Issue>) {
    let by_id: HashMap<String, usize> = plan
        .facts
        .visits
        .iter()
        .enumerate()
        .map(|(i, v)| (v.id.clone(), i))
        .collect();
    let mut placed = vec![false; plan.elements.len()];
    let mut pins = Vec::new();
    let mut originals = Vec::new();
    let mut lists = Vec::new();
    let mut pinned_shifts = Vec::new();
    let regular = plan
        .vehicles
        .iter()
        .flat_map(|v| v.shifts.iter().map(|s| (&s.id, &s.limits)));
    let linked = plan
        .vehicles
        .iter()
        .flat_map(|v| v.linked_shifts.iter().map(|s| (&s.id, &s.limits)));
    let all: Vec<(&String, &Limits)> = regular.chain(linked).collect();
    for (id, limits) in all {
        let s = lists.len();
        let mut list = Vec::new();
        for item in &limits.itinerary {
            let what = format!("shift {id:?} itinerary");
            let Some(&v) = by_id.get(&item.id) else {
                issues.push(Issue::error(
                    "unknown_itinerary_item",
                    format!("{what}: no visit or break {:?}", item.id),
                ));
                continue;
            };
            match (&plan.facts.visits[v].kind, item.kind) {
                (Kind::Visit, ItemKind::Visit) => {}
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
            let start = item.start_service_time.as_deref().and_then(parse_date_time);
            if item.start_service_time.is_some() && start.is_none() {
                issues.push(Issue::error(
                    "bad_time",
                    format!("{what}: {:?} has a bad startServiceTime", item.id),
                ));
            }
            if item.kind == ItemKind::Visit {
                if item.pin {
                    match start {
                        Some(start) => pins.push((v, s, start)),
                        None => issues.push(Issue::error(
                            "pinned_visit_without_start",
                            format!("{what}: pinned {:?} needs a startServiceTime", item.id),
                        )),
                    }
                }
                originals.push((v, s, start.unwrap_or(i64::MIN)));
            }
            list.push(v);
        }
        lists.push(list);
        pinned_shifts.push(limits.pinned);
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
    {
        let facts = Arc::make_mut(&mut plan.facts);
        for &(v, s, start) in &pins {
            facts.visits[v].pin = Some((s, start));
        }
        for &(v, s, start) in &originals {
            facts.visits[v].original = Some((s, start));
        }
    }
    // A group the dataset plans only in part keeps its other visits on the
    // bench, and so does either side of a dependency whose other side is
    // planned, so the broken link shows; visits linked to nothing planned
    // stay out of every list until construction places them.
    let mut benched = Vec::new();
    for group in &plan.facts.groups {
        if group.visits.iter().any(|&v| placed[v]) {
            benched.extend(group.visits.iter().copied().filter(|&v| !placed[v]));
        }
    }
    for (v, fact) in plan.facts.visits.iter().enumerate() {
        for dep in &fact.dependencies {
            let p = dep.preceding;
            if placed[v] != placed[p] {
                benched.push(if placed[v] { p } else { v });
            }
        }
    }
    benched.sort_unstable();
    benched.dedup();
    for (s, list) in lists.into_iter().enumerate() {
        plan.shifts[s].visits = list;
        plan.shifts[s].pinned = pinned_shifts[s];
    }
    let bench = plan.facts.bench();
    plan.shifts[bench].visits = benched;
    crate::route::note_missing(plan);
}
