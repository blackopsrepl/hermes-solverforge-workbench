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
    /// each row of the `parent` table. A path that holds one object, not a
    /// list (`planningWindow`), is a table of that one row. Rows are addressed
    /// by id when they have one, by position otherwise.
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
    /// Written by the solver (a route's itinerary, a machine's idle time):
    /// shown, never edited.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub read_only: bool,
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
    /// The solver writes it; the editor shows it.
    pub fn read_only(mut self) -> Self {
        self.read_only = true;
        self.optional = true;
        self
    }
}

/// A text that is one of `choices`, each `(value, label)`.
pub fn choice(
    name: &'static str,
    label: &'static str,
    choices: &[(&'static str, &'static str)],
) -> Column {
    col(name, label, "text").options(
        choices
            .iter()
            .map(|&(value, words)| ParameterOption::new(value, words))
            .collect(),
    )
}

/// Keys of a plan the data editor leaves to other pages: the name, and the
/// weights and parameters a profile sets.
pub const SETTINGS: [&str; 4] = ["name", "weights", "parameters", "score"];

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
        .flat_map(|v| match v {
            serde_json::Value::Array(rows) => rows.iter().collect(),
            serde_json::Value::Object(_) => vec![v],
            _ => Vec::new(),
        })
        .collect()
}

/// What does not match between `tables` and a real plan: a top-level key no
/// table holds (other than the [`SETTINGS`] and the solver's `outputs`), a
/// table with no rows, rows of one table only some of which have an id, a
/// required column missing from a row, a column no row ever has (a misspelt
/// wire key), or a field the rows carry that no column or nested table shows.
/// Model tests run it on their demos, so every field a model reads has an
/// editor.
pub fn mismatches(plan: &serde_json::Value, tables: &[Table], outputs: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for key in plan
        .as_object()
        .into_iter()
        .flatten()
        .map(|(k, _)| k.as_str())
    {
        let held = tables.iter().any(|t| t.path.split('/').next() == Some(key));
        if !held && !SETTINGS.contains(&key) && !outputs.contains(&key) {
            out.push(format!("the plan has {key}, no table holds it"));
        }
    }
    for table in tables {
        let found = rows(plan, table.path);
        if found.is_empty() {
            out.push(format!("{}: no rows at {}", table.id, table.path));
            continue;
        }
        let with_id = found.iter().filter(|r| r.get("id").is_some()).count();
        if with_id > 0 && with_id < found.len() {
            out.push(format!(
                "{}: {} of {} rows have no id",
                table.id,
                found.len() - with_id,
                found.len()
            ));
        }
        // Every field a row carries is either a column, or holds the rows of a
        // table nested under this one: the editor shows each field somewhere.
        let nested: Vec<&str> = tables
            .iter()
            .filter_map(|t| t.path.strip_prefix(table.path)?.strip_prefix("/*/"))
            .filter_map(|rest| rest.split('/').next())
            .collect();
        let mut extra: Vec<&str> = found
            .iter()
            .flat_map(|r| r.as_object().into_iter().flatten().map(|(k, _)| k.as_str()))
            .filter(|k| {
                *k != "id" && !nested.contains(k) && !table.columns.iter().any(|c| c.name == *k)
            })
            .collect();
        extra.sort_unstable();
        extra.dedup();
        for key in extra {
            out.push(format!(
                "{}: the data has {key}, the table does not describe it",
                table.id
            ));
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
            // A JSON column has no sample value to try (see `unread`): the demo
            // must carry it.
            if present == 0 && column.kind == "json" && !column.read_only {
                out.push(format!("{}.{}: no row has it", table.id, column.name));
            }
            let values: Vec<&serde_json::Value> =
                found.iter().filter_map(|r| r.get(column.name)).collect();
            undescribed(
                &format!("{}.{}", table.id, column.name),
                column,
                &values,
                &mut out,
            );
        }
    }
    out
}

