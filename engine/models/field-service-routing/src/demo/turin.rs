//! A Turin building-services company over two weekdays: electricians,
//! plumbers, heating engineers, gas fitters and lift technicians serving
//! homes, offices and condominiums, with one remote lift job in the Aosta
//! valley that only a linked shift reaches.

use super::rng::Rng;
use crate::domain::Plan;
use crate::time::{format_date_time, parse_date_time};
use crate::wire::{
    Alignment, Break, BreakKind, Coordination, Cost, DateAdjuster, DelayTo, ItineraryItem, Limits,
    LinkedShift, Parameters, PlanningWindow, PoolingType, PreferredVehicle, Rate, RateUnit,
    ServicePart, ShiftLink, Skill, SkillRequirement, TemporarySkills, TemporaryTags, TimeWindow,
    TravelTimeAdjustment, Vehicle, VehicleShift, Visit, VisitDependency, VisitGroup,
};
use solverforge_workbench_host::traffic::{Area, Slowdown, Traffic, Weekday};

/// Monday 1 February 2027, midnight.
const DAY0: &str = "2027-02-01T00:00";
/// Weekdays, when the city's rush hours run.
const WEEKDAYS: [Weekday; 5] = [
    Weekday::Monday,
    Weekday::Tuesday,
    Weekday::Wednesday,
    Weekday::Thursday,
    Weekday::Friday,
];

fn names(roads: &[&str]) -> Vec<String> {
    roads.iter().map(|r| r.to_string()).collect()
}

fn at(day: i64, minute: i64) -> String {
    format_date_time(parse_date_time(DAY0).expect("demo epoch") + day * 86_400 + minute * 60)
}

/// A local date-time `minutes` later.
fn shifted(time: &str, minutes: i64) -> String {
    format_date_time(parse_date_time(time).expect("demo time") + minutes * 60)
}

fn window(day: i64, from: i64, to: i64) -> TimeWindow {
    TimeWindow {
        min_start_time: at(day, from),
        max_start_time: None,
        max_end_time: at(day, to),
    }
}

/// Neighbourhoods customers live and work in, by their centre.
const AREAS: [(&str, f64, f64); 16] = [
    ("Centro", 45.0703, 7.6869),
    ("Crocetta", 45.0569, 7.6676),
    ("San Salvario", 45.0558, 7.6826),
    ("Vanchiglia", 45.0745, 7.7010),
    ("Aurora", 45.0855, 7.6880),
    ("Barriera di Milano", 45.0951, 7.6971),
    ("San Donato", 45.0790, 7.6600),
    ("Parella", 45.0835, 7.6355),
    ("Pozzo Strada", 45.0660, 7.6270),
    ("Santa Rita", 45.0405, 7.6385),
    ("Mirafiori Nord", 45.0360, 7.6200),
    ("Lingotto", 45.0300, 7.6620),
    ("Nizza Millefonti", 45.0365, 7.6730),
    ("Borgo Po", 45.0620, 7.7060),
    ("Madonna di Campagna", 45.1020, 7.6750),
    ("Lucento", 45.0990, 7.6470),
];

/// The two depots technicians start from.
const DEPOTS: [[f64; 2]; 2] = [[45.0880, 7.6620], [45.0380, 7.6560]];

/// The north and south halves of the city, for coverage areas.
fn north() -> Vec<Vec<[f64; 2]>> {
    vec![vec![
        [45.0650, 7.5900],
        [45.1250, 7.5900],
        [45.1250, 7.7500],
        [45.0650, 7.7500],
    ]]
}

fn south() -> Vec<Vec<[f64; 2]>> {
    vec![vec![
        [45.0000, 7.5900],
        [45.0720, 7.5900],
        [45.0720, 7.7500],
        [45.0000, 7.7500],
    ]]
}

/// The remote lift job in the Aosta valley, beyond a day's round trip.
const REMOTE: [f64; 2] = [45.7370, 7.3210];

/// A technician and what they can do.
struct Crew {
    id: &'static str,
    name: &'static str,
    depot: usize,
    /// Skill, level, multiplier.
    skills: &'static [(&'static str, i64, Option<f64>)],
    tags: &'static [&'static str],
    rating: f64,
    /// Hours worked and available in the past month, for fairness.
    history: (i64, i64),
    area: Option<bool>,
}

