//! Busy periods of the day: the hours travel takes longer, and how much longer.
//!
//! A routing plan lists its busy periods (`traffic`): an id, the wall-clock
//! hours it runs (`from` to `to`, past midnight when `to` comes first), the
//! days of the week it runs on (every day when none are listed) and, for road
//! travel, the slowdowns that describe it. The host measures the travel of each
//! period on the roads with those slowdowns ([`crate::roads::Roads::matrix`]);
//! a dataset that brings its own travel matrix brings one per period instead.
//!
//! A leg is driven through the periods it crosses ([`travel_seconds`]): each
//! second spent inside a period covers the share of the leg that period's
//! travel time allows, so leaving later never means arriving earlier.

use serde::{Deserialize, Serialize};
use solverforge_maps::{BoundingBox, RoadClass, Slowdown as MapSlowdown};

use crate::data::{choice, col, list, Table};

/// Seconds in one day.
const DAY: i64 = 86_400;

/// Day of the week a busy period runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

/// A busy period of the day.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Traffic {
    pub id: String,
    /// When it starts, `HH:MM` wall-clock time.
    pub from: String,
    /// When it ends, `HH:MM`; before `from` when it runs past midnight.
    pub to: String,
    /// The days it starts on; every day when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub days: Vec<Weekday>,
    /// For travel measured on the roads: which roads are slower, and how much.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slowdowns: Vec<Slowdown>,
}

/// Roads that take `factor` times as long to drive during a busy period: the
/// listed road classes (all when none are), inside `area` when one is given.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Slowdown {
    pub factor: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roads: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area: Option<Area>,
}

/// A rectangle of the map, in degrees.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Area {
    pub south: f64,
    pub west: f64,
    pub north: f64,
    pub east: f64,
}

/// The travel of one busy period: seconds and meters between the matrix's
/// locations, `null` where no road leads. Measured on the roads, it records the
/// slowdowns it was measured with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelPeriod {
    pub id: String,
    pub seconds: Vec<Vec<Option<i64>>>,
    pub meters: Vec<Vec<Option<i64>>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slowdowns: Vec<Slowdown>,
}

/// The busy periods as a table of the page's data editor.
pub fn table() -> Table {
    const DAYS: [(&str, &str); 7] = [
        ("monday", "Monday"),
        ("tuesday", "Tuesday"),
        ("wednesday", "Wednesday"),
        ("thursday", "Thursday"),
        ("friday", "Friday"),
        ("saturday", "Saturday"),
        ("sunday", "Sunday"),
    ];
    const ROADS: [(&str, &str); 10] = [
        ("motorway", "Motorways"),
        ("trunk", "Trunk roads"),
        ("primary", "Primary roads"),
        ("secondary", "Secondary roads"),
        ("tertiary", "Tertiary roads"),
        ("residential", "Residential streets"),
        ("unclassified", "Unclassified roads"),
        ("service", "Service roads"),
        ("living_street", "Living streets"),
        ("other", "Other roads"),
    ];
    Table {
        id: "traffic",
        label: "Busy periods",
        one: "busy period",
        path: "traffic",
        parent: "",
        title: "id",
        columns: vec![
            col("id", "Name", "text"),
            col("from", "From", "text").unit("HH:MM"),
            col("to", "Until", "text").unit("HH:MM"),
            list("days", "On", choice("day", "Day", &DAYS)),
            list(
                "slowdowns",
                "Slower roads",
                col("slowdown", "Slowdown", "object").fields(vec![
                    col("factor", "Takes", "number").unit("×"),
                    list("roads", "Roads", choice("road", "Road", &ROADS)),
                    col("area", "Only in", "object")
                        .fields(vec![
                            col("south", "South", "number").unit("°"),
                            col("west", "West", "number").unit("°"),
                            col("north", "North", "number").unit("°"),
                            col("east", "East", "number").unit("°"),
                        ])
                        .optional(),
                ]),
            ),
        ],
    }
}

/// Road names a slowdown may list.
pub fn road_names() -> Vec<&'static str> {
    RoadClass::ALL.iter().map(|class| class.as_str()).collect()
}

/// The slowdowns of `period` as solverforge-maps applies them, or why they
/// cannot be.
pub fn map_slowdowns(period: &Traffic) -> Result<Vec<MapSlowdown>, String> {
    period
        .slowdowns
        .iter()
        .map(|rule| {
            let mut slowdown = MapSlowdown::new(rule.factor).map_err(|_| {
                format!(
                    "busy period {}: a slowdown's factor must be a number above zero, not {}",
                    period.id, rule.factor
                )
            })?;
            if !rule.roads.is_empty() {
                let mut classes = Vec::with_capacity(rule.roads.len());
                for name in &rule.roads {
                    classes.push(RoadClass::parse(name).ok_or_else(|| {
                        format!(
                            "busy period {}: {name:?} is not a road class; one of {}",
                            period.id,
                            road_names().join(", ")
                        )
                    })?);
                }
                slowdown = slowdown.roads(classes);
            }
            if let Some(area) = rule.area {
                let bbox = BoundingBox::try_new(area.south, area.west, area.north, area.east)
                    .map_err(|error| {
                        format!(
                            "busy period {}: the slowdown's area is not a map rectangle: {error}",
                            period.id
                        )
                    })?;
                slowdown = slowdown.area(bbox);
            }
            Ok(slowdown)
        })
        .collect()
}