/// Columns the model does not read: every column of each table's first row is
/// set to a sample of its kind, the plan goes through `roundtrip` (the model
/// reading it and writing it back), and every value, down to each field of an
/// object and of a list's item, must come back. Catches a misspelt key or a
/// column for a field the model has no use for, including fields the demo
/// never sets.
pub fn unread(
    plan: &serde_json::Value,
    tables: &[Table],
    roundtrip: impl FnOnce(serde_json::Value) -> Result<serde_json::Value, String>,
) -> Vec<String> {
    let mut filled = plan.clone();
    let mut samples = Vec::new();
    for table in tables {
        let parts: Vec<&str> = table.path.split('/').collect();
        let Some(row) = first_row_mut(&mut filled, &parts) else {
            continue;
        };
        for column in &table.columns {
            if let Some(value) = sample(plan, tables, column) {
                row[column.name] = value.clone();
                samples.push((table, column.name, value));
            }
        }
    }
    let back = match roundtrip(filled) {
        Ok(back) => back,
        Err(error) => return vec![format!("the model refused the sample values: {error}")],
    };
    let mut out = Vec::new();
    for (table, name, value) in samples {
        let parts: Vec<&str> = table.path.split('/').collect();
        let got = first_row(&back, &parts).and_then(|row| row.get(name));
        lost(&format!("{}.{name}", table.id), &value, got, &mut out);
    }
    out
}

fn first_row_mut<'a>(
    value: &'a mut serde_json::Value,
    parts: &[&str],
) -> Option<&'a mut serde_json::Value> {
    match parts.split_first() {
        None => match value {
            serde_json::Value::Array(rows) => rows.first_mut(),
            serde_json::Value::Object(_) => Some(value),
            _ => None,
        },
        Some((&"*", rest)) => value
            .as_array_mut()?
            .iter_mut()
            .find_map(|row| first_row_mut(row, rest)),
        Some((key, rest)) => first_row_mut(value.get_mut(*key)?, rest),
    }
}

fn first_row<'a>(value: &'a serde_json::Value, parts: &[&str]) -> Option<&'a serde_json::Value> {
    match parts.split_first() {
        None => match value {
            serde_json::Value::Array(rows) => rows.first(),
            serde_json::Value::Object(_) => Some(value),
            _ => None,
        },
        Some((&"*", rest)) => value
            .as_array()?
            .iter()
            .find_map(|row| first_row(row, rest)),
        Some((key, rest)) => first_row(value.get(*key)?, rest),
    }
}

/// A value of `column`'s kind no model would drop as a default: the last
/// option (defaults come first), a row id of the referenced table, or a non-zero, non-empty value.
fn sample(
    plan: &serde_json::Value,
    tables: &[Table],
    column: &Column,
) -> Option<serde_json::Value> {
    use serde_json::json;
    if column.read_only {
        return None;
    }
    if let Some(option) = column.options.last() {
        return Some(json!(option.value));
    }
    if !column.reference.is_empty() {
        let table = tables.iter().find(|t| t.id == column.reference)?;
        return rows(plan, table.path).first()?.get("id").cloned();
    }
    Some(match column.kind {
        "text" => json!("sample"),
        "integer" | "minutes" => json!(2),
        "number" => json!(1.5),
        "boolean" => json!(true),
        "date_time" => json!("2027-01-01T08:00"),
        "date" => json!("2027-01-01"),
        "location" => json!([45.0, 7.0]),
        "object" => serde_json::Value::Object(
            column
                .fields
                .iter()
                .filter_map(|f| Some((f.name.to_string(), sample(plan, tables, f)?)))
                .collect(),
        ),
        "list" => json!([sample(plan, tables, column.fields.first()?)?]),
        _ => return None,
    })
}

/// Records where `got` lacks a key `sent` set, recursing into objects and
/// the first item of lists.
fn lost(
    at: &str,
    sent: &serde_json::Value,
    got: Option<&serde_json::Value>,
    out: &mut Vec<String>,
) {
    let Some(got) = got else {
        out.push(format!("{at}: the model does not read it"));
        return;
    };
    match sent {
        serde_json::Value::Object(fields) => {
            for (key, value) in fields {
                lost(&format!("{at}.{key}"), value, got.get(key), out);
            }
        }
        serde_json::Value::Array(items) => {
            if let Some(first) = items.first() {
                lost(at, first, got.as_array().and_then(|a| a.first()), out);
            }
        }
        _ => {}
    }
}

