//! Building blocks shared by the dataset's records.

use serde::{Deserialize, Serialize};

/// Which shifts a rule, span or pattern step looks at, by their tags.
///
/// Empty `with_tags` matches every shift. With `match_all` a shift must carry
/// every tag of `with_tags` (and is excluded only when it carries every tag of
/// `without_tags`); otherwise one tag is enough.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShiftFilter {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub with_tags: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub without_tags: Vec<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub match_all: bool,
}

/// A stretch of local time, optionally limited to some shifts.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeSpan {
    /// Inclusive start, `YYYY-MM-DDTHH:MM`.
    pub from: String,
    /// Exclusive end, `YYYY-MM-DDTHH:MM`.
    pub to: String,
    /// Only shifts that pass this filter are affected.
    #[serde(default, flatten)]
    pub shifts: ShiftFilter,
    /// Importance of the span for soft scoring; 1 by default.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub weight: i64,
}

/// A skill held by an employee, or asked for by a shift.
///
/// Written either as a bare skill id or as an object with a level and, for an
/// employee's skill, the dates it is valid.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SkillRef {
    Id(String),
    Detailed(SkillDetail),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillDetail {
    pub id: String,
    /// The employee's level, or the minimum level the shift asks for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u32>,
    /// First moment the skill counts (employees only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// Moment the skill stops counting (employees only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
}

impl SkillRef {
    pub fn id(&self) -> &str {
        match self {
            SkillRef::Id(id) => id,
            SkillRef::Detailed(detail) => &detail.id,
        }
    }

    pub fn level(&self) -> u32 {
        match self {
            SkillRef::Id(_) => 0,
            SkillRef::Detailed(detail) => detail.level.unwrap_or(0),
        }
    }
}

/// A point on the map, in decimal degrees.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Place {
    pub lat: f64,
    pub lon: f64,
}

impl Eq for Place {}

impl std::hash::Hash for Place {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.lat.to_bits().hash(state);
        self.lon.to_bits().hash(state);
    }
}

impl Place {
    /// Great-circle distance in metres.
    pub fn metres_to(&self, other: &Place) -> f64 {
        let (lat1, lat2) = (self.lat.to_radians(), other.lat.to_radians());
        let dlat = lat2 - lat1;
        let dlon = (other.lon - self.lon).to_radians();
        let a = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
        2.0 * 6_371_000.0 * a.sqrt().asin()
    }
}

/// How firmly a range rule binds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Level {
    /// Breaking it makes the schedule infeasible.
    #[default]
    Required,
    /// Breaking it costs soft score.
    Preferred,
}

/// How a pattern or pairing rule wants the matched situation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Stance {
    /// It must happen.
    Required,
    /// It should happen.
    Preferred,
    /// It should not happen.
    Unwanted,
    /// It must not happen.
    Barred,
}

pub fn one() -> i64 {
    1
}

pub fn is_one(value: &i64) -> bool {
    *value == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_distance() {
        let brussels = Place {
            lat: 50.8503,
            lon: 4.3517,
        };
        let antwerp = Place {
            lat: 51.2194,
            lon: 4.4025,
        };
        let km = brussels.metres_to(&antwerp) / 1000.0;
        assert!((40.0..42.0).contains(&km), "{km}");
    }

    #[test]
    fn reads_both_skill_shapes() {
        let skills: Vec<SkillRef> =
            serde_json::from_str(r#"["nurse", {"id": "triage", "level": 2}]"#).unwrap();
        assert_eq!(skills[0].id(), "nurse");
        assert_eq!(skills[1].level(), 2);
    }
}
