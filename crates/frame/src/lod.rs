//! CDLOD-style terrain node selection on the cube-sphere quadtree (CON-03, ADR 0009).
//!
//! A pure function of the camera, the parameters and the tree: no GPU, no I/O, no clock. A node is split while
//! the screen-space size of its sample spacing exceeds a pixel threshold; the leaves are then balanced so that
//! neighbouring leaves differ by at most one level, and each leaf gets a morph interval (distances between which it
//! blends towards its parent's geometry) so that a merge or split never pops.

use planet_core::cube::FaceMapping;
use planet_core::{Face, PlanetFixed, Side, TileId, Vec3};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq)]
pub struct LodParams {
    pub radius_m: f64,
    /// Highest terrain height above the sphere, metres (used for culling, not for selection).
    pub max_height_m: f64,
    pub fov_y_rad: f64,
    pub viewport_h_px: f64,
    /// Split while the projected sample spacing is larger than this many pixels.
    pub tau_px: f64,
    /// Cells per tile edge (samples are `cells + 1` per edge).
    pub cells: u32,
    pub max_level: u8,
    /// Fraction of the split distance over which a node morphs (0.3 = last 30 % before it merges into its parent).
    pub morph_band: f64,
    /// Drop nodes that lie entirely below the horizon (padded by `max_height_m`).
    pub cull_horizon: bool,
}

impl LodParams {
    pub fn earth_1080p() -> LodParams {
        LodParams {
            radius_m: 6_371_000.0,
            max_height_m: 9_000.0,
            fov_y_rad: 60f64.to_radians(),
            viewport_h_px: 1080.0,
            tau_px: 6.0,
            cells: 32,
            max_level: 26,
            morph_band: 0.3,
            cull_horizon: true,
        }
    }

    /// Distance (to the node's bounding sphere surface) below which a node of `level` is split.
    pub fn split_distance(&self, level: u8) -> f64 {
        self.sample_spacing(level) * self.viewport_h_px / (2.0 * libm::tan(self.fov_y_rad / 2.0) * self.tau_px)
    }

