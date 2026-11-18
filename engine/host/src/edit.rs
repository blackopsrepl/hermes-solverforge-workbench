//! A model's replanning vocabulary: the changes a planner makes to a plan
//! ("a technician is off on Tuesday", "add a visit"), each compiled into JSON
//! Patch operations that address records by id (`/visits/[id=V7]`).
//!
//! [`crate::Model::edits`] lists the changes a plan offers, with the choices
//! for each field read from that plan; [`crate::Model::edit`] compiles one
//! filled-in change. The workbench applies the operations as a new dataset, so
//! a change made on the page and one made by the agent are the same patch.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::protocol::EngineError;

/// One change a plan offers.
#[derive(Debug, Clone, Serialize)]
pub struct Edit {
    pub id: &'static str,
    pub label: String,
    pub description: String,
    pub fields: Vec<Field>,
}

/// One value a change needs.
#[derive(Debug, Clone, Serialize)]
pub struct Field {
    pub name: &'static str,
    pub label: String,
    #[serde(flatten)]
    pub kind: Kind,
    /// A field that may be left empty.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub optional: bool,
}

/// What a field holds, as the page edits it.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Kind {
    /// One of the listed values.
    Choice { options: Vec<Choice> },
    /// Free text.
    Text,
    /// A whole number of minutes.
    Minutes,
    /// A local date-time, `YYYY-MM-DDTHH:MM`.
    DateTime,
    /// A place, `[latitude, longitude]`.
    Location,
}

/// One option of a choice field.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Choice {
    pub value: String,
    pub label: String,
}

impl Choice {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
        }
    }
}

impl Field {
    pub fn choice(name: &'static str, label: &str, options: Vec<Choice>) -> Self {
        Self::new(name, label, Kind::Choice { options })
    }
    pub fn new(name: &'static str, label: &str, kind: Kind) -> Self {
        Self {
            name,
            label: label.into(),
            kind,
            optional: false,
        }
    }
    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }
}

/// A filled-in change, as a request carries it.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditRequest {
    pub id: String,
    #[serde(default)]
    pub args: Value,
}

/// A change compiled: the operations, and a sentence saying what they do.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Compiled {
    pub operations: Vec<Value>,
    pub summary: String,
}

/// The arguments of one change, read with errors that name the field.
pub struct Args<'a> {
    edit: &'a str,
    args: &'a Value,
}

impl<'a> Args<'a> {
    pub fn new(edit: &'a str, args: &'a Value) -> Self {
        Self { edit, args }
    }

    fn missing(&self, name: &str, what: &str) -> EngineError {
        EngineError::new(
            "invalid_edit",
            format!("{}: {name} must be {what}", self.edit),
        )
    }

    fn present(&self, name: &str) -> Option<&'a Value> {
        self.args
            .get(name)
            .filter(|v| !v.is_null() && v != &&json!(""))
    }

    pub fn text(&self, name: &str) -> Result<&'a str, EngineError> {
        self.present(name)
            .and_then(Value::as_str)
            .ok_or_else(|| self.missing(name, "given"))
    }

    pub fn text_opt(&self, name: &str) -> Option<&'a str> {
        self.present(name).and_then(Value::as_str)
    }

    /// A choice, checked against the options the change offered.
    pub fn choice(&self, name: &str, options: &[Choice]) -> Result<&'a str, EngineError> {
        let value = self.text(name)?;
        if options.iter().any(|o| o.value == value) {
            Ok(value)
        } else {
            Err(self.missing(name, "one of the offered options"))
        }
    }

    pub fn minutes(&self, name: &str) -> Result<i64, EngineError> {
        self.present(name)
            .and_then(Value::as_i64)
            .filter(|m| *m > 0)
            .ok_or_else(|| self.missing(name, "a positive number of minutes"))
    }

    pub fn date_time(&self, name: &str) -> Result<String, EngineError> {
        let text = self.text(name)?;
        let ok = text.len() == 16
            && text.as_bytes()[10] == b'T'
            && date(&text[..10]).is_some()
            && matches!(text[11..].split_once(':'), Some((h, m))
                if h.len() == 2 && m.len() == 2
                    && h.parse::<u8>().is_ok_and(|h| h < 24)
                    && m.parse::<u8>().is_ok_and(|m| m < 60));
        if ok {
            Ok(text.to_string())
        } else {
            Err(self.missing(name, "a local date-time such as 2027-02-02T08:00"))
        }
    }

    pub fn location(&self, name: &str) -> Result<[f64; 2], EngineError> {
        let point = self
            .present(name)
            .and_then(|v| serde_json::from_value::<[f64; 2]>(v.clone()).ok())
            .filter(|p| p[0].abs() <= 90.0 && p[1].abs() <= 180.0);
        point.ok_or_else(|| self.missing(name, "a [latitude, longitude] place"))
    }
}

/// The record path segment that addresses a record by id.
pub fn by_id(id: &str) -> String {
    format!("[id={}]", id.replace('~', "~0").replace('/', "~1"))
}

