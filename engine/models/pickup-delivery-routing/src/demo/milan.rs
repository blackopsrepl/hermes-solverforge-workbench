//! A Milan transport cooperative over two weekdays: patient transport to
//! and from hospitals, and a medical courier service between labs,
//! pharmacies and clinics, on one shared fleet of vans.

use super::rng::Rng;
use crate::domain::Plan;
use crate::time::format_date_time;
use crate::wire::{
    Break, BreakKind, BreakPolicy, Capacity, Demand, Driver, DriverShift, Interruption, Job,
    JobDemand, Parameters, PlanningWindow, Skill, SkillRequirement, Stop, TimeWindow,
};

const MONDAY: &str = "2027-02-01";

/// Hospitals and clinics patients are taken to.
const HOSPITALS: [(&str, f64, f64); 12] = [
    ("Ospedale Niguarda", 45.5106, 9.1868),
    ("Policlinico", 45.4594, 9.1975),
    ("Ospedale San Raffaele", 45.5050, 9.2648),
    ("Humanitas Rozzano", 45.3735, 9.1570),
    ("Ospedale Sacco", 45.5219, 9.1218),
    ("Ospedale San Paolo", 45.4367, 9.1505),
    ("Ospedale San Carlo", 45.4600, 9.1150),
    ("Fatebenefratelli", 45.4770, 9.1920),
    ("Istituto Tumori", 45.4588, 9.2270),
    ("Gaetano Pini", 45.4560, 9.1950),
    ("Centro Monzino", 45.4455, 9.2290),
    ("Istituto Besta", 45.4790, 9.2290),
];

/// Neighbourhoods patients live in, by their centre.
const HOMES: [(&str, f64, f64); 20] = [
    ("Baggio", 45.4630, 9.0880),
    ("Quarto Oggiaro", 45.5180, 9.1370),
    ("Gratosoglio", 45.4100, 9.1730),
    ("Corvetto", 45.4400, 9.2230),
    ("Lorenteggio", 45.4520, 9.1180),
    ("Niguarda", 45.5150, 9.1900),
    ("Lambrate", 45.4850, 9.2380),
    ("Porta Romana", 45.4520, 9.2030),
    ("Città Studi", 45.4780, 9.2270),
    ("Bovisa", 45.5030, 9.1590),
    ("Sesto San Giovanni", 45.5350, 9.2330),
    ("Cinisello Balsamo", 45.5560, 9.2150),
    ("Rozzano", 45.3820, 9.1560),
    ("San Siro", 45.4780, 9.1240),
    ("Isola", 45.4880, 9.1890),
    ("Navigli", 45.4500, 9.1750),
    ("Precotto", 45.5090, 9.2240),
    ("Gorla", 45.5010, 9.2240),
    ("Forlanini", 45.4600, 9.2550),
    ("Bicocca", 45.5170, 9.2130),
];

/// Where courier jobs are picked up.
const HUBS: [(&str, f64, f64); 4] = [
    ("Laboratorio Bicocca", 45.5172, 9.2128),
    ("Farmacia ospedaliera Rogoredo", 45.4340, 9.2390),
    ("Hub vaccini Segrate", 45.4930, 9.2950),
    ("Centro trasfusionale Niguarda", 45.5110, 9.1880),
];

/// Clinics courier jobs deliver to, besides the hospitals.
const CLINICS: [(&str, f64, f64); 8] = [
    ("Poliambulatorio Città Studi", 45.4782, 9.2266),
    ("Poliambulatorio Baggio", 45.4628, 9.0874),
    ("Poliambulatorio Quarto Oggiaro", 45.5168, 9.1372),
    ("Casa di cura Corvetto", 45.4398, 9.2236),
    ("Poliambulatorio Sesto", 45.5348, 9.2326),
    ("RSA Gratosoglio", 45.4106, 9.1725),
    ("Poliambulatorio Lorenteggio", 45.4518, 9.1176),
    ("Casa della salute Niguarda", 45.5148, 9.1906),
];

const DEPOTS: [(&str, f64, f64); 3] = [
    ("Depot Lambrate", 45.4846, 9.2367),
    ("Depot Bovisa", 45.5035, 9.1580),
    ("Depot Corvetto", 45.4402, 9.2228),
];

/// A van and who drives it.
struct Crew {
    id: &'static str,
    name: &'static str,
    depot: usize,
    /// Patient van (seats, wheelchair place, a few parcels) or courier van.
    patients: bool,
    wheelchair: i64,
    lift: bool,
    refrigerated: bool,
    /// Skill levels: wheelchair assistance, medical escort, hazardous goods.
    assist: Option<i64>,
    escort: Option<i64>,
    hazmat: bool,
    /// Late shift (11:00) instead of the early one (07:00).
    late: bool,
}

