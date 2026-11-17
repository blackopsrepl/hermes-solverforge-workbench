//! The generic plan view: items grouped by what they are assigned to.

use std::any::Any;
use std::collections::BTreeMap;

use super::{blame, Assignments, Board, Broken, Item, Lane, View};
use crate::analysis::Analysis;
use crate::fill::{name_of, short, slots, ListVar};
use crate::model::{EntityName, Model};

/// Every planning variable of the plan as a board: single-value variables
/// through the planning descriptors, list variables through `lists`.
pub fn assignments<M: Model>(
    plan: &M::Plan,
    analysis: &Analysis,
    lists: &[ListVar<M::Plan>],
) -> Option<View> {
    let broken = blame(analysis);
    let item = |type_name: &str, name: EntityName| -> Item {
        let id = name.id.unwrap_or_else(|| name.label.clone());
        Item {
            broken: broken
                .get(&(type_name.to_string(), id.clone()))
                .cloned()
                .unwrap_or_default(),
            id,
            label: name.label,
        }
    };
    let lane = |type_name: &str, name: EntityName, items: Vec<Item>| -> Lane {
        let id = name.id.unwrap_or_else(|| name.label.clone());
        Lane {
            broken: broken
                .get(&(type_name.to_string(), id.clone()))
                .cloned()
                .unwrap_or_default(),
            id,
            label: name.label,
            items,
        }
    };
    let descriptor = M::descriptor();
    let mut boards = Vec::new();
    for slot in slots(&descriptor) {
        let entity = &descriptor.entity_descriptors[slot.descriptor];
        let var = &entity.variable_descriptors[slot.variable];
        let (Some(extractor), Some(getter)) = (entity.extractor.as_ref(), var.usize_getter) else {
            continue;
        };
        let any: &dyn Any = plan;
        let value_count = var
            .value_range_provider
            .and_then(|field| {
                descriptor
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
                    .find_map(|extractor| extractor.count(any))
            })
            .unwrap_or(0);
        let mut held: BTreeMap<usize, Vec<Item>> = BTreeMap::new();
        let mut unassigned = Vec::new();
        for index in 0..extractor.count(any).unwrap_or(0) {
            let Some(record) = extractor.get(any, index) else {
                continue;
            };
            let shown = item(
                &slot.entity_type,
                name_of::<M>(plan, &slot.entity_type, index),
            );
            match getter(record) {
                Some(value) => held.entry(value).or_default().push(shown),
                None => unassigned.push(shown),
            }
        }
        let lane_type = slot
            .value_type
            .clone()
            .unwrap_or_else(|| var.name.to_string());
        let values: Vec<usize> = if slot.value_type.is_some() {
            (0..value_count)
                .chain(held.keys().copied().filter(|v| *v >= value_count))
                .collect()
        } else {
            held.keys().copied().collect()
        };
        let lanes = values
            .into_iter()
            .map(|value| {
                let name = match &slot.value_type {
                    Some(type_name) => name_of::<M>(plan, type_name, value),
                    None => EntityName {
                        id: Some(value.to_string()),
                        label: value.to_string(),
                    },
                };
                lane(&lane_type, name, held.remove(&value).unwrap_or_default())
            })
            .collect();
        boards.push(Board {
            title: format!("{} by {}", slot.entity_type, lane_type),
            item: slot.entity_type.clone(),
            lane: lane_type,
            lanes,
            unassigned,
        });
    }
    for var in lists {
        let owners = (var.owner_count)(plan);
        let mut placed = vec![false; (var.element_count)(plan)];
        let mut lanes = Vec::with_capacity(owners);
        for owner in 0..owners {
            let items = (var.list)(plan, owner)
                .iter()
                .map(|&element| {
                    if let Some(seen) = placed.get_mut(element) {
                        *seen = true;
                    }
                    item(
                        var.element_type,
                        name_of::<M>(plan, var.element_type, element),
                    )
                })
                .collect();
            lanes.push(lane(
                var.owner_type,
                name_of::<M>(plan, var.owner_type, owner),
                items,
            ));
        }
        let unassigned = placed
            .iter()
            .enumerate()
            .filter(|(_, placed)| !**placed)
            .map(|(element, _)| {
                item(
                    var.element_type,
                    name_of::<M>(plan, var.element_type, element),
                )
            })
            .collect();
        boards.push(Board {
            title: format!("{} by {}", short(var.element_type), short(var.owner_type)),
            item: var.element_type.to_string(),
            lane: var.owner_type.to_string(),
            lanes,
            unassigned,
        });
    }
    if boards.is_empty() {
        return None;
    }
    let open: usize = boards.iter().map(|b| b.unassigned.len()).sum();
    let breaking = boards
        .iter()
        .flat_map(|b| {
            b.lanes
                .iter()
                .flat_map(|l| l.items.iter())
                .chain(b.unassigned.iter())
        })
        .filter(|i| has_own_break(&i.broken))
        .count();
    Some(View::Assignments(Assignments {
        title: M::name().to_string(),
        subtitle: format!(
            "{open} unassigned \u{b7} {breaking} breaking a hard or medium rule; red marks a hard break, rose a medium one"
        ),
        boards,
    }))
}

fn has_own_break(broken: &[Broken]) -> bool {
    broken.iter().any(|b| !b.shared)
}
