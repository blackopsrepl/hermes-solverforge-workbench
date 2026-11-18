//! Roads from solverforge-maps: the network, the travel between a plan's places
//! and the road geometry of its routes.
//!
//! A model that travels lists its places ([`crate::Model::places`]). The host
//! loads the road network around them (cached on disk in the request's
//! `roads.cache_dir`, fetched from OpenStreetMap the first time), snaps each place
//! to the road it can be driven to and from, and measures every pair along the
//! roads, at each road's own speed (its `maxspeed`, else its road class). Only
//! the area's largest strongly connected road network is kept, so every place
//! can be driven to and from every other; places far from it, and pairs no road
//! connects in a network a test provides, come back as issues so nothing is
//! dropped without a word. Views draw each route along the same roads.

use std::ops::Deref;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use solverforge_maps::{
    BoundingBox, ConnectivityPolicy, Coord, NetworkConfig, NetworkRef, RoadNetwork, SnapAccess,
    SnappedCoord, UNREACHABLE,
};

use crate::model::Issue;
use crate::protocol::EngineError;

/// Where a request keeps road networks between calls.
#[derive(Debug, Clone, Deserialize)]
pub struct RoadsConfig {
    pub cache_dir: PathBuf,
}

/// A place a plan travels to or from: `[lat, lng]` and what to call it in an issue.
#[derive(Debug, Clone)]
pub struct Place {
    pub point: [f64; 2],
    pub label: String,
}

/// Travel between places measured along the roads; `None` where no road leads.
#[derive(Debug, Clone, PartialEq)]
pub struct RoadMatrix {
    pub locations: Vec<[f64; 2]>,
    pub seconds: Vec<Vec<Option<i64>>>,
    pub meters: Vec<Vec<Option<i64>>>,
    /// How far each place is from the road it was snapped to; `None` when no road is near.
    pub snapped: Vec<Option<f64>>,
}

/// What a plan's travel stands on now.
pub enum Travel {
    /// A matrix the dataset brought itself: used as given, never replaced.
    Given,
    /// Road travel measured earlier.
    Roads(RoadMatrix),
    /// Nothing yet.
    Missing,
}

/// A place farther than this from the nearest road is reported.
pub const FAR_FROM_ROAD_M: f64 = 150.0;

fn unavailable() -> EngineError {
    EngineError::new(
        "roads_unavailable",
        "this model travels on roads and the request names no road network cache",
    )
}

fn coord(point: [f64; 2]) -> Result<Coord, EngineError> {
    Coord::try_new(point[0], point[1]).map_err(|error| {
        EngineError::new(
            "invalid_plan",
            format!("{point:?} is not a [latitude, longitude] place: {error}"),
        )
    })
}

enum Network {
    Loaded(NetworkRef),
    Provided(Arc<RoadNetwork>),
}

impl Deref for Network {
    type Target = RoadNetwork;

    fn deref(&self) -> &RoadNetwork {
        match self {
            Network::Loaded(network) => network,
            Network::Provided(network) => network,
        }
    }
}

/// The road network around `points`, loaded from the cache or fetched once.
pub struct Roads {
    runtime: tokio::runtime::Runtime,
    network: Network,
}