const CREWS: [Crew; 15] = [
    Crew {
        id: "amina",
        name: "Amina Rossi",
        depot: 0,
        patients: true,
        wheelchair: 2,
        lift: true,
        refrigerated: false,
        assist: Some(3),
        escort: Some(2),
        hazmat: false,
        late: false,
    },
    Crew {
        id: "luca",
        name: "Luca Bianchi",
        depot: 1,
        patients: true,
        wheelchair: 1,
        lift: true,
        refrigerated: false,
        assist: Some(2),
        escort: None,
        hazmat: false,
        late: false,
    },
    Crew {
        id: "giulia",
        name: "Giulia Ferrari",
        depot: 2,
        patients: true,
        wheelchair: 2,
        lift: true,
        refrigerated: false,
        assist: Some(2),
        escort: Some(1),
        hazmat: false,
        late: false,
    },
    Crew {
        id: "marco",
        name: "Marco Esposito",
        depot: 0,
        patients: true,
        wheelchair: 0,
        lift: false,
        refrigerated: false,
        assist: None,
        escort: Some(1),
        hazmat: false,
        late: false,
    },
    Crew {
        id: "sara",
        name: "Sara Romano",
        depot: 1,
        patients: true,
        wheelchair: 1,
        lift: true,
        refrigerated: false,
        assist: Some(1),
        escort: Some(2),
        hazmat: false,
        late: true,
    },
    Crew {
        id: "davide",
        name: "Davide Colombo",
        depot: 2,
        patients: true,
        wheelchair: 0,
        lift: false,
        refrigerated: false,
        assist: None,
        escort: None,
        hazmat: false,
        late: true,
    },
    Crew {
        id: "elena",
        name: "Elena Ricci",
        depot: 0,
        patients: true,
        wheelchair: 1,
        lift: true,
        refrigerated: false,
        assist: Some(3),
        escort: None,
        hazmat: false,
        late: true,
    },
    Crew {
        id: "paolo",
        name: "Paolo Marino",
        depot: 1,
        patients: true,
        wheelchair: 0,
        lift: false,
        refrigerated: false,
        assist: None,
        escort: Some(1),
        hazmat: false,
        late: false,
    },
    Crew {
        id: "chiara",
        name: "Chiara Greco",
        depot: 0,
        patients: false,
        wheelchair: 0,
        lift: false,
        refrigerated: true,
        assist: None,
        escort: None,
        hazmat: true,
        late: false,
    },
    Crew {
        id: "matteo",
        name: "Matteo Bruno",
        depot: 1,
        patients: false,
        wheelchair: 0,
        lift: false,
        refrigerated: false,
        assist: None,
        escort: None,
        hazmat: false,
        late: false,
    },
    Crew {
        id: "federica",
        name: "Federica Gallo",
        depot: 2,
        patients: false,
        wheelchair: 0,
        lift: false,
        refrigerated: true,
        assist: None,
        escort: None,
        hazmat: false,
        late: false,
    },
    Crew {
        id: "andrea",
        name: "Andrea Conti",
        depot: 0,
        patients: false,
        wheelchair: 0,
        lift: false,
        refrigerated: false,
        assist: None,
        escort: None,
        hazmat: true,
        late: true,
    },
    Crew {
        id: "valentina",
        name: "Valentina De Luca",
        depot: 1,
        patients: false,
        wheelchair: 0,
        lift: false,
        refrigerated: false,
        assist: None,
        escort: None,
        hazmat: false,
        late: true,
    },
    Crew {
        id: "simone",
        name: "Simone Mancini",
        depot: 2,
        patients: false,
        wheelchair: 0,
        lift: false,
        refrigerated: true,
        assist: None,
        escort: None,
        hazmat: false,
        late: false,
    },
    Crew {
        id: "francesca",
        name: "Francesca Costa",
        depot: 0,
        patients: false,
        wheelchair: 0,
        lift: false,
        refrigerated: false,
        assist: None,
        escort: None,
        hazmat: false,
        late: true,
    },
];
fn at(day: i64, minutes: i64) -> String {
    let base = crate::time::parse_date_time(&format!("{MONDAY}T00:00")).expect("demo date");
    format_date_time(base + day * crate::time::DAY + minutes * 60)
}

fn round5(minutes: i64) -> i64 {
    minutes / 5 * 5
}