/// The periods whose travel the roads measure: those with slowdowns, by id
/// and slowdowns, the way [`TravelPeriod`]s measured on the roads record them.
pub fn slowed_periods(traffic: &[Traffic]) -> Vec<(&str, &[Slowdown])> {
    traffic
        .iter()
        .filter(|t| !t.slowdowns.is_empty())
        .map(|t| (t.id.as_str(), t.slowdowns.as_slice()))
        .collect()
}

/// A stretch of time one busy period runs: `[start, end)` in seconds, and the
/// period's index in the plan's `traffic`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BusyWindow {
    pub start: i64,
    pub end: i64,
    pub period: usize,
}

/// `HH:MM` as seconds after midnight; `24:00` is the end of the day.
fn clock(text: &str) -> Option<i64> {
    let (h, m) = text.split_once(':')?;
    if h.len() != 2 || m.len() != 2 || !(h.bytes().chain(m.bytes())).all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (h, m): (i64, i64) = (h.parse().ok()?, m.parse().ok()?);
    (m < 60 && (h < 24 || (h == 24 && m == 0))).then_some(h * 3_600 + m * 60)
}

/// Every stretch of time the busy periods run, from day `first_day` to day
/// `last_day` (days since 1970-01-01, both included), in time order; or what is
/// wrong with them: a malformed hour, a period of no length, two with one id,
/// or two that overlap.
pub fn windows(
    traffic: &[Traffic],
    first_day: i64,
    last_day: i64,
) -> Result<Vec<BusyWindow>, String> {
    let mut out = Vec::new();
    for (index, period) in traffic.iter().enumerate() {
        if traffic[..index].iter().any(|t| t.id == period.id) {
            return Err(format!("two busy periods are named {}", period.id));
        }
        let hour = |text: &str| {
            clock(text).ok_or_else(|| {
                format!(
                    "busy period {}: {text:?} is not an HH:MM time of day",
                    period.id
                )
            })
        };
        let (from, to) = (hour(&period.from)?, hour(&period.to)?);
        if from == to {
            return Err(format!(
                "busy period {} starts and ends at {}: it has no length",
                period.id, period.from
            ));
        }
        let length = if to > from {
            to - from
        } else {
            to + DAY - from
        };
        for day in first_day..=last_day {
            // 1970-01-01 was a Thursday.
            let weekday = (day + 3).rem_euclid(7) as u8;
            if period.days.is_empty() || period.days.iter().any(|d| *d as u8 == weekday) {
                let start = day * DAY + from;
                out.push(BusyWindow {
                    start,
                    end: start + length,
                    period: index,
                });
            }
        }
    }
    out.sort_by_key(|w| (w.start, w.end));
    if let Some(pair) = out.windows(2).find(|pair| pair[0].end > pair[1].start) {
        return Err(format!(
            "busy periods {} and {} overlap",
            traffic[pair[0].period].id, traffic[pair[1].period].id
        ));
    }
    Ok(out)
}

