//! A model's data as tables the page edits: each entity and fact type a
//! table of rows, each field a typed column, references naming the table
//! they point into. Read from the model's wire records, so one generic grid
//! edits every model's datasets instead of a hand-made editor per model.

use serde::Serialize;

use crate::model::ParameterOption;

/// One kind of record in a plan: technicians, visits, shifts of a technician...
#[derive(Debug, Clone, Serialize)]
pub struct Table {
    pub id: &'static str,
    /// What a person calls the rows: `Visits`.
    pub label: &'static str,
    /// One row: `visit`.
    pub one: &'static str,
    /// Where the rows are: `visits`, or `vehicles/*/shifts` for rows inside
    /// each row of the `parent` table.
    pub path: &'static str,
    #[serde(skip_serializing_if = "str::is_empty")]
    pub parent: &'static str,
    /// The field that names a row; its id when unset.
    #[serde(skip_serializing_if = "str::is_empty")]
    pub title: &'static str,
    pub columns: Vec<Column>,
}

/// One field of a row, typed for its editor.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Column {
    /// The wire key: `serviceDuration`.
    pub name: &'static str,
    pub label: &'static str,
    /// `text`, `integer`, `number`, `boolean`, `minutes`, `date_time`
    /// (`YYYY-MM-DDTHH:MM`), `date`, `location` (`[lat, lng]`), `object` (its
    /// `fields`), `list` (of `fields[0]`) or `json` (any value, edited as JSON).
    pub kind: &'static str,
    #[serde(skip_serializing_if = "str::is_empty")]
    pub unit: &'static str,
    /// The values a text may take; any when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<ParameterOption>,
    /// An object's fields, or a list's item as its one field.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<Column>,
    /// The table whose row ids this value names.
    #[serde(skip_serializing_if = "str::is_empty")]
    pub reference: &'static str,
    /// A value that may be left out.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub optional: bool,
}

/// A column of `kind`.
pub fn col(name: &'static str, label: &'static str, kind: &'static str) -> Column {
    Column {
        name,
        label,
        kind,
        ..Column::default()
    }
}

impl Column {
    pub fn unit(mut self, unit: &'static str) -> Self {
        self.unit = unit;
        self
    }
    pub fn options(mut self, options: Vec<ParameterOption>) -> Self {
        self.options = options;
        self
    }
    pub fn fields(mut self, fields: Vec<Column>) -> Self {
        self.fields = fields;
        self
    }
    pub fn reference(mut self, table: &'static str) -> Self {
        self.reference = table;
        self
    }
    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }
}

/// A list of `item`s.
pub fn list(name: &'static str, label: &'static str, item: Column) -> Column {
    col(name, label, "list").fields(vec![item]).optional()
}

/// A list of time windows with the given field names, all local date-times.
pub fn windows(
    name: &'static str,
    label: &'static str,
    fields: &[(&'static str, &'static str)],
) -> Column {
    list(
        name,
        label,
        col("window", "Window", "object").fields(
            fields
                .iter()
                .map(|&(field, words)| col(field, words, "date_time").optional())
                .collect(),
        ),
    )
}

/// The rows of `table` in a wire plan, following its path (`*` walks every row
/// of the parent level).
pub fn rows<'a>(plan: &'a serde_json::Value, path: &str) -> Vec<&'a serde_json::Value> {
    let mut level = vec![plan];
    for part in path.split('/') {
        level = level
            .into_iter()
            .flat_map(|value| match part {
                "*" => value
                    .as_array()
                    .map(|a| a.iter().collect())
                    .unwrap_or_default(),
                key => value.get(key).into_iter().collect::<Vec<_>>(),
            })
            .collect();
    }
    level
        .into_iter()
        .flat_map(|v| v.as_array().into_iter().flatten())
        .collect()
}

/// What does not match between `tables` and a real plan: a table with no rows,
/// a row without an id, a required column missing from a row, or a column no
/// row ever has (a misspelt wire key). Model tests run it on their demos.
pub fn mismatches(plan: &serde_json::Value, tables: &[Table]) -> Vec<String> {
    let mut out = Vec::new();
    for table in tables {
        let found = rows(plan, table.path);
        if found.is_empty() {
            out.push(format!("{}: no rows at {}", table.id, table.path));
            continue;
        }
        for row in &found {
            if row.get("id").is_none() {
                out.push(format!("{}: a row without an id", table.id));
            }
        }
        for column in &table.columns {
            let present = found
                .iter()
                .filter(|r| r.get(column.name).is_some())
                .count();
            if !column.optional && present < found.len() {
                out.push(format!(
                    "{}.{}: missing from {} rows",
                    table.id,
                    column.name,
                    found.len() - present
                ));
            }
            if present == 0 {
                out.push(format!("{}.{}: no row has it", table.id, column.name));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_read_as_their_editors_need_them() {
        let skills = list(
            "skills",
            "Skills",
            col("skill", "Skill", "object").fields(vec![
                col("name", "Name", "text"),
                col("level", "Level", "integer").optional(),
            ]),
        );
        let json = serde_json::to_value(&skills).unwrap();
        assert_eq!(json["kind"], "list");
        assert_eq!(json["optional"], true);
        assert_eq!(json["fields"][0]["fields"][1]["optional"], true);
        assert!(json.get("reference").is_none() && json.get("unit").is_none());
        let plan = serde_json::json!({"crew": [{"id": "a", "shifts": [{"id": "s1", "day": 1}, {"id": "s2"}]}]});
        let shifts = rows(&plan, "crew/*/shifts");
        assert_eq!(shifts.len(), 2);
        let table = Table {
            id: "shifts",
            label: "Shifts",
            one: "shift",
            path: "crew/*/shifts",
            parent: "crew",
            title: "",
            columns: vec![
                col("day", "Day", "integer"),
                col("night", "Night", "boolean").optional(),
            ],
        };
        assert_eq!(
            mismatches(&plan, &[table]),
            [
                "shifts.day: missing from 1 rows",
                "shifts.night: no row has it"
            ]
        );
        let window = windows("timeWindows", "Time windows", &[("minStartTime", "From")]);
        assert_eq!(
            serde_json::to_value(window).unwrap()["fields"][0]["fields"][0]["kind"],
            "date_time"
        );
    }
}