fn near(rng: &mut Rng, lat: f64, lon: f64, meters: f64) -> [f64; 2] {
    let dlat = rng.signed() * meters / 111_000.0;
    let dlon = rng.signed() * meters / (111_000.0 * lat.to_radians().cos());
    [
        ((lat + dlat) * 1e5).round() / 1e5,
        ((lon + dlon) * 1e5).round() / 1e5,
    ]
}

fn shifts(crew: &Crew, day: i64) -> DriverShift {
    let (start, end) = if crew.late {
        (11 * 60, 19 * 60 + 30)
    } else {
        (7 * 60, 15 * 60 + 30)
    };
    let (_, lat, lon) = DEPOTS[crew.depot];
    let mut capacities = vec![
        Capacity {
            kind: "seats".into(),
            quantity: if crew.patients { 4 } else { 1 },
        },
        Capacity {
            kind: "parcels".into(),
            quantity: if crew.patients { 6 } else { 40 },
        },
    ];
    if crew.wheelchair > 0 {
        capacities.push(Capacity {
            kind: "wheelchair".into(),
            quantity: crew.wheelchair,
        });
    }
    let mut skills = Vec::new();
    if let Some(level) = crew.assist {
        skills.push(Skill {
            name: "wheelchair-assist".into(),
            level: Some(level),
            multiplier: (level == 3).then_some(0.8),
        });
    }
    if let Some(level) = crew.escort {
        skills.push(Skill {
            name: "medical-escort".into(),
            level: Some(level),
            multiplier: None,
        });
    }
    if crew.hazmat {
        skills.push(Skill {
            name: "hazardous-goods".into(),
            level: Some(1),
            multiplier: None,
        });
    }
    let mut tags = Vec::new();
    if crew.lift {
        tags.push("lift".to_string());
    }
    if crew.refrigerated {
        tags.push("refrigerated".to_string());
    }
    let mut breaks = vec![Break {
        id: format!("{}-lunch-{}", crew.id, day + 1),
        kind: BreakKind::Floating,
        // Patient vans take lunch with nobody on board.
        policy: if crew.patients {
            BreakPolicy::RejectJobs
        } else {
            BreakPolicy::PermitJobs
        },
        min_start_time: Some(at(
            day,
            if crew.late {
                14 * 60 + 30
            } else {
                11 * 60 + 30
            },
        )),
        max_end_time: Some(at(day, if crew.late { 17 * 60 } else { 14 * 60 })),
        duration: Some(30),
        ..Break::default()
    }];
    if crew.id == "luca" && day == 0 {
        breaks.push(Break {
            id: "luca-van-inspection".into(),
            kind: BreakKind::Fixed,
            start_time: Some(at(day, 10 * 60)),
            end_time: Some(at(day, 10 * 60 + 20)),
            ..Break::default()
        });
    }
    if crew.id == "matteo" && day == 1 {
        breaks.push(Break {
            id: "matteo-medical-check".into(),
            kind: BreakKind::Fixed,
            location: Some([45.5106, 9.1868]),
            start_time: Some(at(day, 13 * 60)),
            end_time: Some(at(day, 13 * 60 + 45)),
            ..Break::default()
        });
    }
    DriverShift {
        id: format!("{}-{}", crew.id, ["mon", "tue"][day as usize]),
        start_location: [lat, lon],
        stock: Vec::new(),
        end_location: None,
        min_start_time: Some(at(day, start)),
        max_soft_end_time: Some(at(day, end)),
        max_end_time: Some(at(day, end + 60)),
        max_soft_last_stop_departure_time: Some(at(day, end - 20)),
        max_last_stop_departure_time: Some(at(day, end + 30)),
        capacities,
        skills,
        tags,
        required_breaks: breaks,
        ..DriverShift::default()
    }
}

fn window(day: i64, from: i64, to: i64) -> TimeWindow {
    TimeWindow {
        min_start_time: at(day, from),
        max_start_time: None,
        max_end_time: at(day, to),
    }
}

