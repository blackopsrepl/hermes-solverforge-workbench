//! Where a pickup and delivery plan travels: every shift's start and end,
//! every stop and every break taken at a place, for the host to measure along the roads.

use solverforge_workbench_host::roads::{Place, RoadMatrix, Travel};

use crate::domain::Plan;
use crate::wire::TravelMatrix;

/// Road travel a dataset carries is marked with this source.
pub const ROADS: &str = "roads";

pub fn places(plan: &Plan) -> Vec<Place> {
    let mut out = Vec::new();
    for driver in &plan.drivers {
        let name = driver.name.clone().unwrap_or_else(|| driver.id.clone());
        for shift in &driver.shifts {
            out.push(Place {
                point: shift.start_location,
                label: format!("The start of {name}'s shift {}", shift.id),
            });
            if let Some(end) = shift.end_location {
                out.push(Place {
                    point: end,
                    label: format!("The end of {name}'s shift {}", shift.id),
                });
            }
            for b in &shift.required_breaks {
                if let Some(point) = b.location {
                    out.push(Place {
                        point,
                        label: format!("Break {} of {name}", b.id),
                    });
                }
            }
        }
    }
    for job in &plan.jobs {
        for stop in &job.stops {
            out.push(Place {
                point: stop.location,
                label: format!(
                    "Stop {}",
                    stop.name.clone().unwrap_or_else(|| stop.id.clone())
                ),
            });
        }
    }
    out
}

pub fn travel(plan: &Plan) -> Travel {
    match &plan.travel_matrix {
        Some(matrix) if matrix.source.as_deref() == Some(ROADS) => Travel::Roads(RoadMatrix {
            locations: matrix.locations.clone(),
            seconds: matrix.seconds.clone(),
            meters: matrix.meters.clone(),
            snapped: matrix
                .snap_meters
                .clone()
                .unwrap_or_else(|| vec![Some(0.0); matrix.locations.len()]),
        }),
        Some(_) => Travel::Given,
        None => Travel::Missing,
    }
}

pub fn set_roads(plan: &mut Plan, roads: RoadMatrix) {
    plan.travel_matrix = Some(TravelMatrix {
        locations: roads.locations,
        seconds: roads.seconds,
        meters: roads.meters,
        source: Some(ROADS.to_string()),
        snap_meters: Some(roads.snapped),
    });
}