/// Seconds a leg takes when it leaves at `depart`: `base` seconds outside every
/// busy period, `during(period)` seconds while that period runs. Each second
/// inside a stretch covers `1 / seconds` of the leg, so a leg that starts in a
/// quiet hour and runs into a busy one slows down from the moment it does,
/// and leaving later never arrives earlier.
pub fn travel_seconds(
    depart: i64,
    base: i64,
    windows: &[BusyWindow],
    during: impl Fn(usize) -> i64,
) -> i64 {
    if base <= 0 || windows.is_empty() {
        return base;
    }
    let mut at = depart as f64;
    let mut left = 1.0_f64;
    let mut next = windows.partition_point(|w| w.end <= depart);
    loop {
        let (until, seconds, inside) = match windows.get(next) {
            Some(w) if (w.start as f64) <= at => (w.end as f64, during(w.period), true),
            Some(w) => (w.start as f64, base, false),
            None => (f64::INFINITY, base, false),
        };
        let seconds = seconds.max(1) as f64;
        let need = left * seconds;
        if at + need <= until {
            at += need;
            break;
        }
        left -= (until - at) / seconds;
        at = until;
        if inside {
            next += 1;
        }
    }
    (at - depart as f64).round() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn period(id: &str, from: &str, to: &str, days: Vec<Weekday>) -> Traffic {
        Traffic {
            id: id.into(),
            from: from.into(),
            to: to.into(),
            days,
            slowdowns: Vec::new(),
        }
    }

    // 2027-02-01 is a Monday: day 20_850 since 1970-01-01.
    const MONDAY: i64 = 20_850;

    #[test]
    fn a_period_runs_on_its_days_and_past_midnight() {
        let rush = period("rush", "07:00", "09:00", vec![Weekday::Monday]);
        let night = period("night", "22:00", "06:00", Vec::new());
        let all = windows(&[rush, night], MONDAY, MONDAY + 1).unwrap();
        let at = |h: i64| MONDAY * DAY + h * 3_600;
        assert_eq!(
            all,
            vec![
                BusyWindow {
                    start: at(7),
                    end: at(9),
                    period: 0
                },
                BusyWindow {
                    start: at(22),
                    end: at(30),
                    period: 1
                },
                BusyWindow {
                    start: at(46),
                    end: at(54),
                    period: 1
                },
            ]
        );
    }

    #[test]
    fn malformed_empty_twin_and_overlapping_periods_are_refused() {
        let check = |traffic: Vec<Traffic>| windows(&traffic, MONDAY, MONDAY).unwrap_err();
        assert!(check(vec![period("a", "7:00", "09:00", vec![])]).contains("HH:MM"));
        assert!(check(vec![period("a", "07:00", "24:30", vec![])]).contains("HH:MM"));
        assert!(check(vec![period("a", "08:00", "08:00", vec![])]).contains("no length"));
        let twins = vec![
            period("a", "07:00", "08:00", vec![]),
            period("a", "12:00", "13:00", vec![]),
        ];
        assert!(check(twins).contains("two busy periods"));
        let overlap = vec![
            period("a", "07:00", "09:00", vec![]),
            period("b", "08:30", "10:00", vec![]),
        ];
        assert_eq!(check(overlap), "busy periods a and b overlap");
        let touching = vec![
            period("a", "07:00", "09:00", vec![]),
            period("b", "09:00", "10:00", vec![]),
        ];
        assert!(windows(&touching, MONDAY, MONDAY).is_ok());
    }

    #[test]
    fn a_leg_slows_down_from_the_moment_it_enters_a_busy_period() {
        let at = |h: i64, m: i64| MONDAY * DAY + h * 3_600 + m * 60;
        let rush = [BusyWindow {
            start: at(8, 0),
            end: at(9, 0),
            period: 0,
        }];
        // 20 quiet minutes, 40 during the rush.
        let during = |_| 2_400;
        assert_eq!(travel_seconds(at(6, 0), 1_200, &rush, during), 1_200);
        assert_eq!(travel_seconds(at(8, 10), 1_200, &rush, during), 2_400);
        // Leaves 07:50: half the leg by 08:00, the other half takes 20 rush minutes.
        assert_eq!(travel_seconds(at(7, 50), 1_200, &rush, during), 1_800);
        // Leaves 08:50: a quarter of the leg in 10 rush minutes, the rest in 15 quiet ones.
        assert_eq!(travel_seconds(at(8, 50), 1_200, &rush, during), 1_500);
    }

    #[test]
    fn leaving_later_never_arrives_earlier() {
        let at = |m: i64| MONDAY * DAY + m * 60;
        let rush = [BusyWindow {
            start: at(480),
            end: at(540),
            period: 0,
        }];
        let mut last = i64::MIN;
        for minute in 400..600 {
            let arrival = at(minute) + travel_seconds(at(minute), 1_200, &rush, |_| 3_600);
            assert!(
                arrival >= last,
                "leaving at minute {minute} arrives earlier"
            );
            last = arrival;
        }
    }

    #[test]
    fn the_table_offers_every_road_class() {
        let table = table();
        let slowdown = &table.columns[4].fields[0];
        let roads: Vec<&str> = slowdown.fields[1].fields[0]
            .options
            .iter()
            .map(|o| o.value.as_str())
            .collect();
        assert_eq!(roads, road_names());
    }

    #[test]
    fn slowdowns_name_known_road_classes_and_positive_factors() {
        let mut rush = period("rush", "07:00", "09:00", vec![]);
        rush.slowdowns = vec![Slowdown {
            factor: 1.5,
            roads: vec!["primary".into()],
            area: None,
        }];
        assert_eq!(map_slowdowns(&rush).unwrap().len(), 1);
        rush.slowdowns[0].roads = vec!["highway".into()];
        assert!(map_slowdowns(&rush)
            .unwrap_err()
            .contains("not a road class"));
        rush.slowdowns[0] = Slowdown {
            factor: 0.0,
            roads: vec![],
            area: None,
        };
        assert!(map_slowdowns(&rush).unwrap_err().contains("above zero"));
        rush.slowdowns[0] = Slowdown {
            factor: 2.0,
            roads: vec![],
            area: Some(Area {
                south: 45.1,
                west: 7.0,
                north: 45.0,
                east: 7.1,
            }),
        };
        assert!(map_slowdowns(&rush).unwrap_err().contains("area"));
    }
}
