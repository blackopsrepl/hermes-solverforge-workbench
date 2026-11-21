//! Parts a vehicle carries and the parts a stop or visit uses up.
//!
//! A shift loads its `stock` (`[{part, quantity}]`) before it sets off, and
//! every visit it serves takes the `parts` it needs from that stock, in route
//! order. A visit reached when the stock no longer holds what it needs is
//! short by the missing units.

use serde::{Deserialize, Serialize};

use crate::data::{col, list, Column};

/// Distinct part names a plan may use.
pub const MAX_PARTS: usize = 16;

/// A quantity of one part.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Part {
    pub part: String,
    pub quantity: i64,
}

/// Part names by first use, so a plan's parts become indices.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PartNames {
    pub names: Vec<String>,
}

impl PartNames {
    /// The index of `name`, given one the first time it is seen.
    pub fn index(&mut self, name: &str) -> usize {
        match self.names.iter().position(|n| n == name) {
            Some(i) => i,
            None => {
                self.names.push(name.to_string());
                self.names.len() - 1
            }
        }
    }

    /// `parts` as `(index, quantity)`, or why they cannot be read: a quantity
    /// below zero, or more distinct parts than [`MAX_PARTS`].
    pub fn read(&mut self, parts: &[Part], what: &str) -> Result<Vec<(usize, i64)>, String> {
        let mut out = Vec::with_capacity(parts.len());
        for part in parts {
            if part.quantity < 0 {
                return Err(format!(
                    "{what}: part {:?} has a quantity below zero",
                    part.part
                ));
            }
            let index = self.index(&part.part);
            if index >= MAX_PARTS {
                return Err(format!(
                    "the dataset names more than {MAX_PARTS} different parts"
                ));
            }
            out.push((index, part.quantity));
        }
        Ok(out)
    }
}

/// Takes `needs` from a stock of which `used` is already taken: the units
/// this takes beyond the stock, the parts it had before counted once.
pub fn consume(used: &mut [i64; MAX_PARTS], stock: &[i64], needs: &[(usize, i64)]) -> i64 {
    let mut short = 0;
    for &(part, quantity) in needs {
        let held = stock.get(part).copied().unwrap_or(0);
        let before = (used[part] - held).max(0);
        used[part] += quantity;
        short += (used[part] - held).max(0) - before;
    }
    short
}

/// A list of parts and quantities, for the page's data editor.
pub fn column(name: &'static str, label: &'static str) -> Column {
    list(
        name,
        label,
        col("part", "Part", "object").fields(vec![
            col("part", "Part", "text"),
            col("quantity", "Quantity", "integer"),
        ]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_visit_is_short_only_by_the_units_the_stock_no_longer_holds() {
        let mut used = [0; MAX_PARTS];
        let stock = [3, 1];
        assert_eq!(consume(&mut used, &stock, &[(0, 2)]), 0);
        assert_eq!(consume(&mut used, &stock, &[(0, 2), (1, 1)]), 1);
        assert_eq!(consume(&mut used, &stock, &[(0, 1), (1, 2)]), 3);
        // A part the shift does not carry at all.
        assert_eq!(consume(&mut used, &stock, &[(5, 2)]), 2);
    }

    #[test]
    fn part_names_become_indices_and_bad_quantities_are_refused() {
        let mut names = PartNames::default();
        let part = |name: &str, quantity| Part {
            part: name.into(),
            quantity,
        };
        assert_eq!(
            names.read(
                &[part("filter", 1), part("valve", 2), part("filter", 3)],
                "v"
            ),
            Ok(vec![(0, 1), (1, 2), (0, 3)])
        );
        assert!(names
            .read(&[part("x", -1)], "v")
            .unwrap_err()
            .contains("below zero"));
        let many: Vec<Part> = (0..20).map(|i| part(&format!("p{i}"), 1)).collect();
        assert!(names.read(&many, "v").unwrap_err().contains("more than 16"));
    }
}
