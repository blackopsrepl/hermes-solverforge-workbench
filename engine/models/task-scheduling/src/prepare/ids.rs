//! Record ids resolved to indexes, with duplicates and unknown ids reported.

use std::collections::HashMap;

use solverforge_workbench_host::Issue;

/// Index of every record of one kind by id.
pub struct Ids<'p> {
    kind: &'static str,
    by_id: HashMap<&'p str, usize>,
}

impl<'p> Ids<'p> {
    /// Indexes `ids`, reporting empty and duplicate ones.
    pub fn new(
        kind: &'static str,
        ids: impl Iterator<Item = &'p str>,
        issues: &mut Vec<Issue>,
    ) -> Self {
        let mut by_id = HashMap::new();
        for (index, id) in ids.enumerate() {
            if id.trim().is_empty() {
                issues.push(Issue::error(
                    "empty_id",
                    format!("{kind} {index} has an empty id"),
                ));
            } else if by_id.insert(id, index).is_some() {
                issues.push(Issue::error(
                    "duplicate_id",
                    format!("two {kind}s have the id {id:?}"),
                ));
            }
        }
        Self { kind, by_id }
    }

    /// The index of `id`, or an `unknown_reference` error naming `context`.
    pub fn find(&self, id: &str, context: &str, issues: &mut Vec<Issue>) -> Option<usize> {
        let found = self.by_id.get(id).copied();
        if found.is_none() {
            issues.push(Issue::error(
                "unknown_reference",
                format!(
                    "{context} refers to {} {id:?}, which the dataset does not declare",
                    self.kind
                ),
            ));
        }
        found
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }
}