const CREWS: [Crew; 30] = [
    Crew {
        id: "marco",
        name: "Marco Ferrero",
        depot: 0,
        skills: &[("electrical", 3, Some(0.8))],
        tags: &["ladder"],
        rating: 4.8,
        history: (150, 160),
        area: None,
    },
    Crew {
        id: "luca",
        name: "Luca Bianchi",
        depot: 0,
        skills: &[("electrical", 2, None)],
        tags: &["ladder"],
        rating: 4.2,
        history: (140, 160),
        area: Some(true),
    },
    Crew {
        id: "giulia",
        name: "Giulia Rossi",
        depot: 1,
        skills: &[("electrical", 2, None), ("hvac", 1, None)],
        tags: &["ladder"],
        rating: 4.5,
        history: (120, 160),
        area: None,
    },
    Crew {
        id: "paolo",
        name: "Paolo Gallo",
        depot: 1,
        skills: &[("electrical", 1, Some(1.25))],
        tags: &[],
        rating: 3.6,
        history: (100, 120),
        area: Some(false),
    },
    Crew {
        id: "sara",
        name: "Sara Conti",
        depot: 0,
        skills: &[("plumbing", 3, Some(0.85))],
        tags: &["van-large"],
        rating: 4.9,
        history: (155, 160),
        area: None,
    },
    Crew {
        id: "andrea",
        name: "Andrea Ricci",
        depot: 1,
        skills: &[("plumbing", 2, None)],
        tags: &["van-large"],
        rating: 4.0,
        history: (130, 160),
        area: Some(false),
    },
    Crew {
        id: "elena",
        name: "Elena Marino",
        depot: 0,
        skills: &[("plumbing", 2, None), ("gas", 2, None)],
        tags: &["van-large"],
        rating: 4.4,
        history: (145, 160),
        area: None,
    },
    Crew {
        id: "davide",
        name: "Davide Greco",
        depot: 1,
        skills: &[("plumbing", 1, Some(1.2))],
        tags: &[],
        rating: 3.5,
        history: (90, 120),
        area: None,
    },
    Crew {
        id: "chiara",
        name: "Chiara Bruno",
        depot: 0,
        skills: &[("hvac", 3, Some(0.8))],
        tags: &["ladder", "van-large"],
        rating: 4.7,
        history: (160, 160),
        area: None,
    },
    Crew {
        id: "matteo",
        name: "Matteo Esposito",
        depot: 1,
        skills: &[("hvac", 2, None), ("lift", 2, None)],
        tags: &["ladder"],
        rating: 4.1,
        history: (135, 160),
        area: None,
    },
    Crew {
        id: "francesca",
        name: "Francesca Romano",
        depot: 0,
        skills: &[("hvac", 2, None), ("electrical", 1, None)],
        tags: &[],
        rating: 4.3,
        history: (125, 160),
        area: Some(true),
    },
    Crew {
        id: "simone",
        name: "Simone Colombo",
        depot: 1,
        skills: &[("hvac", 1, Some(1.2)), ("lift", 2, None)],
        tags: &[],
        rating: 3.8,
        history: (110, 140),
        area: None,
    },
    Crew {
        id: "alessia",
        name: "Alessia Fontana",
        depot: 0,
        skills: &[("gas", 3, None), ("plumbing", 1, None)],
        tags: &[],
        rating: 4.6,
        history: (150, 160),
        area: None,
    },
    Crew {
        id: "roberto",
        name: "Roberto Moretti",
        depot: 1,
        skills: &[("gas", 2, None)],
        tags: &["van-large"],
        rating: 4.0,
        history: (140, 160),
        area: None,
    },
    Crew {
        id: "federica",
        name: "Federica Barbieri",
        depot: 0,
        skills: &[("gas", 1, None), ("hvac", 1, None)],
        tags: &[],
        rating: 3.9,
        history: (100, 160),
        area: None,
    },
    Crew {
        id: "stefano",
        name: "Stefano Lombardi",
        depot: 1,
        skills: &[("lift", 3, Some(0.85))],
        tags: &["van-large"],
        rating: 4.8,
        history: (158, 160),
        area: None,
    },
    Crew {
        id: "valentina",
        name: "Valentina Ferri",
        depot: 0,
        skills: &[("lift", 2, None)],
        tags: &[],
        rating: 4.2,
        history: (130, 160),
        area: None,
    },
    Crew {
        id: "giorgio",
        name: "Giorgio Costa",
        depot: 1,
        skills: &[("lift", 2, None), ("electrical", 2, None)],
        tags: &["ladder"],
        rating: 4.4,
        history: (145, 160),
        area: None,
    },
    Crew {
        id: "martina",
        name: "Martina Giordano",
        depot: 0,
        skills: &[("lift", 2, None), ("electrical", 1, None)],
        tags: &[],
        rating: 4.0,
        history: (120, 160),
        area: None,
    },
    Crew {
        id: "nicola",
        name: "Nicola Rinaldi",
        depot: 1,
        skills: &[("electrical", 3, None)],
        tags: &["ladder"],
        rating: 4.6,
        history: (150, 160),
        area: Some(false),
    },
    Crew {
        id: "ilaria",
        name: "Ilaria Caruso",
        depot: 0,
        skills: &[("plumbing", 2, None), ("hvac", 1, None)],
        tags: &["van-large"],
        rating: 4.1,
        history: (115, 160),
        area: Some(true),
    },
    Crew {
        id: "fabio",
        name: "Fabio Mancini",
        depot: 1,
        skills: &[("hvac", 2, None), ("gas", 1, None)],
        tags: &["van-large"],
        rating: 4.3,
        history: (140, 160),
        area: None,
    },
    Crew {
        id: "silvia",
        name: "Silvia Testa",
        depot: 0,
        skills: &[("electrical", 1, None), ("hvac", 1, None)],
        tags: &[],
        rating: 3.7,
        history: (80, 120),
        area: None,
    },
    Crew {
        id: "emanuele",
        name: "Emanuele Villa",
        depot: 1,
        skills: &[("plumbing", 3, None)],
        tags: &["van-large"],
        rating: 4.5,
        history: (152, 160),
        area: None,
    },
    Crew {
        id: "beatrice",
        name: "Beatrice Gatti",
        depot: 0,
        skills: &[("gas", 2, None), ("hvac", 2, None)],
        tags: &["ladder"],
        rating: 4.4,
        history: (138, 160),
        area: None,
    },
    Crew {
        id: "tommaso",
        name: "Tommaso Pellegrini",
        depot: 1,
        skills: &[("electrical", 2, None)],
        tags: &["ladder"],
        rating: 4.0,
        history: (125, 160),
        area: None,
    },
    Crew {
        id: "laura",
        name: "Laura Sala",
        depot: 0,
        skills: &[("plumbing", 2, None)],
        tags: &["ladder", "van-large"],
        rating: 4.2,
        history: (130, 160),
        area: None,
    },
    Crew {
        id: "riccardo",
        name: "Riccardo Leone",
        depot: 1,
        skills: &[("hvac", 3, None)],
        tags: &["ladder"],
        rating: 4.7,
        history: (155, 160),
        area: None,
    },
    Crew {
        id: "anna",
        name: "Anna Longo",
        depot: 0,
        skills: &[("electrical", 2, None), ("gas", 1, None)],
        tags: &["ladder"],
        rating: 4.1,
        history: (120, 160),
        area: None,
    },
    // A trainee: certified for gas on Tuesday, after Monday's exam.
    Crew {
        id: "pietro",
        name: "Pietro Marchetti",
        depot: 1,
        skills: &[("plumbing", 1, Some(1.3))],
        tags: &[],
        rating: 3.4,
        history: (60, 80),
        area: None,
    },
];