    /// Worst-case sample spacing on the sphere at `level`, metres (face centre, tangent warp).
    pub fn sample_spacing(&self, level: u8) -> f64 {
        // A face spans 2R·(π/4)·(2/1) ≈ R·π/2 along the middle; level L divides it into 2^L tiles of `cells` cells.
        self.radius_m * core::f64::consts::FRAC_PI_2 / f64::from(1u32 << level) / f64::from(self.cells)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectedNode {
    pub tile: TileId,
    /// 0 = full detail, 1 = fully morphed to the parent's geometry.
    pub morph: f32,
    /// Distance from the camera to the node's bounding sphere surface, metres.
    pub distance_m: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NodeSelection {
    /// Leaves in ascending tile-id order (deterministic).
    pub nodes: Vec<SelectedNode>,
}

/// Bounding sphere of a tile on the reference sphere: centre on the surface, radius covering corners and edge midpoints.
/// Terrain height is deliberately not added: selection works on distance to the reference surface (padding by
/// `max_height_m` would force full subdivision around a ground-level camera); culling will pad it.
pub fn bounding_sphere(map: &dyn FaceMapping, tile: TileId, p: &LodParams) -> (Vec3, f64) {
    let (s0, s1, t0, t1) = tile.bounds();
    let (sm, tm) = (0.5 * (s0 + s1), 0.5 * (t0 + t1));
    let centre = tile.center_direction(map) * p.radius_m;
    let mut r = 0.0f64;
    for (s, t) in [(s0, t0), (s1, t0), (s0, t1), (s1, t1), (sm, t0), (sm, t1), (s0, tm), (s1, tm)] {
        let corner = planet_core::cube::face_to_direction(map, tile.face(), s, t) * p.radius_m;
        r = r.max((corner - centre).length());
    }
    (centre, r)
}

fn surface_distance(map: &dyn FaceMapping, tile: TileId, camera: PlanetFixed, p: &LodParams) -> f64 {
    let (c, r) = bounding_sphere(map, tile, p);
    ((camera.0 - c).length() - r).max(0.0)
}

fn children_of(t: TileId) -> [TileId; 4] {
    t.children().expect("below the maximum level")
}

/// Select the nodes to draw for `camera`.
pub fn select_nodes(map: &dyn FaceMapping, camera: PlanetFixed, p: &LodParams) -> NodeSelection {
    let mut leaves: BTreeSet<TileId> = BTreeSet::new();
    let mut stack: Vec<TileId> = Face::ALL.iter().map(|&f| TileId::new(f, 0, 0, 0).expect("root")).collect();
    while let Some(t) = stack.pop() {
        if p.cull_horizon && below_horizon(map, t, camera, p) {
            continue;
        }
        if t.level() < p.max_level && surface_distance(map, t, camera, p) < p.split_distance(t.level()) {
            stack.extend(children_of(t));
        } else {
            leaves.insert(t);
        }
    }
    balance_leaves(map, &mut leaves);
    let nodes = leaves
        .iter()
        .map(|&tile| {
            let distance_m = surface_distance(map, tile, camera, p);
            SelectedNode { tile, morph: morph_factor(map, tile, camera, p) as f32, distance_m }
        })
        .collect();
    NodeSelection { nodes }
}

/// Split leaves until neighbouring leaves differ by at most one level.
pub fn balance_leaves(map: &dyn FaceMapping, leaves: &mut BTreeSet<TileId>) {
    loop {
        let mut to_split: BTreeSet<TileId> = BTreeSet::new();
        for &t in leaves.iter() {
            if t.level() < 2 {
                continue;
            }
            for side in Side::ALL {
                let mut a = t.neighbor(map, side);
                // Any leaf that is an ancestor of the neighbour two or more levels up is too coarse.
                a = match a.parent().and_then(|p| p.parent()) {
                    Some(x) => x,
                    None => continue,
                };
                loop {
                    if leaves.contains(&a) {
                        to_split.insert(a);
                    }
                    match a.parent() {
                        Some(x) => a = x,
                        None => break,
                    }
                }
            }
        }
        if to_split.is_empty() {
            return;
        }
        for t in to_split {
            leaves.remove(&t);
            leaves.extend(children_of(t));
        }
    }
}

/// Morph of a leaf: 0 until the camera is within `morph_band` of its parent's split distance, 1 at that distance.
/// Measured to the parent's bounding sphere so a child is exactly fully morphed when its parent stops splitting.
fn morph_factor(map: &dyn FaceMapping, tile: TileId, camera: PlanetFixed, p: &LodParams) -> f64 {
    let Some(parent) = tile.parent() else { return 0.0 };
    let d = surface_distance(map, parent, camera, p) / p.split_distance(parent.level());
    ((d - (1.0 - p.morph_band)) / p.morph_band).clamp(0.0, 1.0)
}

impl NodeSelection {
    /// Compact text for snapshots: node count, per-level histogram, hash of the full list.
    pub fn summary(&self) -> String {
        let mut hist = [0usize; 29];
        for n in &self.nodes {
            hist[n.tile.level() as usize] += 1;
        }
        let levels: Vec<String> = hist.iter().enumerate().filter(|(_, c)| **c > 0).map(|(l, c)| format!("L{l}:{c}")).collect();
        let mut v: Vec<u64> = Vec::new();
        for n in &self.nodes {
            v.push(n.tile.raw());
            v.push(u64::from(n.morph.to_bits()));
        }
        format!("nodes {} [{}] hash {:016x}", self.nodes.len(), levels.join(" "), planet_core::hash::hash_u64s(0x6c6f64, &v))
    }
}

/// True when no point of the node, up to `max_height_m` above the sphere, can be seen over the planet's horizon.
/// Angles are measured at the planet centre: the node is visible if its angular distance from the camera, minus its
/// own angular radius, is within the camera's horizon angle plus the horizon angle of the highest terrain.
pub fn below_horizon(map: &dyn FaceMapping, tile: TileId, camera: PlanetFixed, p: &LodParams) -> bool {
    // A camera at or below the reference sphere is treated as sitting on it: the terrain horizon still applies.
    let d = camera.0.length().max(p.radius_m * (1.0 + 1e-12));
    let (centre, _) = bounding_sphere(map, tile, p);
    let (c_hat, n_hat) = (camera.0.normalized(), centre.normalized());
    let theta = libm::atan2(c_hat.cross(n_hat).length(), c_hat.dot(n_hat));
    let alpha_camera = libm::acos(p.radius_m / d);
    let alpha_terrain = libm::acos(p.radius_m / (p.radius_m + p.max_height_m));
    theta - angular_radius(map, tile) > alpha_camera + alpha_terrain
}

/// Largest angle at the planet centre between the tile centre and its corners and edge midpoints (the true angular
/// radius; the chord radius of the bounding sphere underestimates it).
pub fn angular_radius(map: &dyn FaceMapping, tile: TileId) -> f64 {
    let (s0, s1, t0, t1) = tile.bounds();
    let (sm, tm) = (0.5 * (s0 + s1), 0.5 * (t0 + t1));
    let c = tile.center_direction(map);
    let mut worst = 0.0f64;
    for (s, t) in [(s0, t0), (s1, t0), (s0, t1), (s1, t1), (sm, t0), (sm, t1), (s0, tm), (s1, tm)] {
        let d = planet_core::cube::face_to_direction(map, tile.face(), s, t);
        worst = worst.max(libm::atan2(c.cross(d).length(), c.dot(d)));
    }
    // Margin for the sampled boundary: the maximum is attained at a corner for these tiles; keep a small relative safety.
    worst * 1.001
}