impl Roads {
    pub fn load(config: Option<&RoadsConfig>, points: &[[f64; 2]]) -> Result<Self, EngineError> {
        let bbox = area(points)?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|error| EngineError::new("roads_failed", error.to_string()))?;
        if let Some(network) = provided(points) {
            return Ok(Self {
                runtime,
                network: Network::Provided(network),
            });
        }
        let config = config.ok_or_else(unavailable)?;
        std::fs::create_dir_all(&config.cache_dir).map_err(|error| {
            EngineError::new(
                "roads_failed",
                format!("cannot use {}: {error}", config.cache_dir.display()),
            )
        })?;
        // Only the roads every other road can be driven to and from: a place by a
        // private lane or a cut-off fragment snaps to the city's network instead,
        // and is reported by how far that is.
        let settings = NetworkConfig::default()
            .cache_dir(&config.cache_dir)
            .connectivity_policy(ConnectivityPolicy::LargestStronglyConnectedComponent);
        let network = runtime
            .block_on(RoadNetwork::load_or_fetch(&bbox, &settings, None))
            .map_err(|error| {
                EngineError::new(
                    "roads_failed",
                    format!("the road network could not be loaded: {error}"),
                )
            })?;
        Ok(Self {
            runtime,
            network: Network::Loaded(network),
        })
    }

    /// Every place snapped to a road it can be driven to and from, and the
    /// travel between each pair; with an issue per place far from a road and
    /// per place no road reaches or leaves.
    pub fn matrix(&self, places: &[Place]) -> Result<(RoadMatrix, Vec<Issue>), EngineError> {
        let coords = places
            .iter()
            .map(|p| coord(p.point))
            .collect::<Result<Vec<_>, _>>()?;
        let access = vec![SnapAccess::Both; coords.len()];
        let table = self.runtime.block_on(
            self.network
                .compute_matrix_with_access(&coords, &access, None),
        );
        let n = places.len();
        let cell = |value: Option<i64>| value.filter(|&v| v != UNREACHABLE && v >= 0);
        let seconds: Vec<Vec<Option<i64>>> = (0..n)
            .map(|i| {
                (0..n)
                    .map(|j| {
                        if i == j {
                            Some(0)
                        } else {
                            cell(table.get(i, j))
                        }
                    })
                    .collect()
            })
            .collect();
        let meters: Vec<Vec<Option<i64>>> = (0..n)
            .map(|i| {
                (0..n)
                    .map(|j| {
                        if i == j {
                            Some(0)
                        } else if seconds[i][j].is_some() {
                            cell(table.distance_meters(i, j))
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .collect();
        let matrix = RoadMatrix {
            locations: places.iter().map(|p| p.point).collect(),
            seconds,
            meters,
            snapped: table
                .locations()
                .iter()
                .map(|snap| Some(snap.snap_distance_m).filter(|m| m.is_finite()))
                .collect(),
        };
        let issues = issues(places, &matrix);
        Ok((matrix, issues))
    }

    /// The road geometry through `points` in order, as `[lat, lng]` pairs. A hop
    /// no road connects keeps its two ends, so the gap stays visible.
    pub fn path(&self, points: &[[f64; 2]]) -> Result<Vec<[f64; 2]>, EngineError> {
        let mut snaps: Vec<SnappedCoord> = Vec::with_capacity(points.len());
        for &point in points {
            let snapped = self
                .network
                .snap_road_index_with_access(coord(point)?, SnapAccess::Both)
                .map_err(|error| EngineError::new("roads_failed", error.to_string()))?;
            snaps.push(snapped);
        }
        let mut path: Vec<[f64; 2]> = Vec::new();
        for (hop, pair) in snaps.windows(2).enumerate() {
            let leg: Vec<[f64; 2]> = match self.network.route_snapped(&pair[0], &pair[1]) {
                Ok(route) => std::iter::once(points[hop])
                    .chain(route.geometry.iter().map(|c| [c.lat, c.lng]))
                    .chain(std::iter::once(points[hop + 1]))
                    .collect(),
                Err(_) => vec![points[hop], points[hop + 1]],
            };
            for point in leg {
                if path.last() != Some(&point) {
                    path.push(point);
                }
            }
        }
        if path.is_empty() {
            path.extend(points.first());
        }
        Ok(path)
    }
}

/// Whether two `[lat, lng]` places are the same, up to the last bits a JSON
/// round trip may change.
pub fn same(a: &[f64; 2], b: &[f64; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9
}

/// Each point once, in first-seen order: the places a network is loaded for.
pub fn unique(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut out: Vec<[f64; 2]> = Vec::with_capacity(points.len());
    for &point in points {
        if !out.iter().any(|p| same(p, &point)) {
            out.push(point);
        }
    }
    out
}

/// The area [`Roads::load`] asks solverforge-maps for, given `points`.
fn area(points: &[[f64; 2]]) -> Result<BoundingBox, EngineError> {
    let coords = unique(points)
        .into_iter()
        .map(coord)
        .collect::<Result<Vec<_>, _>>()?;
    if coords.is_empty() {
        return Err(EngineError::new("invalid_plan", "the plan has no places"));
    }
    Ok(BoundingBox::from_coords(&coords).expand_for_routing(&coords))
}

type Provided = Vec<(Vec<[f64; 2]>, Arc<RoadNetwork>)>;

static PROVIDED: Mutex<Provided> = Mutex::new(Vec::new());

/// Makes [`Roads::load`] use `network`, for this process, whenever every place
/// it loads for is one of `points`, instead of the cache or OpenStreetMap; the
/// latest provided wins. Tests use it to choose a plan's roads; the product
/// never calls it.
pub fn provide(points: &[[f64; 2]], network: RoadNetwork) {
    PROVIDED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push((unique(points), Arc::new(network)));
}

fn provided(points: &[[f64; 2]]) -> Option<Arc<RoadNetwork>> {
    let all = PROVIDED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    all.iter()
        .rev()
        .find(|(covered, _)| points.iter().all(|p| covered.iter().any(|c| same(c, p))))
        .map(|(_, network)| network.clone())
}

/// A network with one node per distinct point and a road between every ordered
/// pair `leg` returns `(seconds, meters)` for, provided for that area.
pub fn provide_complete(
    points: &[[f64; 2]],
    leg: impl Fn([f64; 2], [f64; 2]) -> Option<(f64, f64)>,
) {
    let nodes = unique(points);
    let mut edges = Vec::new();
    for (i, &a) in nodes.iter().enumerate() {
        for (j, &b) in nodes.iter().enumerate() {
            if i != j {
                if let Some((seconds, meters)) = leg(a, b) {
                    edges.push((i, j, seconds, meters));
                }
            }
        }
    }
    let at: Vec<(f64, f64)> = nodes.iter().map(|p| (p[0], p[1])).collect();
    provide(&nodes, RoadNetwork::from_test_data(&at, &edges));
}

/// What the roads say about the places: each one far from any road, and each
/// one no road connects with most of the others, by the label `places` gives it.
pub fn issues(places: &[Place], matrix: &RoadMatrix) -> Vec<Issue> {
    let at = |point: &[f64; 2]| matrix.locations.iter().position(|l| same(l, point));
    let mut out = Vec::new();
    let mut seen: Vec<usize> = Vec::new();
    for place in places {
        let Some(i) = at(&place.point) else { continue };
        if seen.contains(&i) {
            continue;
        }
        seen.push(i);
        match matrix.snapped.get(i).copied().flatten() {
            None => out.push(Issue::warning(
                "no_road",
                format!("{} has no road near it", place.label),
            )),
            Some(m) if m > FAR_FROM_ROAD_M => out.push(Issue::warning(
                "far_from_road",
                format!("{} is {} from the nearest road", place.label, distance(m)),
            )),
            Some(_) => {}
        }
        let n = matrix.locations.len();
        if n < 2 {
            continue;
        }
        let cut = (0..n)
            .filter(|&j| {
                j != i && (matrix.seconds[i][j].is_none() || matrix.seconds[j][i].is_none())
            })
            .count();
        if cut == n - 1 {
            out.push(Issue::warning(
                "unreachable",
                format!("No road connects {} with any other place", place.label),
            ));
        } else if cut * 2 > n - 1 {
            out.push(Issue::warning(
                "unreachable",
                format!(
                    "No road connects {} with {cut} of the other {} places",
                    place.label,
                    n - 1
                ),
            ));
        }
    }
    out
}

fn distance(meters: f64) -> String {
    if meters >= 1000.0 {
        format!("{:.1} km", meters / 1000.0)
    } else {
        format!("{} m", meters.round() as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances_read_in_meters_then_kilometers() {
        assert_eq!(distance(317.4), "317 m");
        assert_eq!(distance(1240.0), "1.2 km");
    }

    fn place(point: [f64; 2], label: &str) -> Place {
        Place {
            point,
            label: label.into(),
        }
    }

    #[test]
    fn travel_follows_the_roads_and_says_what_it_cannot_reach() {
        let (a, b, c) = ([45.0, 7.6], [45.01, 7.61], [45.02, 7.6]);
        let (d, e, g) = ([45.2, 7.9], [45.21, 7.9], [45.21, 7.91]);
        let points = [a, b, c, d, e, g];
        // A one-way loop a -> b -> c -> a, and apart from it, as large, d, e and g
        // joined both ways (snapping prefers a larger network, so equal sizes stay apart).
        provide_complete(&points, |x, y| {
            let loop_hop = (x == a && y == b) || (x == b && y == c) || (x == c && y == a);
            let apart = [d, e, g].contains(&x) && [d, e, g].contains(&y);
            (loop_hop || apart).then_some((100.0, 1000.0))
        });
        let roads = Roads::load(None, &points).unwrap();
        let far = [45.0, 7.64];
        let places = [
            place(a, "Depot"),
            place(b, "Visit B"),
            place(c, "Visit C"),
            place(d, "Visit D"),
            place(e, "Visit E"),
            place(g, "Visit G"),
            place(far, "Visit F"),
        ];
        let (matrix, issues) = roads.matrix(&places).unwrap();
        assert_eq!(matrix.seconds[0][..3], [Some(0), Some(100), Some(200)]);
        assert_eq!(
            matrix.seconds[1][0],
            Some(200),
            "b reaches a around the loop"
        );
        assert_eq!(matrix.meters[0][2], Some(2000));
        assert_eq!(matrix.seconds[0][3], None, "no road joins the loop and d");
        let messages: Vec<&str> = issues.iter().map(|i| i.message.as_str()).collect();
        assert!(
            messages.contains(&"No road connects Visit D with 4 of the other 6 places"),
            "{messages:?}"
        );
        assert!(
            messages
                .iter()
                .any(|m| m.starts_with("Visit F is ") && m.ends_with(" from the nearest road")),
            "{messages:?}"
        );
        let path = roads.path(&[b, a]).unwrap();
        assert_eq!((path.first(), path.last()), (Some(&b), Some(&a)));
        assert!(path.contains(&c), "b to a drives through c: {path:?}");
    }

    #[test]
    fn a_request_without_a_cache_says_roads_are_unavailable() {
        let error = Roads::load(None, &[[12.5, 3.25]]).err().unwrap();
        assert_eq!(error.code, "roads_unavailable");
    }
}
