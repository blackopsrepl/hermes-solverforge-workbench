//! Score analysis of one plan: per-constraint totals plus individual matches.

use serde::Serialize;
use solverforge::{PlanningSolution, ScoreDirector};

use crate::model::{ConstraintInfo, EntityName, Level, Model};

/// Most characters kept from one entity's display text.
const MAX_ENTITY_TEXT: usize = 240;

#[derive(Debug, Serialize)]
pub struct Analysis {
    pub score: String,
    pub feasible: bool,
    pub constraints: Vec<ConstraintReport>,
}

#[derive(Debug, Serialize)]
pub struct ConstraintReport {
    pub name: String,
    pub level: Level,
    pub weight: u32,
    pub score: String,
    pub match_count: usize,
    pub matches: Vec<MatchReport>,
}

#[derive(Debug, Serialize)]
pub struct MatchReport {
    pub score: String,
    pub entities: Vec<EntityReport>,
}

#[derive(Debug, Serialize)]
pub struct EntityReport {
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(flatten)]
    pub name: EntityName,
}

/// Scores `plan` under its own constraint weights.
pub fn analyze<M: Model>(mut plan: M::Plan, max_matches: usize) -> Analysis {
    plan.update_all_shadows();
    let catalog = M::catalog();
    let mut director = ScoreDirector::new_weighted(plan, M::constraints());
    let score = director.calculate_score();
    let analyses = director.constraint_analyses();
    let plan = director.working_solution();
    let constraints = analyses
        .into_iter()
        .map(|analysis| {
            let name = analysis.constraint_ref.full_name();
            let score = analysis.score.to_string();
            let level = level_of(&catalog, &name, analysis.is_hard, &score);
            let match_count = analysis.matches.len();
            let matches = analysis
                .matches
                .into_iter()
                .take(max_matches)
                .map(|found| MatchReport {
                    score: found.score.to_string(),
                    entities: found
                        .justification
                        .entities
                        .iter()
                        .map(|entity| {
                            let type_name = short_type(&entity.type_name);
                            EntityReport {
                                type_name: type_name.to_string(),
                                name: entity
                                    .planning_id
                                    .as_deref()
                                    .and_then(|id| M::name_entity(plan, type_name, id))
                                    .unwrap_or_else(|| EntityName {
                                        id: None,
                                        label: clip(&entity.display),
                                    }),
                            }
                        })
                        .collect(),
                })
                .collect();
            ConstraintReport {
                name,
                level,
                weight: analysis.weight_multiplier,
                score,
                match_count,
                matches,
            }
        })
        .collect();
    let score_text = score.to_string();
    Analysis {
        feasible: is_feasible(&score_text),
        score: score_text,
        constraints,
    }
}

/// A plan is feasible when its hard level is not negative.
pub fn is_feasible(score: &str) -> bool {
    level_value(score, "hard").is_none_or(|hard| hard >= 0.0)
}

/// The catalog's level when the model declares one; otherwise hard when the
/// constraint is hard, medium when it moves the medium level, else soft.
pub fn level_of(catalog: &[ConstraintInfo], name: &str, is_hard: bool, score: &str) -> Level {
    if let Some(info) = catalog.iter().find(|info| info.name == name) {
        return info.level;
    }
    if is_hard {
        Level::Hard
    } else if level_value(score, "medium").is_some_and(|medium| medium != 0.0) {
        Level::Medium
    } else {
        Level::Soft
    }
}

/// Reads one level from a score such as `-2hard/0medium/-17soft`.
fn level_value(score: &str, level: &str) -> Option<f64> {
    score
        .split('/')
        .find_map(|part| part.strip_suffix(level))
        .and_then(|number| number.parse().ok())
}

fn short_type(path: &str) -> &str {
    path.rsplit("::").next().unwrap_or(path)
}

fn clip(text: &str) -> String {
    if text.chars().count() <= MAX_ENTITY_TEXT {
        return text.to_string();
    }
    let mut clipped: String = text.chars().take(MAX_ENTITY_TEXT).collect();
    clipped.push('…');
    clipped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feasibility_reads_the_hard_level() {
        assert!(is_feasible("0hard/-5soft"));
        assert!(!is_feasible("-1hard/0soft"));
        assert!(!is_feasible("-2hard/0medium/-17soft"));
        assert!(is_feasible("-3soft"));
    }

    #[test]
    fn medium_constraints_are_recognised_from_their_score() {
        assert_eq!(
            level_of(&[], "x", false, "0hard/-4medium/0soft"),
            Level::Medium
        );
        assert_eq!(
            level_of(&[], "x", false, "0hard/0medium/-4soft"),
            Level::Soft
        );
        assert_eq!(level_of(&[], "x", true, "-1hard/0soft"), Level::Hard);
    }

    #[test]
    fn the_catalog_level_wins() {
        let catalog = [ConstraintInfo {
            name: "x",
            group: "g",
            level: Level::Medium,
            description: "d",
        }];
        assert_eq!(level_of(&catalog, "x", false, "0hard/0soft"), Level::Medium);
    }
}