/// Keys the data carries inside an object or a list of objects that `column` does
/// not describe: the editor would show them nowhere.
fn undescribed(at: &str, column: &Column, values: &[&serde_json::Value], out: &mut Vec<String>) {
    let (shape, objects): (&Column, Vec<&serde_json::Value>) = match column.kind {
        "object" => (column, values.to_vec()),
        "list" => match column.fields.first() {
            Some(item) if item.kind == "object" => (
                item,
                values
                    .iter()
                    .flat_map(|v| v.as_array().into_iter().flatten())
                    .collect(),
            ),
            _ => return,
        },
        _ => return,
    };
    let mut missing: Vec<&str> = objects
        .iter()
        .flat_map(|o| o.as_object().into_iter().flatten().map(|(k, _)| k.as_str()))
        .filter(|k| !shape.fields.iter().any(|f| f.name == *k))
        .collect();
    missing.sort_unstable();
    missing.dedup();
    for key in missing {
        out.push(format!(
            "{at}: the data has {key}, the table does not describe it"
        ));
    }
    for field in &shape.fields {
        let inner: Vec<&serde_json::Value> =
            objects.iter().filter_map(|o| o.get(field.name)).collect();
        undescribed(&format!("{at}.{}", field.name), field, &inner, out);
    }
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
            mismatches(&plan, &[table], &[]),
            ["shifts.day: missing from 1 rows"]
        );
        let plan = serde_json::json!({"crew": [{"id": "a", "skills": [{"name": "x", "level": 1, "speed": 2}]}]});
        let table = Table {
            id: "crew",
            label: "Crew",
            one: "member",
            path: "crew",
            parent: "",
            title: "",
            columns: vec![list(
                "skills",
                "Skills",
                col("skill", "Skill", "object").fields(vec![
                    col("name", "Name", "text"),
                    col("level", "Level", "integer"),
                ]),
            )],
        };
        assert_eq!(
            mismatches(&plan, &[table], &[]),
            ["crew.skills: the data has speed, the table does not describe it"]
        );
        // A field of the row itself that no column shows is caught too, and a
        // top-level key the solver writes is left alone when named.
        let plan = serde_json::json!({
            "crew": [{"id": "a", "home": [1, 2]}],
            "window": {"start": "2027-01-01T00:00"},
            "unplanned": ["x"],
            "weights": {}
        });
        let crew = Table {
            id: "crew",
            label: "Crew",
            one: "member",
            path: "crew",
            parent: "",
            title: "",
            columns: vec![col("home", "Home", "location")],
        };
        assert_eq!(
            mismatches(&plan, std::slice::from_ref(&crew), &["unplanned"]),
            ["the plan has window, no table holds it"]
        );
        let window = Table {
            id: "window",
            label: "Planning window",
            one: "window",
            path: "window",
            parent: "",
            title: "",
            columns: vec![],
        };
        assert_eq!(rows(&plan, "window").len(), 1);
        assert_eq!(
            mismatches(&plan, &[crew, window], &["unplanned"]),
            ["window: the data has start, the table does not describe it"]
        );
        let kind = serde_json::to_value(choice("kind", "Kind", &[("A", "a"), ("B", "b")])).unwrap();
        assert_eq!(kind["options"][1]["value"], "B");
        // Every column gets a sample; one the model drops (a misspelt key, a
        // field it never reads) is named, down to an object's field.
        let plan = serde_json::json!({"crew": [{"id": "a"}, {"id": "b"}]});
        let crew = Table {
            id: "crew",
            label: "Crew",
            one: "member",
            path: "crew",
            parent: "",
            title: "",
            columns: vec![
                col("rating", "Rating", "integer").optional(),
                col("ratnig", "Misspelt", "integer").optional(),
                col("buddy", "Buddy", "text").reference("crew").optional(),
                col("home", "Home", "object")
                    .fields(vec![
                        col("lat", "Lat", "number"),
                        col("lon", "Lon", "number"),
                    ])
                    .optional(),
                col("idle", "Idle", "minutes").read_only(),
            ],
        };
        let mut seen = serde_json::Value::Null;
        let problems = unread(&plan, &[crew], |mut filled| {
            seen = filled["crew"][0].clone();
            let row = filled["crew"][0].as_object_mut().unwrap();
            row.remove("ratnig");
            row["home"].as_object_mut().unwrap().remove("lon");
            Ok(filled)
        });
        assert_eq!(seen["buddy"], "a");
        assert_eq!(seen["rating"], 2);
        assert!(seen.get("idle").is_none());
        assert_eq!(
            problems,
            [
                "crew.ratnig: the model does not read it",
                "crew.home.lon: the model does not read it"
            ]
        );
        let computed = serde_json::to_value(col("idle", "Idle", "minutes").read_only()).unwrap();
        assert_eq!(
            (computed["read_only"].clone(), computed["optional"].clone()),
            (true.into(), true.into())
        );
        let window = windows("timeWindows", "Time windows", &[("minStartTime", "From")]);
        assert_eq!(
            serde_json::to_value(window).unwrap()["fields"][0]["fields"][0]["kind"],
            "date_time"
        );
    }
}
