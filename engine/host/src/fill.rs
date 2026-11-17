//! `fill`: the best places for items a plan leaves open, ranked and explained.
//!
//! Each option is scored by the model itself: the item is placed, the plan is
//! rescored incrementally, and the change is undone. The options for one item
//! are independent of each other and of other items.
//!
//! [`scalar`] places entities whose planning variable is a single value (a
//! shift's employee, a lesson's room) for any model, through the planning
//! descriptors the macros generate. [`list`] inserts list elements (a
//! vehicle's visits) at every position of every owner, through the
//! [`ListVar`] accessors a model hands it. Models with richer list moves
//! override [`Model::fill`](crate::Model::fill) and use [`Ranking`] to score
//! their own candidate insertions the same way.

use std::any::Any;

use serde::{Deserialize, Serialize};
use solverforge::domain::{SolutionDescriptor, VariableType};
use solverforge::{ConstraintSet, Score, ScoreDirector};
use solverforge_scoring::WeightedMode;

use crate::model::{EntityName, Level, Model, ScoreOf};
use crate::protocol::EngineError;

mod lists;

pub use lists::{list, ListVar};

/// What to fill, and how many options to keep per item.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FillRequest {
    /// Item ids (as [`Model::name_entity`](crate::Model::name_entity) names
    /// them); empty means every item the plan leaves open.
    #[serde(default)]
    pub items: Vec<String>,
    /// Options kept per item, best first.
    #[serde(default = "default_top")]
    pub top: usize,
    /// Apply one option and answer with the resulting plan instead.
    #[serde(default)]
    pub apply: Option<FillChoice>,
}

fn default_top() -> usize {
    5
}

impl Default for FillRequest {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            top: default_top(),
            apply: None,
        }
    }
}

/// One option to apply: put `item` at `value`, both by id.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FillChoice {
    pub item: String,
    pub value: String,
}

/// The answer to a fill request.
pub enum FillAnswer<P> {
    /// Ranked options per item.
    Options(Vec<FillItem>),
    /// The plan with the chosen option applied.
    Applied(P),
}

/// One open item and its best placements.
#[derive(Debug, Clone, Serialize)]
pub struct FillItem {
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(flatten)]
    pub item: EntityName,
    pub options: Vec<FillOption>,
}

/// One placement and what it does to the score.
#[derive(Debug, Clone, Serialize)]
pub struct FillOption {
    /// Where the item goes: a value (an employee) or a position (after a stop).
    #[serde(flatten)]
    pub value: EntityName,
    /// The plan's score with the item placed.
    pub score: String,
    /// The score change the placement makes.
    pub delta: String,
    pub feasible: bool,
    /// Every constraint whose score the placement changes.
    pub changes: Vec<ConstraintChange>,
}

/// How one constraint's score changes with a placement.
#[derive(Debug, Clone, Serialize)]
pub struct ConstraintChange {
    pub constraint: String,
    pub level: Level,
    pub delta: String,
}

/// Scores candidate placements for one plan.
///
/// Build it with `Ranking::new(plan, M::constraints())`, then call
/// [`Ranking::rank`] once per item with closures that make and undo each
/// candidate's change through the director.
pub struct Ranking<M: Model, C: ConstraintSet<M::Plan, ScoreOf<M>>> {
    director: Director<M, C>,
    base: ScoreOf<M>,
    base_by: Vec<(String, ScoreOf<M>)>,
}

/// The weighted incremental director `fill` scores with.
pub type Director<M, C> = ScoreDirector<<M as Model>::Plan, C, SolutionDescriptor, WeightedMode>;

/// Makes candidate `n`'s change and returns the plan's new score.
pub type Apply<'a, M, C> = dyn FnMut(&mut Director<M, C>, usize) -> ScoreOf<M> + 'a;
/// Reverts candidate `n`'s change.
pub type Undo<'a, M, C> = dyn FnMut(&mut Director<M, C>, usize) + 'a;

/// A candidate's score, kept until the best ones are explained.
struct Tried<C> {
    value: EntityName,
    score: C,
    change: usize,
}

impl<M: Model, C: ConstraintSet<M::Plan, ScoreOf<M>>> Ranking<M, C> {
    pub fn new(plan: M::Plan, constraints: C) -> Self {
        let mut director = ScoreDirector::new_weighted(plan, constraints);
        let base = director.calculate_score();
        let base_by = by_constraint::<M, C>(&mut director);
        Self {
            director,
            base,
            base_by,
        }
    }

    /// The plan, for reading while candidates are tried.
    pub fn plan(&self) -> &M::Plan {
        self.director.working_solution()
    }

    /// The director, for models that make changes themselves.
    pub fn director(&mut self) -> &mut Director<M, C> {
        &mut self.director
    }

