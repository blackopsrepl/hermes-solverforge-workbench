//! Where a field service plan travels: every shift's start and end, every
//! visit and every break taken at a place, for the host to measure along the roads.

use solverforge_workbench_host::roads::{Place, RoadMatrix, Travel};

use crate::domain::Plan;
use crate::wire::TravelMatrix;

/// Road travel a dataset carries is marked with this source.
pub const ROADS: &str = "roads";

pub fn places(plan: &Plan) -> Vec<Place> {
    let mut out = Vec::new();
    for vehicle in &plan.vehicles {
        let name = vehicle.name.clone().unwrap_or_else(|| vehicle.id.clone());
        for shift in &vehicle.shifts {
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
            for b in &shift.limits.required_breaks {
                if let Some(point) = b.location {
                    out.push(Place {
                        point,
                        label: format!("Break {} of {name}", b.id),
                    });
                }
            }
        }
    }
    let visits = plan
        .visits
        .iter()
        .chain(plan.visit_groups.iter().flat_map(|g| g.visits.iter()));
    for visit in visits {
        out.push(Place {
            point: visit.location,
            label: format!(
                "Visit {}",
                visit.name.clone().unwrap_or_else(|| visit.id.clone())
            ),
        });
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