fn skills(list: &[(&str, i64, Option<f64>)]) -> Vec<Skill> {
    list.iter()
        .map(|&(name, level, multiplier)| Skill {
            name: name.into(),
            level: Some(level),
            multiplier,
        })
        .collect()
}

/// Day `day`'s shift of `crew`.
fn shift(crew: &Crew, day: i64, k: usize) -> VehicleShift {
    let depot = DEPOTS[crew.depot];
    let mut breaks = vec![Break {
        id: format!("{}-lunch-{day}", crew.id),
        kind: BreakKind::Floating,
        min_start_time: Some(at(day, 12 * 60)),
        max_end_time: Some(at(day, 14 * 60 + 30)),
        duration: Some(30),
        ..Break::default()
    }];
    // Valentina's Monday physiotherapy appointment, at the clinic.
    if crew.id == "valentina" && day == 0 {
        breaks.push(Break {
            id: "valentina-physio".into(),
            kind: BreakKind::Fixed,
            location: Some([45.0640, 7.6760]),
            start_time: Some(at(day, 15 * 60)),
            end_time: Some(at(day, 16 * 60)),
            ..Break::default()
        });
    }
    // Marco's Tuesday fixed toolbox meeting at the depot.
    if crew.id == "marco" && day == 1 {
        breaks.push(Break {
            id: "marco-meeting".into(),
            kind: BreakKind::Fixed,
            start_time: Some(at(day, 8 * 60)),
            end_time: Some(at(day, 8 * 60 + 30)),
            ..Break::default()
        });
    }
    let (start, soft_end, end) = match k % 3 {
        0 => (7 * 60 + 30, 16 * 60 + 30, 17 * 60 + 30),
        1 => (8 * 60, 17 * 60, 18 * 60),
        _ => (8 * 60 + 30, 17 * 60 + 30, 18 * 60 + 30),
    };
    VehicleShift {
        id: format!("{}-{}", crew.id, ["mon", "tue"][day as usize]),
        start_location: depot,
        end_location: None,
        min_start_time: Some(at(day, start)),
        min_first_visit_arrival_time: None,
        max_last_visit_departure_time: Some(at(day, end - 15)),
        max_soft_last_visit_departure_time: Some(at(day, soft_end - 15)),
        max_end_time: Some(at(day, end)),
        max_soft_end_time: Some(at(day, soft_end)),
        required_area: None,
        preferred_area: None,
        limits: Limits {
            max_travel_time_per_visit: Some(60),
            max_soft_travel_time: Some(150),
            max_travel_time: Some(300),
            max_service_duration: Some(9 * 60),
            max_soft_service_duration: Some(7 * 60),
            max_soft_visits: Some(8),
            max_visits: Some(12),
            skills: skills(crew.skills),
            tags: crew
                .tags
                .iter()
                .filter(|t| !(crew.id == "giulia" && day == 1 && **t == "ladder"))
                .map(|t| t.to_string())
                .collect(),
            required_breaks: breaks,
            temporary_skill_sets: if crew.id == "pietro" {
                vec![TemporarySkills {
                    start: at(1, 0),
                    end: at(2, 0),
                    skills: vec![Skill {
                        name: "gas".into(),
                        level: Some(1),
                        multiplier: Some(1.3),
                    }],
                }]
            } else {
                Vec::new()
            },
            // Giulia's ladder is lent out on Tuesday afternoon; Simone has
            // the large van on Monday morning only.
            temporary_tag_sets: match (crew.id, day) {
                ("giulia", 1) => vec![TemporaryTags {
                    start: at(1, 0),
                    end: at(1, 12 * 60),
                    tags: vec!["ladder".into()],
                }],
                ("simone", 0) => vec![TemporaryTags {
                    start: at(0, 0),
                    end: at(0, 12 * 60),
                    tags: vec!["van-large".into()],
                }],
                _ => Vec::new(),
            },
            movable_occupation_ratio_threshold: Some(0.6),
            cost: Some(Cost {
                fixed_cost: 25.0,
                rates: vec![
                    Rate {
                        duration: Some(8 * 60),
                        activation_cost: 0.0,
                        cost_per_unit: 32.0,
                        unit: RateUnit::Hour,
                    },
                    Rate {
                        duration: None,
                        activation_cost: 15.0,
                        cost_per_unit: 48.0,
                        unit: RateUnit::Hour,
                    },
                ],
            }),
            travel_time_adjustment: (crew.depot == 1).then_some(TravelTimeAdjustment {
                multiplier: 1.1,
                extra_time: 2,
            }),
            ..Limits::default()
        },
    }
}