    /// Ranks the candidates for one item: `apply(n)` makes candidate `n`'s
    /// change and returns the new score; `undo(n)` reverts it. Keeps `top`.
    pub fn rank(
        &mut self,
        type_name: &str,
        item: EntityName,
        candidates: Vec<EntityName>,
        top: usize,
        apply: &mut Apply<'_, M, C>,
        undo: &mut Undo<'_, M, C>,
    ) -> FillItem {
        let mut tried: Vec<Tried<ScoreOf<M>>> = Vec::with_capacity(candidates.len());
        for (change, value) in candidates.into_iter().enumerate() {
            let score = apply(&mut self.director, change);
            undo(&mut self.director, change);
            tried.push(Tried {
                value,
                score,
                change,
            });
        }
        tried.sort_by_key(|candidate| std::cmp::Reverse(candidate.score));
        tried.truncate(top);
        let mut options = Vec::with_capacity(tried.len());
        for candidate in tried {
            apply(&mut self.director, candidate.change);
            let after = by_constraint::<M, C>(&mut self.director);
            undo(&mut self.director, candidate.change);
            options.push(FillOption {
                value: candidate.value,
                score: candidate.score.to_string(),
                delta: (candidate.score - self.base).to_string(),
                feasible: candidate.score.is_feasible(),
                changes: changes::<M>(&self.base_by, &after),
            });
        }
        FillItem {
            type_name: type_name.to_string(),
            item,
            options,
        }
    }

    /// Gives the plan back, with every tried change undone.
    pub fn into_plan(self) -> M::Plan {
        self.director.into_working_solution()
    }
}

fn by_constraint<M: Model, C: ConstraintSet<M::Plan, ScoreOf<M>>>(
    director: &mut Director<M, C>,
) -> Vec<(String, ScoreOf<M>)> {
    director
        .constraint_analyses()
        .into_iter()
        .map(|analysis| (analysis.constraint_ref.full_name(), analysis.score))
        .collect()
}

fn changes<M: Model>(
    before: &[(String, ScoreOf<M>)],
    after: &[(String, ScoreOf<M>)],
) -> Vec<ConstraintChange> {
    let catalog = M::catalog();
    after
        .iter()
        .filter_map(|(name, score)| {
            let old = before
                .iter()
                .find(|(other, _)| other == name)
                .map_or(ScoreOf::<M>::zero(), |(_, s)| *s);
            let delta = *score - old;
            if delta == ScoreOf::<M>::zero() {
                return None;
            }
            let level = catalog
                .iter()
                .find(|info| info.name == name)
                .map(|info| info.level)
                .unwrap_or_else(|| level_of_delta::<M>(&delta));
            Some(ConstraintChange {
                constraint: name.clone(),
                level,
                delta: delta.to_string(),
            })
        })
        .collect()
}

fn level_of_delta<M: Model>(delta: &ScoreOf<M>) -> Level {
    let levels = delta.to_level_numbers();
    match levels.iter().position(|value| *value != 0) {
        Some(0) if levels.len() > 1 => Level::Hard,
        Some(index) if index + 1 < levels.len() => Level::Medium,
        _ => Level::Soft,
    }
}

/// Names entity `index` of `type_name`: through the model, or by type and
/// position when the model has no name for it.
pub fn name_of<M: Model>(plan: &M::Plan, type_name: &str, index: usize) -> EntityName {
    M::name_at(plan, type_name, index).unwrap_or_else(|| EntityName {
        id: Some(format!("{type_name}#{index}")),
        label: format!("{type_name} {index}"),
    })
}

pub(crate) fn short(type_name: &str) -> &str {
    type_name.rsplit("::").next().unwrap_or(type_name)
}

/// One scalar planning variable the generic fill can set.
pub(crate) struct Slot {
    pub(crate) descriptor: usize,
    pub(crate) entity_type: String,
    pub(crate) variable: usize,
    pub(crate) value_type: Option<String>,
}

pub(crate) fn slots(descriptor: &SolutionDescriptor) -> Vec<Slot> {
    let mut out = Vec::new();
    for (index, entity) in descriptor.entity_descriptors.iter().enumerate() {
        for (variable, var) in entity.variable_descriptors.iter().enumerate() {
            if var.variable_type != VariableType::Genuine
                || var.usize_getter.is_none()
                || var.usize_setter.is_none()
            {
                continue;
            }
            let value_type = var.value_range_provider.and_then(|field| {
                descriptor
                    .problem_fact_descriptors
                    .iter()
                    .map(|fact| (fact.solution_field, fact.type_name))
                    .chain(
                        descriptor
                            .entity_descriptors
                            .iter()
                            .map(|e| (e.solution_field, e.type_name)),
                    )
                    .find(|(solution_field, _)| *solution_field == field)
                    .map(|(_, type_name)| short(type_name).to_string())
            });
            out.push(Slot {
                descriptor: index,
                entity_type: short(entity.type_name).to_string(),
                variable,
                value_type,
            });
        }
    }
    out
}