/// An id not yet used by any of `taken`, from `stem`: `NEW-1`, `NEW-2`, ...
pub fn fresh_id<'a>(stem: &str, taken: impl Iterator<Item = &'a str> + Clone) -> String {
    (1..)
        .map(|n| format!("{stem}-{n}"))
        .find(|id| !taken.clone().any(|t| t == id))
        .expect("an unused id")
}

/// `(year, month, day)` of a valid `YYYY-MM-DD` date.
fn date(text: &str) -> Option<(i64, u32, u32)> {
    let mut parts = text.splitn(3, '-');
    let (y, m, d) = (parts.next()?, parts.next()?, parts.next()?);
    if (y.len(), m.len(), d.len()) != (4, 2, 2) {
        return None;
    }
    let (y, m, d): (i64, u32, u32) = (y.parse().ok()?, m.parse().ok()?, d.parse().ok()?);
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    ((1..=12).contains(&m) && d >= 1 && d <= days[m as usize - 1]).then_some((y, m, d))
}

/// The `YYYY-MM-DD` date after `text`, or None when `text` is not a date.
pub fn next_day(text: &str) -> Option<String> {
    let (y, m, d) = date(text)?;
    let next = if date(&format!("{y:04}-{m:02}-{:02}", d + 1)).is_some() {
        (y, m, d + 1)
    } else if m < 12 {
        (y, m + 1, 1)
    } else {
        (y + 1, 1, 1)
    };
    Some(format!("{:04}-{:02}-{:02}", next.0, next.1, next.2))
}

/// The operation that appends `value` to the array at `path`, creating the
/// array when the record has none yet (`existing` is its current length).
pub fn append(path: &str, existing: usize, value: Value) -> Value {
    if existing == 0 {
        json!({"op": "add", "path": path, "value": [value]})
    } else {
        json!({"op": "add", "path": format!("{path}/-"), "value": value})
    }
}

/// `Mon 1 Feb` for a `YYYY-MM-DD` date.
pub fn day_label(text: &str) -> String {
    let Some((y, m, d)) = date(text) else {
        return text.to_string();
    };
    // Days since 1970-01-01 (a Thursday), by the civil-from-days inverse.
    let (y2, m2) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * m2 as i64 + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    const WEEK: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    format!(
        "{} {d} {}",
        WEEK[days.rem_euclid(7) as usize],
        MONTHS[m as usize - 1]
    )
}

/// Refusal for a change the model does not offer.
pub fn unknown(model: &str, id: &str, edits: &[Edit]) -> EngineError {
    EngineError::new(
        "unknown_edit",
        format!("model {model} offers no change {id:?}"),
    )
    .with_detail(json!({ "edits": edits.iter().map(|e| e.id).collect::<Vec<_>>() }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_are_read_with_errors_that_name_the_field() {
        let args = json!({"who": "e1", "at": "2027-02-02T08:00", "long": 0, "where": [45.0, 7.6]});
        let a = Args::new("off", &args);
        assert_eq!(a.choice("who", &[Choice::new("e1", "Ada")]).unwrap(), "e1");
        assert_eq!(a.date_time("at").unwrap(), "2027-02-02T08:00");
        assert_eq!(a.location("where").unwrap(), [45.0, 7.6]);
        assert_eq!(
            a.minutes("long").err().unwrap().message,
            "off: long must be a positive number of minutes"
        );
        assert_eq!(
            a.choice("who", &[Choice::new("e2", "Bo")])
                .err()
                .unwrap()
                .code,
            "invalid_edit"
        );
        assert!(a.text_opt("nothing").is_none());
    }

    #[test]
    fn ids_and_days_read_as_records_and_words() {
        assert_eq!(by_id("a/b"), "[id=a~1b]");
        assert_eq!(fresh_id("NEW", ["NEW-1", "X"].into_iter()), "NEW-2");
        assert_eq!(day_label("2027-02-02"), "Tue 2 Feb");
        assert_eq!(day_label("2027-03-01"), "Mon 1 Mar");
        assert_eq!(day_label("2024-02-29"), "Thu 29 Feb");
        assert_eq!(day_label("1970-01-01"), "Thu 1 Jan");
        assert_eq!(next_day("2027-02-28").as_deref(), Some("2027-03-01"));
        assert_eq!(next_day("2024-02-28").as_deref(), Some("2024-02-29"));
        assert_eq!(next_day("2027-12-31").as_deref(), Some("2028-01-01"));
        assert_eq!(
            append("/a", 0, json!(1)),
            json!({"op": "add", "path": "/a", "value": [1]})
        );
        assert_eq!(
            append("/a", 2, json!(1)),
            json!({"op": "add", "path": "/a/-", "value": 1})
        );
        let a = json!({"t": "2027-02-30T08:00"});
        assert!(Args::new("x", &a).date_time("t").is_err());
    }
}