fn near(rng: &mut Rng, lat: f64, lon: f64, meters: f64) -> [f64; 2] {
    let r = meters * (rng.below(1000) as f64 / 1000.0);
    let a = rng.below(6283) as f64 / 1000.0;
    [
        lat + r * a.cos() / 111_320.0,
        lon + r * a.sin() / (111_320.0 * lat.to_radians().cos()),
    ]
}

/// What a customer calls for.
#[derive(Clone, Copy)]
struct Job {
    name: &'static str,
    skill: &'static str,
    level: i64,
    minutes: i64,
    /// Minutes of the service that need the skill; the rest is fixed.
    skilled: i64,
    tag: Option<&'static str>,
}

/// How often each job is called for, in sixteenths.
const MIX: [u64; 9] = [3, 2, 3, 1, 2, 1, 2, 1, 1];

fn pick(rng: &mut Rng) -> Job {
    let mut roll = rng.below(MIX.iter().sum());
    for (job, &share) in JOBS.iter().zip(&MIX) {
        if roll < share {
            return *job;
        }
        roll -= share;
    }
    JOBS[0]
}

const JOBS: [Job; 9] = [
    Job {
        name: "Electrical repair",
        skill: "electrical",
        level: 1,
        minutes: 60,
        skilled: 45,
        tag: None,
    },
    Job {
        name: "Rewiring",
        skill: "electrical",
        level: 2,
        minutes: 120,
        skilled: 100,
        tag: Some("ladder"),
    },
    Job {
        name: "Leak repair",
        skill: "plumbing",
        level: 1,
        minutes: 60,
        skilled: 50,
        tag: None,
    },
    Job {
        name: "Boiler replacement",
        skill: "plumbing",
        level: 2,
        minutes: 150,
        skilled: 120,
        tag: Some("van-large"),
    },
    Job {
        name: "Heat pump service",
        skill: "hvac",
        level: 1,
        minutes: 75,
        skilled: 60,
        tag: None,
    },
    Job {
        name: "Air conditioning install",
        skill: "hvac",
        level: 2,
        minutes: 120,
        skilled: 90,
        tag: Some("ladder"),
    },
    Job {
        name: "Gas safety check",
        skill: "gas",
        level: 1,
        minutes: 45,
        skilled: 40,
        tag: None,
    },
    Job {
        name: "Gas line repair",
        skill: "gas",
        level: 2,
        minutes: 90,
        skilled: 75,
        tag: None,
    },
    Job {
        name: "Lift maintenance",
        skill: "lift",
        level: 2,
        minutes: 90,
        skilled: 80,
        tag: None,
    },
];