/// Values the variable may take for one entity: its own value provider, or
/// every record of the value range.
pub(crate) fn values(
    descriptor: &SolutionDescriptor,
    slot: &Slot,
    plan: &dyn Any,
    entity: &dyn Any,
) -> Vec<usize> {
    let var = &descriptor.entity_descriptors[slot.descriptor].variable_descriptors[slot.variable];
    if let Some(provider) = var.entity_value_provider {
        return provider(entity);
    }
    let Some(field) = var.value_range_provider else {
        return Vec::new();
    };
    let count = descriptor
        .problem_fact_descriptors
        .iter()
        .filter(|fact| fact.solution_field == field)
        .filter_map(|fact| fact.extractor.as_ref())
        .chain(
            descriptor
                .entity_descriptors
                .iter()
                .filter(|e| e.solution_field == field)
                .filter_map(|e| e.extractor.as_ref()),
        )
        .find_map(|extractor| extractor.count(plan))
        .unwrap_or(0);
    (0..count).collect()
}

/// The generic fill for models whose planning variables are single values.
pub fn scalar<M: Model>(
    plan: M::Plan,
    request: &FillRequest,
) -> Result<FillAnswer<M::Plan>, EngineError> {
    let descriptor = M::descriptor();
    let slots = slots(&descriptor);
    if slots.is_empty() {
        return Err(EngineError::new(
            "fill_unsupported",
            format!(
                "model {} has no single-value planning variable to fill",
                M::id()
            ),
        ));
    }
    let mut ranking = Ranking::<M, _>::new(plan, M::constraints());
    // (slot, entity index, item name, candidate value indexes and names)
    let mut work = Vec::new();
    for (slot_index, slot) in slots.iter().enumerate() {
        let entity_descriptor = &descriptor.entity_descriptors[slot.descriptor];
        let var = &entity_descriptor.variable_descriptors[slot.variable];
        let (Some(extractor), Some(getter)) =
            (entity_descriptor.extractor.as_ref(), var.usize_getter)
        else {
            continue;
        };
        let plan: &dyn Any = ranking.plan();
        let count = extractor.count(plan).unwrap_or(0);
        for index in 0..count {
            let Some(entity) = extractor.get(plan, index) else {
                continue;
            };
            let item = name_of::<M>(ranking.plan(), &slot.entity_type, index);
            let wanted = if request.items.is_empty() {
                getter(entity).is_none()
            } else {
                item.id
                    .as_ref()
                    .is_some_and(|id| request.items.contains(id))
            };
            if !wanted
                && request
                    .apply
                    .as_ref()
                    .is_none_or(|c| item.id.as_deref() != Some(&c.item))
            {
                continue;
            }
            let candidates: Vec<usize> = values(&descriptor, slot, plan, entity);
            let named: Vec<EntityName> = candidates
                .iter()
                .map(|value| match &slot.value_type {
                    Some(type_name) => name_of::<M>(ranking.plan(), type_name, *value),
                    None => EntityName {
                        id: Some(value.to_string()),
                        label: value.to_string(),
                    },
                })
                .collect();
            work.push((slot_index, index, item, candidates, named, getter(entity)));
        }
    }
    if let Some(choice) = &request.apply {
        let Some((slot_index, index, _, candidates, named, _)) = work
            .iter()
            .find(|(_, _, item, ..)| item.id.as_deref() == Some(choice.item.as_str()))
        else {
            return Err(EngineError::new(
                "unknown_item",
                format!("no open item {:?} to fill", choice.item),
            ));
        };
        let Some(position) = named
            .iter()
            .position(|n| n.id.as_deref() == Some(choice.value.as_str()))
        else {
            return Err(EngineError::new(
                "unknown_value",
                format!("{:?} is not a place for {:?}", choice.value, choice.item),
            ));
        };
        let slot = &slots[*slot_index];
        let value = candidates[position];
        set::<M, _>(&descriptor, slot, ranking.director(), *index, Some(value));
        return Ok(FillAnswer::Applied(ranking.into_plan()));
    }
    let mut items = Vec::with_capacity(work.len());
    for (slot_index, index, item, candidates, named, current) in work {
        let slot = &slots[slot_index];
        let descriptor = &descriptor;
        items.push(ranking.rank(
            &slot.entity_type,
            item,
            named,
            request.top.max(1),
            &mut |director, n| set::<M, _>(descriptor, slot, director, index, Some(candidates[n])),
            &mut |director, _| {
                set::<M, _>(descriptor, slot, director, index, current);
            },
        ));
    }
    Ok(FillAnswer::Options(items))
}

fn set<M: Model, C: ConstraintSet<M::Plan, ScoreOf<M>>>(
    descriptor: &SolutionDescriptor,
    slot: &Slot,
    director: &mut Director<M, C>,
    index: usize,
    value: Option<usize>,
) -> ScoreOf<M> {
    let entity_descriptor = &descriptor.entity_descriptors[slot.descriptor];
    let setter = entity_descriptor.variable_descriptors[slot.variable]
        .usize_setter
        .expect("slots only hold variables with a setter");
    let extractor = entity_descriptor
        .extractor
        .as_ref()
        .expect("slots only hold entities with an extractor");
    director.do_change(slot.descriptor, index, |plan: &mut M::Plan| {
        let plan: &mut dyn Any = plan;
        if let Some(entity) = extractor.get_mut(plan, index) {
            setter(entity, value);
        }
    })
}
