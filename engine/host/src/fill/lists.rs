//! The generic fill for list planning variables.

use solverforge::ConstraintSet;

use super::{name_of, Director, FillAnswer, FillRequest, Ranking};
use crate::model::{EntityName, Model, ScoreOf};
use crate::protocol::EngineError;

/// How [`list`] reads and changes one list planning variable: owners (a
/// vehicle) hold ordered lists of element indexes (its visits).
pub struct ListVar<P> {
    /// Short type name of the owner entity, as `name_at` knows it (`Route`).
    pub owner_type: &'static str,
    /// The owner's entity descriptor index, as the score director counts them.
    pub owner_descriptor: usize,
    /// Short type name of the list's elements (`Visit`).
    pub element_type: &'static str,
    /// How many owners the plan has.
    pub owner_count: fn(&P) -> usize,
    /// How many elements the plan has, assigned or not.
    pub element_count: fn(&P) -> usize,
    /// One owner's list.
    pub list: fn(&P, usize) -> &[usize],
    /// Inserts `element` into owner `owner`'s list at `position`.
    pub insert: fn(&mut P, usize, usize, usize),
    /// Removes and returns the element at `position` of owner `owner`'s list.
    pub remove: fn(&mut P, usize, usize) -> usize,
}

/// Where an element can go: owner and position in the owner's list as it is
/// without the element.
#[derive(Clone, Copy)]
struct Place {
    owner: usize,
    position: usize,
}

/// One element to place: where it is now, and every place it can go.
struct Work {
    var: usize,
    element: usize,
    item: EntityName,
    current: Option<Place>,
    places: Vec<Place>,
    named: Vec<EntityName>,
}

/// Ranked insertions for the list elements a plan leaves open (or the
/// requested ones, moved), or one applied.
///
/// A place names its owner and position: `"<owner id>@<position>"`, the
/// position counted in the owner's list without the element.
pub fn list<M: Model>(
    plan: M::Plan,
    request: &FillRequest,
    vars: &[ListVar<M::Plan>],
) -> Result<FillAnswer<M::Plan>, EngineError> {
    if vars.is_empty() {
        return Err(EngineError::new(
            "fill_unsupported",
            format!("model {} has no list planning variable to fill", M::id()),
        ));
    }
    let mut ranking = Ranking::<M, _>::new(plan, M::constraints());
    let mut work = Vec::new();
    for (var_index, var) in vars.iter().enumerate() {
        let plan = ranking.plan();
        let owners = (var.owner_count)(plan);
        let mut at = vec![None; (var.element_count)(plan)];
        for owner in 0..owners {
            for (position, &element) in (var.list)(plan, owner).iter().enumerate() {
                if let Some(slot) = at.get_mut(element) {
                    *slot = Some(Place { owner, position });
                }
            }
        }
        for (element, current) in at.iter().enumerate() {
            let item = name_of::<M>(plan, var.element_type, element);
            let chosen = request
                .apply
                .as_ref()
                .is_some_and(|c| item.id.as_deref() == Some(c.item.as_str()));
            let wanted = if request.items.is_empty() {
                current.is_none()
            } else {
                item.id
                    .as_ref()
                    .is_some_and(|id| request.items.contains(id))
            };
            if !wanted && !chosen {
                continue;
            }
            let mut places = Vec::new();
            let mut named = Vec::new();
            for owner in 0..owners {
                let owner_name = name_of::<M>(plan, var.owner_type, owner);
                let others: Vec<usize> = (var.list)(plan, owner)
                    .iter()
                    .copied()
                    .filter(|&other| other != element)
                    .collect();
                for position in 0..=others.len() {
                    places.push(Place { owner, position });
                    named.push(place_name::<M>(
                        plan,
                        var,
                        &owner_name,
                        position,
                        position.checked_sub(1).map(|p| others[p]),
                    ));
                }
            }
            work.push(Work {
                var: var_index,
                element,
                item,
                current: *current,
                places,
                named,
            });
        }
    }
    if let Some(choice) = &request.apply {
        let Some(found) = work
            .iter()
            .find(|w| w.item.id.as_deref() == Some(choice.item.as_str()))
        else {
            return Err(EngineError::new(
                "unknown_item",
                format!("no item {:?} to fill", choice.item),
            ));
        };
        let Some(n) = found
            .named
            .iter()
            .position(|name| name.id.as_deref() == Some(choice.value.as_str()))
        else {
            return Err(EngineError::new(
                "unknown_value",
                format!("{:?} is not a place for {:?}", choice.value, choice.item),
            ));
        };
        let var = &vars[found.var];
        let director = ranking.director();
        if let Some(from) = found.current {
            take::<M, _>(var, director, from);
        }
        put::<M, _>(var, director, found.places[n], found.element);
        return Ok(FillAnswer::Applied(ranking.into_plan()));
    }
    let mut items = Vec::with_capacity(work.len());
    for job in work {
        let var = &vars[job.var];
        let Work {
            element,
            item,
            current,
            places,
            named,
            ..
        } = job;
        items.push(ranking.rank(
            var.element_type,
            item,
            named,
            request.top.max(1),
            &mut |director, n| {
                if let Some(from) = current {
                    take::<M, _>(var, director, from);
                }
                put::<M, _>(var, director, places[n], element)
            },
            &mut |director, n| {
                take::<M, _>(var, director, places[n]);
                if let Some(from) = current {
                    put::<M, _>(var, director, from, element);
                }
            },
        ));
    }
    Ok(FillAnswer::Options(items))
}

/// Names a place: `"<owner id>@<position>"`, read as the owner and the
/// element it follows.
fn place_name<M: Model>(
    plan: &M::Plan,
    var: &ListVar<M::Plan>,
    owner: &EntityName,
    position: usize,
    after: Option<usize>,
) -> EntityName {
    let owner_id = owner.id.as_deref().unwrap_or(&owner.label);
    let label = match after {
        None => format!("{}, first", owner.label),
        Some(element) => format!(
            "{}, after {}",
            owner.label,
            name_of::<M>(plan, var.element_type, element).label
        ),
    };
    EntityName {
        id: Some(format!("{owner_id}@{position}")),
        label,
    }
}

fn put<M: Model, C: ConstraintSet<M::Plan, ScoreOf<M>>>(
    var: &ListVar<M::Plan>,
    director: &mut Director<M, C>,
    place: Place,
    element: usize,
) -> ScoreOf<M> {
    let insert = var.insert;
    director.do_change(var.owner_descriptor, place.owner, |plan: &mut M::Plan| {
        insert(plan, place.owner, place.position, element)
    })
}

fn take<M: Model, C: ConstraintSet<M::Plan, ScoreOf<M>>>(
    var: &ListVar<M::Plan>,
    director: &mut Director<M, C>,
    place: Place,
) -> ScoreOf<M> {
    let remove = var.remove;
    director.do_change(var.owner_descriptor, place.owner, |plan: &mut M::Plan| {
        remove(plan, place.owner, place.position);
    })
}