/// The technicians who may spend both days on the Aosta trip.
const TRAVELLERS: [&str; 2] = ["stefano", "giorgio"];

/// The first technician from the `k`-th on who can do `job` at `place`, so
/// a drawn required or preferred technician is one the job allows and who
/// is not possibly away in the Aosta valley.
fn able(k: u64, job: Job, place: [f64; 2]) -> &'static str {
    (0..CREWS.len())
        .map(|i| &CREWS[(k as usize + i) % CREWS.len()])
        .find(|crew| {
            !TRAVELLERS.contains(&crew.id)
                && crew
                    .skills
                    .iter()
                    .any(|&(skill, level, _)| skill == job.skill && level >= job.level)
                && job.tag.is_none_or(|tag| crew.tags.contains(&tag))
                && crew.area.is_none_or(|north_side| {
                    crate::prepare::inside(&if north_side { north() } else { south() }, place)
                })
        })
        .map_or(CREWS[k as usize].id, |crew| crew.id)
}

fn visit(rng: &mut Rng, id: String, day: i64, job: Job) -> Visit {
    let (area, lat, lon) = AREAS[rng.below(AREAS.len() as u64) as usize];
    let morning = rng.chance(50);
    let windows = match rng.below(10) {
        0..=3 if morning => vec![window(day, 8 * 60, 12 * 60 + 30)],
        0..=3 => vec![window(day, 13 * 60, 17 * 60 + 30)],
        4 => vec![
            window(day, 8 * 60, 10 * 60 + 30),
            window(day, 15 * 60, 17 * 60 + 30),
        ],
        _ => vec![window(day, 8 * 60, 17 * 60 + 30)],
    };
    // Every window holds the job with time to park and set up.
    let windows: Vec<TimeWindow> = windows
        .into_iter()
        .map(|w| {
            let earliest_end = shifted(&w.min_start_time, job.minutes + 45);
            TimeWindow {
                max_end_time: earliest_end.max(w.max_end_time),
                ..w
            }
        })
        .collect();
    let urgent = rng.chance(12);
    Visit {
        id,
        name: Some(format!("{} · {area}", job.name)),
        location: near(rng, lat, lon, 900.0),
        service_duration: job.minutes,
        service_duration_breakdown: vec![ServicePart {
            skill: job.skill.into(),
            duration: job.skilled,
        }],
        // An urgent call must be started within two hours of its window
        // opening, and done within four.
        latest_sla_end_time: urgent.then(|| shifted(&windows[0].min_start_time, 4 * 60)),
        time_windows: if urgent {
            windows
                .into_iter()
                .map(|w| TimeWindow {
                    max_start_time: Some(shifted(&w.min_start_time, 2 * 60)),
                    ..w
                })
                .collect()
        } else {
            windows
        },
        required_skills: vec![SkillRequirement {
            name: job.skill.into(),
            min_level: Some(job.level),
        }],
        required_tags: job.tag.map(|t| vec![t.to_string()]).unwrap_or_default(),
        priority: Some(if urgent { "2" } else { "5" }.into()),
        ..Visit::default()
    }
}