/// A patient's ride to an appointment and, for most, the ride home.
fn patient_jobs(rng: &mut Rng, day: i64, n: usize, jobs: &mut Vec<Job>) {
    for k in 0..n {
        let (area, hlat, hlon) = HOMES[rng.below(HOMES.len() as u64) as usize];
        let (hospital, plat, plon) = HOSPITALS[rng.below(HOSPITALS.len() as u64) as usize];
        let home = near(rng, hlat, hlon, 700.0);
        let appointment = round5(8 * 60 + 30 + rng.below(8 * 60) as i64);
        let length = [45, 60, 90, 120, 240][rng.below(5) as usize];
        let wheelchair = rng.chance(18);
        let fragile = rng.chance(8);
        let escort = !wheelchair && rng.chance(15);
        let dialysis = length == 240;
        let id = format!("P{}-{:02}", day + 1, k + 1);
        let mut demands = vec![JobDemand {
            kind: "seats".into(),
            demand: 1,
        }];
        if wheelchair {
            demands = vec![JobDemand {
                kind: "wheelchair".into(),
                demand: 1,
            }];
        }
        let mut required_skills = Vec::new();
        let mut required_tags = Vec::new();
        if wheelchair {
            required_skills.push(SkillRequirement {
                name: "wheelchair-assist".into(),
                min_level: Some(2),
            });
            required_tags.push("lift".to_string());
        }
        if escort {
            required_skills.push(SkillRequirement {
                name: "medical-escort".into(),
                min_level: Some(1),
            });
        }
        let regular = ["amina", "giulia", "elena"][rng.below(3) as usize];
        let make = |id: String, name: String, from: Stop, to: Stop, rng: &mut Rng| Job {
            id,
            name: Some(name),
            priority: Some("2".into()),
            stops: vec![from, to],
            prohibit_job_pooling: fragile,
            allow_interruption: Interruption::None,
            preferred_drivers: if dialysis {
                vec![regular.to_string()]
            } else {
                Vec::new()
            },
            prohibited_drivers: if rng.chance(4) {
                vec!["paolo".to_string()]
            } else {
                Vec::new()
            },
            required_drivers: if rng.chance(3) {
                vec![
                    "amina".to_string(),
                    "giulia".to_string(),
                    "sara".to_string(),
                ]
            } else {
                Vec::new()
            },
            required_skills: required_skills.clone(),
            required_tags: required_tags.clone(),
            maximum_time_burden: if fragile { Some(10) } else { None },
            demands: demands.clone(),
            ..Job::default()
        };
        let pickup = Stop {
            id: format!("{id}-a"),
            name: Some(format!("{area} home")),
            location: home,
            parts: Vec::new(),
            duration: if wheelchair { 10 } else { 5 },
            // Collected in time to reach the appointment, and no earlier than
            // the hospital lets the patient in less the ride: a pick-up any
            // earlier would hold the patient on board.
            time_windows: vec![TimeWindow {
                max_start_time: Some(at(day, appointment - 30)),
                ..window(day, appointment - 75, appointment - 15)
            }],
            ..Stop::default()
        };
        // Patients board and leave at the hospital's main entrance.
        let drop = Stop {
            id: format!("{id}-b"),
            name: Some(hospital.to_string()),
            location: [plat, plon],
            parts: Vec::new(),
            duration: if wheelchair { 8 } else { 4 },
            time_windows: vec![window(day, appointment - 60, appointment)],
            ..Stop::default()
        };
        jobs.push(make(
            id.clone(),
            format!("Ride to {hospital}"),
            pickup,
            drop,
            rng,
        ));
        let back = appointment + length;
        if back + 30 < 18 * 60 + 45 && rng.chance(80) {
            let rid = format!("{id}R");
            let pickup = Stop {
                id: format!("{rid}-a"),
                name: Some(hospital.to_string()),
                location: [plat, plon],
                parts: Vec::new(),
                duration: if wheelchair { 10 } else { 5 },
                time_windows: vec![window(day, back, back + 90)],
                ..Stop::default()
            };
            let drop = Stop {
                id: format!("{rid}-b"),
                name: Some(format!("{area} home")),
                location: home,
                parts: Vec::new(),
                duration: if wheelchair { 8 } else { 4 },
                time_windows: vec![window(day, back, back + 150)],
                ..Stop::default()
            };
            jobs.push(make(
                rid,
                format!("Ride home from {hospital}"),
                pickup,
                drop,
                rng,
            ));
        }
    }
}

