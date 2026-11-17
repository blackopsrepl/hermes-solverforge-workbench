//! Plan views: a plan drawn for people, as a timeline, as routes, or as
//! boards of assignments.
//!
//! The engine builds the view because the model knows its domain; the
//! plugin only draws it. Both kinds reuse the timeline model of the
//! solverforge-ui rail timeline (`axis` plus `lanes`), passed through as JSON
//! so a model can use every feature the timeline has.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use crate::analysis::Analysis;
use crate::model::Level;

mod assignments;

pub use assignments::assignments;

/// A plan view.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum View {
    /// One lane per resource (employee, machine, room, crew member…).
    Timeline(Timeline),
    /// Vehicles driving through stops on a map, with a per-vehicle timeline.
    Routes(Routes),
    /// Items grouped by what they are assigned to, for models with no time
    /// or place to draw: one board per planning variable.
    Assignments(Assignments),
}

/// Boards of items grouped by what they are assigned to.
#[derive(Debug, Clone, Serialize)]
pub struct Assignments {
    pub title: String,
    pub subtitle: String,
    pub boards: Vec<Board>,
}

/// One planning variable: a lane per value or list owner, holding its items
/// (in list order for list variables), and the items no lane holds.
#[derive(Debug, Clone, Serialize)]
pub struct Board {
    pub title: String,
    /// Type of the items (`Task`).
    pub item: String,
    /// Type of the lanes (`Resource`).
    pub lane: String,
    pub lanes: Vec<Lane>,
    pub unassigned: Vec<Item>,
}

/// A value or list owner and the items it holds.
#[derive(Debug, Clone, Serialize)]
pub struct Lane {
    pub id: String,
    pub label: String,
    pub items: Vec<Item>,
    /// Rules the lane's own record breaks, worst first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub broken: Vec<Broken>,
}

/// A planning entity or list element on a board.
#[derive(Debug, Clone, Serialize)]
pub struct Item {
    pub id: String,
    pub label: String,
    /// Rules this item breaks, worst first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub broken: Vec<Broken>,
}

/// A solverforge-ui rail timeline: `model` holds `axis` and `lanes`.
#[derive(Debug, Clone, Serialize)]
pub struct Timeline {
    pub title: String,
    /// One line under the title; say what the colours mean here.
    pub subtitle: String,
    /// Heading of the lane-label column, for example `Employee`.
    pub label: String,
    pub model: Value,
}

/// Routes on a map.
#[derive(Debug, Clone, Serialize)]
pub struct Routes {
    pub title: String,
    pub subtitle: String,
    pub vehicles: Vec<Route>,
    /// Stops no vehicle serves.
    pub unassigned: Vec<Stop>,
    /// The same plan as a timeline, one lane per vehicle.
    pub timeline: Timeline,
}

/// One vehicle's route, from its start through its stops to its end.
#[derive(Debug, Clone, Serialize)]
pub struct Route {
    pub id: String,
    pub label: String,
    /// CSS colour of the route on the map and the timeline.
    pub color: String,
    pub start: Point,
    pub end: Point,
    pub stops: Vec<Stop>,
    /// Road geometry as `[lat, lon]` points, when the model has it; straight
    /// lines between stops otherwise.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<[f64; 2]>,
    /// Short facts shown with the route, for example `6 stops · 41 km`.
    pub stats: Vec<String>,
}

/// A named place.
#[derive(Debug, Clone, Serialize)]
pub struct Point {
    pub label: String,
    pub lat: f64,
    pub lon: f64,
}

/// A stop on a route, or one left unassigned.
#[derive(Debug, Clone, Serialize)]
pub struct Stop {
    /// The record's id in the model's wire format.
    pub id: String,
    pub label: String,
    pub lat: f64,
    pub lon: f64,
    /// Local date-times, when the plan has them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arrival: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub departure: Option<String>,
    /// Rules this stop breaks, worst first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub broken: Vec<Broken>,
}

/// A rule a plan item breaks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Broken {
    pub level: Level,
    pub constraint: String,
    /// How much this rule costs in total across its matches naming the item,
    /// at its level, as a positive number.
    pub cost: i64,
    /// The match named other entities too: a rule over a group (an
    /// employee's roster, a vehicle's route) rather than this item alone.
    pub shared: bool,
}

/// Every entity a hard or medium match blames, keyed by `(type, id)`.
///
/// Entities without a wire id are left out. Soft matches are not breaks.
pub fn blame(analysis: &Analysis) -> BTreeMap<(String, String), Vec<Broken>> {
    let mut out: BTreeMap<(String, String), Vec<Broken>> = BTreeMap::new();
    for constraint in &analysis.constraints {
        if constraint.level == Level::Soft {
            continue;
        }
        for found in &constraint.matches {
            let cost = level_cost(&found.score, constraint.level);
            if cost <= 0 {
                continue;
            }
            let shared = found.entities.len() > 1;
            for entity in &found.entities {
                let Some(id) = entity.name.id.clone() else {
                    continue;
                };
                let list = out.entry((entity.type_name.clone(), id)).or_default();
                match list.iter_mut().find(|b| b.constraint == constraint.name) {
                    Some(existing) => {
                        existing.cost += cost;
                        existing.shared |= shared;
                    }
                    None => list.push(Broken {
                        level: constraint.level,
                        constraint: constraint.name.clone(),
                        cost,
                        shared,
                    }),
                }
            }
        }
    }
    for list in out.values_mut() {
        list.sort_by(|a, b| {
            (a.level != Level::Hard, -a.cost, &a.constraint).cmp(&(
                b.level != Level::Hard,
                -b.cost,
                &b.constraint,
            ))
        });
    }
    out
}

/// The cost of a match at `level`, as a positive number, from a score such
/// as `-480hard/0medium/0soft`.
pub fn level_cost(score: &str, level: Level) -> i64 {
    let suffix = match level {
        Level::Hard => "hard",
        Level::Medium => "medium",
        Level::Soft => "soft",
    };
    score
        .split('/')
        .find_map(|part| part.strip_suffix(suffix))
        .and_then(|number| number.parse::<i64>().ok())
        .map_or(0, |value| -value)
}

/// Tones of the timeline for items that break a rule.
pub const HARD_TONE: &str = "red";
pub const MEDIUM_TONE: &str = "rose";

/// The timeline tone for an item's breaks, if it has any that are its own.
pub fn break_tone(broken: &[Broken]) -> Option<&'static str> {
    let own: Vec<&Broken> = broken.iter().filter(|b| !b.shared).collect();
    if own.iter().any(|b| b.level == Level::Hard) {
        Some(HARD_TONE)
    } else if own.iter().any(|b| b.level == Level::Medium) {
        Some(MEDIUM_TONE)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_match_costs_its_own_level() {
        assert_eq!(level_cost("-480hard/0medium/-3soft", Level::Hard), 480);
        assert_eq!(level_cost("-480hard/0medium/-3soft", Level::Soft), 3);
        assert_eq!(level_cost("0hard/0soft", Level::Medium), 0);
    }

    #[test]
    fn only_an_items_own_breaks_colour_it() {
        let own = Broken {
            level: Level::Medium,
            constraint: "Open".into(),
            cost: 1,
            shared: false,
        };
        let group = Broken {
            level: Level::Hard,
            constraint: "Overlap".into(),
            cost: 480,
            shared: true,
        };
        assert_eq!(break_tone(std::slice::from_ref(&group)), None);
        assert_eq!(break_tone(&[group, own]), Some(MEDIUM_TONE));
    }
}