pub fn turin_two_days() -> Plan {
    let mut rng = Rng::new(0x544F_5249_4E4F);
    let mut vehicles: Vec<Vehicle> = CREWS
        .iter()
        .enumerate()
        .map(|(k, crew)| Vehicle {
            id: crew.id.into(),
            name: Some(crew.name.into()),
            shifts: (0..2).map(|day| shift(crew, day, k)).collect(),
            linked_shifts: Vec::new(),
            historical_time_utilized: crew.history.0 * 60,
            historical_time_capacity: crew.history.1 * 60,
            technician_rating: Some(crew.rating),
            required_area: crew.area.map(|n| if n { north() } else { south() }),
            preferred_area: None,
        })
        .collect();
    // Stefano and Giorgio may work Monday and Tuesday as one trip to the
    // Aosta valley, staying there overnight.
    for id in TRAVELLERS {
        let v = vehicles.iter_mut().find(|v| v.id == id).expect("crew");
        v.linked_shifts.push(LinkedShift {
            id: format!("{id}-aosta"),
            shifts: vec![ShiftLink {
                id: format!("{id}-mon-tue"),
                from_shift_id: format!("{id}-mon"),
                to_shift_id: format!("{id}-tue"),
            }],
            limits: Limits {
                skills: skills(if id == "stefano" {
                    &[("lift", 3, Some(0.85))]
                } else {
                    &[("lift", 2, None), ("electrical", 2, None)]
                }),
                required_breaks: (0..2)
                    .map(|day| Break {
                        id: format!("{id}-aosta-lunch-{day}"),
                        kind: BreakKind::Floating,
                        // On the trip, lunch whenever the job allows: after
                        // a full day's work on the lift.
                        min_start_time: Some(at(day, 11 * 60)),
                        max_end_time: Some(at(day, 18 * 60)),
                        duration: Some(30),
                        ..Break::default()
                    })
                    .collect(),
                cost: Some(Cost {
                    fixed_cost: 120.0,
                    rates: vec![Rate {
                        duration: None,
                        activation_cost: 0.0,
                        cost_per_unit: 32.0,
                        unit: RateUnit::Hour,
                    }],
                }),
                ..Limits::default()
            },
        });
    }
    // Lucento and Madonna di Campagna are Luca's preferred patch.
    vehicles[1].preferred_area = Some(vec![vec![
        [45.0900, 7.6300],
        [45.1150, 7.6300],
        [45.1150, 7.6900],
        [45.0900, 7.6900],
    ]]);

    let mut visits = Vec::new();
    for day in 0..2 {
        for k in 0..86 {
            let job = pick(&mut rng);
            let id = format!("V{}-{:03}", day + 1, k + 1);
            let mut v = visit(&mut rng, id, day, job);
            match rng.below(40) {
                0 => v.required_vehicles = vec![able(rng.below(30), job, v.location).into()],
                1 | 2 => v.prohibited_vehicles = vec!["paolo".into()],
                3..=5 => v.preferred_vehicles = vec![able(rng.below(30), job, v.location).into()],
                6 if job.skill == "plumbing" => {
                    v.preferred_vehicles_weights = vec![
                        PreferredVehicle {
                            id: "sara".into(),
                            weight: 10,
                        },
                        PreferredVehicle {
                            id: "emanuele".into(),
                            weight: 3,
                        },
                    ]
                }
                7 | 8 => v.preferred_time_windows = vec![window(day, 9 * 60, 11 * 60)],
                9 => v.service_duration_adjustment = 15,
                _ => {}
            }
            visits.push(v);
        }
    }
    // Routine maintenance any time over both days: movable, optional, and
    // worth a contract renewal.
    for k in 0..20 {
        let job = JOBS[[4, 6, 8][k % 3]];
        let (area, lat, lon) = AREAS[rng.below(AREAS.len() as u64) as usize];
        visits.push(Visit {
            id: format!("M-{:02}", k + 1),
            name: Some(format!("Annual {} · {area}", job.name.to_lowercase())),
            location: near(&mut rng, lat, lon, 900.0),
            service_duration: job.minutes,
            time_windows: vec![
                window(0, 8 * 60, 17 * 60 + 30),
                window(1, 8 * 60, 17 * 60 + 30),
            ],
            required_skills: vec![SkillRequirement {
                name: job.skill.into(),
                min_level: Some(1),
            }],
            priority: Some("opt-3".into()),
            profit: Some(60.0 + rng.below(140) as f64),
            ..Visit::default()
        });
    }
    // A condominium at Via Nizza: four flats at one address, then two
    // buildings of one block on Corso Francia that should be done in a row.
    for k in 0..4 {
        visits.push(Visit {
            id: format!("NIZZA-{}", k + 1),
            name: Some(format!("Gas safety check · Via Nizza 120, flat {}", k + 1)),
            location: [45.0480, 7.6735],
            service_duration: 30,
            time_windows: vec![window(0, 8 * 60, 17 * 60 + 30)],
            required_skills: vec![SkillRequirement {
                name: "gas".into(),
                min_level: Some(1),
            }],
            priority: Some("5".into()),
            ..Visit::default()
        });
    }
    for k in 0..3 {
        visits.push(Visit {
            id: format!("FRANCIA-{}", k + 1),
            name: Some(format!("Lighting check · Corso Francia {}", 200 + 10 * k)),
            location: [45.0760 + 0.0006 * k as f64, 7.6420 - 0.0012 * k as f64],
            service_duration: 40,
            time_windows: vec![window(1, 8 * 60, 17 * 60 + 30)],
            required_skills: vec![SkillRequirement {
                name: "electrical".into(),
                min_level: Some(1),
            }],
            priority: Some("5".into()),
            preferred_consecutive_group_id: Some("corso-francia".into()),
            ..Visit::default()
        });
    }
    // Inspections followed by the repair they find: a lift inspection then
    // its repair by the same technician on the next working day, and a
    // parts pick-up at the wholesaler followed straight away by a boiler
    // fitting.
    for k in 0..6 {
        let (area, lat, lon) = AREAS[rng.below(AREAS.len() as u64) as usize];
        let place = near(&mut rng, lat, lon, 700.0);
        visits.push(Visit {
            id: format!("INSPECT-{}", k + 1),
            name: Some(format!("Lift inspection · {area}")),
            location: place,
            service_duration: 45,
            time_windows: vec![window(0, 8 * 60, 16 * 60)],
            required_skills: vec![SkillRequirement {
                name: "lift".into(),
                min_level: Some(2),
            }],
            priority: Some("4".into()),
            ..Visit::default()
        });
        visits.push(Visit {
            id: format!("REPAIR-{}", k + 1),
            name: Some(format!("Lift repair · {area}")),
            location: place,
            service_duration: 90,
            time_windows: vec![window(1, 8 * 60, 17 * 60 + 30)],
            required_skills: vec![SkillRequirement {
                name: "lift".into(),
                min_level: Some(2),
            }],
            priority: Some("4".into()),
            visit_dependencies: vec![VisitDependency {
                id: format!("INSPECT-{}-then-repair", k + 1),
                preceding_visit: format!("INSPECT-{}", k + 1),
                min_delay_to: Some(DelayTo {
                    adjuster: DateAdjuster::NextWorkingDay,
                    increment: 1,
                    time: "08:00".into(),
                }),
                coordination: if k % 2 == 0 {
                    Coordination::SameVehicle
                } else {
                    Coordination::None
                },
                ..VisitDependency::default()
            }],
            ..Visit::default()
        });
    }
    for k in 0..4 {
        let day = (k % 2) as i64;
        let (area, lat, lon) = AREAS[rng.below(AREAS.len() as u64) as usize];
        visits.push(Visit {
            id: format!("PARTS-{}", k + 1),
            name: Some("Boiler pick-up · wholesaler, Strada del Drosso".into()),
            location: [45.0250, 7.6250],
            service_duration: 20,
            time_windows: vec![window(day, 8 * 60, 15 * 60)],
            required_tags: vec!["van-large".into()],
            priority: Some("5".into()),
            ..Visit::default()
        });
        visits.push(Visit {
            id: format!("FIT-{}", k + 1),
            name: Some(format!("Boiler fitting · {area}")),
            location: near(&mut rng, lat, lon, 700.0),
            service_duration: 120,
            time_windows: vec![window(day, 8 * 60, 17 * 60 + 30)],
            required_skills: vec![SkillRequirement {
                name: "plumbing".into(),
                min_level: Some(2),
            }],
            required_tags: vec!["van-large".into()],
            priority: Some("5".into()),
            visit_dependencies: vec![VisitDependency {
                id: format!("PARTS-{}-then-fit", k + 1),
                preceding_visit: format!("PARTS-{}", k + 1),
                max_delay: Some(90),
                coordination: Coordination::SameShift,
                pooling_type: Some(PoolingType::DisallowPooling),
                ..VisitDependency::default()
            }],
            ..Visit::default()
        });
    }
    // Two alternative quotes for one office: whichever technician goes,
    // only one visit happens.
    visits.push(Visit {
        id: "QUOTE-A".into(),
        name: Some("Heating quote, morning · Centro office".into()),
        location: [45.0712, 7.6855],
        service_duration: 45,
        time_windows: vec![window(0, 9 * 60, 12 * 60)],
        required_skills: vec![SkillRequirement {
            name: "hvac".into(),
            min_level: Some(2),
        }],
        priority: Some("opt-2".into()),
        exclusions: vec!["QUOTE-B".into()],
        profit: Some(80.0),
        ..Visit::default()
    });
    visits.push(Visit {
        id: "QUOTE-B".into(),
        name: Some("Heating quote, afternoon · Centro office".into()),
        location: [45.0712, 7.6855],
        service_duration: 45,
        time_windows: vec![window(1, 14 * 60, 17 * 60)],
        required_skills: vec![SkillRequirement {
            name: "hvac".into(),
            min_level: Some(2),
        }],
        priority: Some("opt-2".into()),
        exclusions: vec!["QUOTE-A".into()],
        profit: Some(80.0),
        ..Visit::default()
    });
    // The remote job: two full days on a hydro plant's lift in the Aosta
    // valley. Each day's work plus the drive there and back (about 75 minutes
    // each way on the roads) is longer than any shift, so it is done from a
    // trip that stays there overnight; the second day follows the first on
    // the same trip.
    for (part, minutes) in [(1, 460), (2, 460)] {
        visits.push(Visit {
            id: format!("AOSTA-LIFT-{part}"),
            name: Some(format!(
                "Lift overhaul, day {part} · hydro plant, Aosta valley"
            )),
            location: REMOTE,
            service_duration: minutes,
            time_windows: vec![TimeWindow {
                min_start_time: at(0, 8 * 60),
                max_start_time: None,
                max_end_time: at(1, 18 * 60),
            }],
            required_skills: vec![SkillRequirement {
                name: "lift".into(),
                min_level: Some(2),
            }],
            priority: Some("3".into()),
            visit_dependencies: if part == 2 {
                vec![VisitDependency {
                    id: "AOSTA-LIFT-next-day".into(),
                    preceding_visit: "AOSTA-LIFT-1".into(),
                    min_delay_to: Some(DelayTo {
                        adjuster: DateAdjuster::NextWorkingDay,
                        increment: 1,
                        time: "08:00".into(),
                    }),
                    coordination: Coordination::SameShift,
                    ..VisitDependency::default()
                }]
            } else {
                Vec::new()
            },
            ..Visit::default()
        });
    }

    // Two-technician jobs: a commercial heat pump and rooftop units.
    let mut groups = Vec::new();
    for k in 0..8 {
        let day = (k % 2) as i64;
        let (area, lat, lon) = AREAS[rng.below(AREAS.len() as u64) as usize];
        let place = near(&mut rng, lat, lon, 800.0);
        let member = |m: usize, level: i64| Visit {
            id: format!("JOINT-{}-{}", k + 1, ["a", "b"][m]),
            name: Some(format!("Rooftop unit install · {area}")),
            location: place,
            service_duration: 150,
            time_windows: vec![window(day, 8 * 60, 17 * 60 + 30)],
            required_skills: vec![SkillRequirement {
                name: if m == 0 { "hvac" } else { "electrical" }.into(),
                min_level: Some(level),
            }],
            required_tags: if m == 0 {
                vec!["ladder".into()]
            } else {
                Vec::new()
            },
            priority: Some("4".into()),
            ..Visit::default()
        };
        groups.push(VisitGroup {
            id: format!("JOINT-{}", k + 1),
            alignment: if k % 4 == 3 {
                Alignment::End
            } else {
                Alignment::Start
            },
            visits: vec![member(0, 1), member(1, 1)],
        });
    }

    // Luca has already been told about his first Monday visit: the first
    // Monday electrical repair in his part of town.
    let told = visits
        .iter()
        .find(|v| {
            v.id.starts_with("V1-")
                && v.required_skills.iter().all(|k| k.name == "electrical")
                && v.required_tags.is_empty()
                && v.required_vehicles.is_empty()
                && crate::prepare::inside(&north(), v.location)
        })
        .map(|v| v.id.clone())
        .expect("a Monday electrical repair in the north");
    vehicles[1].shifts[0].limits.itinerary = vec![ItineraryItem {
        id: told,
        kind: crate::wire::ItemKind::Visit,
        start_service_time: None,
        pin: false,
    }];

    Plan {
        name: Some("Turin building services, two days".into()),
        parameters: Parameters {
            duration_added_for_first_visit_on_location: 10,
            visit_completion_risk_minimal_time_to_shift_end: 30,
            visit_completion_risk_minimal_priority: "4".into(),
            ..Parameters::default()
        },
        planning_window: Some(PlanningWindow {
            start_date: at(0, 0),
            end_date: at(2, 0),
        }),
        priority_weights: Vec::new(),
        travel_matrix: None,
        traffic: vec![
            Traffic {
                id: "morning-rush".into(),
                from: "07:30".into(),
                to: "09:30".into(),
                days: WEEKDAYS.to_vec(),
                slowdowns: vec![
                    Slowdown {
                        factor: 1.5,
                        roads: names(&["primary", "secondary", "tertiary"]),
                        area: None,
                    },
                    Slowdown {
                        factor: 1.3,
                        roads: names(&["motorway", "trunk"]),
                        area: None,
                    },
                    // The centre's streets are slower still.
                    Slowdown {
                        factor: 1.2,
                        roads: names(&["residential", "unclassified"]),
                        area: Some(Area {
                            south: 45.055,
                            west: 7.655,
                            north: 45.085,
                            east: 7.705,
                        }),
                    },
                ],
            },
            Traffic {
                id: "evening-rush".into(),
                from: "17:00".into(),
                to: "19:00".into(),
                days: WEEKDAYS.to_vec(),
                slowdowns: vec![Slowdown {
                    factor: 1.4,
                    roads: names(&["trunk", "primary", "secondary", "tertiary"]),
                    area: None,
                }],
            },
        ],
        skills: Vec::new(),
        tags: Vec::new(),
        vehicles,
        visits,
        visit_groups: groups,
        elements: Vec::new(),
        shifts: Vec::new(),
        facts: Default::default(),
        weights: Default::default(),
        score: None,
    }
}