/// Courier runs: samples, drugs and vaccines from a hub to one or two
/// clinics or hospitals.
fn courier_jobs(rng: &mut Rng, day: i64, n: usize, jobs: &mut Vec<Job>) {
    let destinations: Vec<(&str, f64, f64)> =
        CLINICS.iter().chain(HOSPITALS.iter()).copied().collect();
    for k in 0..n {
        let hub = rng.below(HUBS.len() as u64) as usize;
        let (hub_name, hlat, hlon) = HUBS[hub];
        let id = format!("C{}-{:02}", day + 1, k + 1);
        let kind = rng.below(10);
        let vaccines = hub == 2;
        let cytotoxic = hub == 1 && kind < 3;
        let stat = hub == 0 && kind < 4;
        let drops = if rng.chance(30) { 2 } else { 1 };
        let parcels = 2 + rng.below(8) as i64;
        let ready = round5(7 * 60 + 30 + rng.below(9 * 60) as i64);
        // The cold-chain hub dispatches by 14:00, while the refrigerated
        // vans still have the hours to deliver before their last departure.
        let ready = if vaccines { ready.min(14 * 60) } else { ready };
        // A third of the routine runs may go on either day.
        let movable = day == 0 && !stat && !vaccines && rng.chance(30);
        let windows = |from: i64, to: i64| {
            let mut w = vec![window(day, from, to)];
            if movable {
                w.push(window(1, from, to));
            }
            w
        };
        let mut stops = vec![Stop {
            id: format!("{id}-a"),
            name: Some(hub_name.to_string()),
            location: [hlat, hlon],
            parts: Vec::new(),
            duration: 5,
            time_windows: windows(ready, (ready + 240).min(18 * 60 + 30)),
            demands: vec![Demand {
                kind: "parcels".into(),
                pickup: parcels,
                deliver: 0,
            }],
            ..Stop::default()
        }];
        let mut left = parcels;
        for d in 0..drops {
            let (name, lat, lon) = destinations[rng.below(destinations.len() as u64) as usize];
            let deliver = if d + 1 == drops { left } else { parcels / 2 };
            left -= deliver;
            stops.push(Stop {
                id: format!("{id}-{}", ["b", "c"][d]),
                name: Some(name.to_string()),
                location: near(rng, lat, lon, 60.0),
                parts: Vec::new(),
                duration: 5 + rng.below(3) as i64 * 5,
                time_windows: windows(ready, (ready + 360).min(19 * 60)),
                demands: vec![Demand {
                    kind: "parcels".into(),
                    pickup: 0,
                    deliver,
                }],
                latest_sla_end_time: stat.then(|| at(day, ready + 90)),
                ..Stop::default()
            });
        }
        jobs.push(Job {
            id: id.clone(),
            name: Some(match (stat, vaccines, cytotoxic) {
                (true, _, _) => "Urgent lab samples".to_string(),
                (_, true, _) => "Vaccine delivery".to_string(),
                (_, _, true) => "Chemotherapy drugs".to_string(),
                _ => "Pharmacy delivery".to_string(),
            }),
            priority: Some(match (stat, movable, rng.below(10)) {
                (true, _, _) => "3".to_string(),
                (_, true, _) => "6".to_string(),
                (_, _, 0..=1) => "opt-3".to_string(),
                _ => "5".to_string(),
            }),
            stops,
            required_tags: if vaccines {
                vec!["refrigerated".to_string()]
            } else {
                Vec::new()
            },
            required_skills: if cytotoxic {
                vec![SkillRequirement {
                    name: "hazardous-goods".into(),
                    min_level: None,
                }]
            } else {
                Vec::new()
            },
            ..Job::default()
        });
    }
}

pub fn milan_two_days() -> Plan {
    let mut rng = Rng::new(0x4D49_4C41_4E4F);
    let drivers = CREWS
        .iter()
        .map(|crew| Driver {
            id: crew.id.to_string(),
            name: Some(crew.name.to_string()),
            travel_mode: None,
            shifts: (0..2).map(|day| shifts(crew, day)).collect(),
        })
        .collect();
    let mut jobs = Vec::new();
    for day in 0..2 {
        patient_jobs(&mut rng, day, 26, &mut jobs);
        courier_jobs(&mut rng, day, 34, &mut jobs);
    }
    // Chemotherapy drugs never ride with food-grade vaccine boxes.
    let cytotoxic: Vec<String> = jobs
        .iter()
        .filter(|j| j.name.as_deref() == Some("Chemotherapy drugs"))
        .map(|j| j.id.clone())
        .collect();
    for job in jobs
        .iter_mut()
        .filter(|j| j.name.as_deref() == Some("Vaccine delivery"))
    {
        job.prohibited_jobs = cytotoxic.clone();
    }
    Plan {
        name: Some("Milan patient transport and medical courier, two days".into()),
        parameters: Parameters {
            maximum_time_burden: Some(25),
            ..Parameters::default()
        },
        planning_window: Some(PlanningWindow {
            start_date: at(0, 0),
            end_date: at(2, 0),
        }),
        priority_weights: Vec::new(),
        travel_matrix: None,
        traffic: Vec::new(),
        drivers,
        jobs,
        visits: Vec::new(),
        shifts: Vec::new(),
        facts: Default::default(),
        weights: Default::default(),
        score: None,
    }
}
