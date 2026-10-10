//! CDLOD-style terrain node selection on the cube-sphere quadtree (CON-03, ADR 0009).
//!
//! A pure function of the camera, the parameters and the tree: no GPU, no I/O, no clock. A node is split while
//! the screen-space size of its sample spacing exceeds a pixel threshold; the leaves are then balanced so that
//! neighbouring leaves differ by at most one level, and each leaf gets a morph interval (distances between which it
//! blends towards its parent's geometry) so that a merge or split never pops.

use crate::camera::{Camera, Frustum};
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

    /// Upper bound of a node's bounding-sphere radius at `level`: three quarters of the tile edge (half diagonal 0.71, margin).
    pub fn node_radius_bound(&self, level: u8) -> f64 {
        0.75 * self.sample_spacing(level) * f64::from(self.cells)
    }

    /// Vertex distances `(start, end)` between which a node of `level >= 1` morphs from its own grid (0) to its parent's (1).
    /// The end is the parent's split distance, so every vertex is fully morphed when the parent stops splitting. The start is
    /// never earlier than `split(level) + 2 r`: a node that still borders finer leaves has vertices up to that far away and
    /// must not have started to morph yet, or the finer side cannot meet it.
    pub fn morph_interval(&self, level: u8) -> (f64, f64) {
        let end = self.split_distance(level - 1);
        let start = ((1.0 - self.morph_band) * end).max(self.split_distance(level) + 2.0 * self.node_radius_bound(level));
        (start, end)
    }

    /// Reject parameters for which crack-free morphing is impossible (the morph interval would vanish at some level).
    pub fn validate(&self) -> Result<(), String> {
        for level in 1..=self.max_level {
            let (start, end) = self.morph_interval(level);
            if end - start < 0.05 * end {
                return Err(format!(
                    "level {level}: morph interval [{start:.1}, {end:.1}] m is empty; increase viewport_h_px / tau_px or reduce cells (need viewport_h_px / tau_px > about {:.0} for {} cells)",
                    2.0 * self.cells as f64,
                    self.cells
                ));
            }
        }
        Ok(())
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

/// Per-tile geometry used by selection: computed once per visited tile (each evaluation needs nine face-to-direction
/// transforms with transcendentals) and shared by the horizon test, the frustum test, the split distance and the morph.
/// Per-node height bounds (LOD-008) belong here too.
#[derive(Clone, Copy, Debug)]
struct TileGeom {
    centre: Vec3,
    radius: f64,
    angular_radius: f64,
}

fn tile_geom(map: &dyn FaceMapping, tile: TileId, p: &LodParams) -> TileGeom {
    // The eight boundary directions are evaluated once for both the chord radius of the bounding sphere and the
    // angular radius (the same expressions as `bounding_sphere` and `angular_radius`, so results are bit-identical).
    let (s0, s1, t0, t1) = tile.bounds();
    let (sm, tm) = (0.5 * (s0 + s1), 0.5 * (t0 + t1));
    let centre_dir = tile.center_direction(map);
    let centre = centre_dir * p.radius_m;
    let (mut radius, mut worst) = (0.0f64, 0.0f64);
    for (s, t) in [(s0, t0), (s1, t0), (s0, t1), (s1, t1), (sm, t0), (sm, t1), (s0, tm), (s1, tm)] {
        let d = planet_core::cube::face_to_direction(map, tile.face(), s, t);
        radius = radius.max((d * p.radius_m - centre).length());
        worst = worst.max(libm::atan2(centre_dir.cross(d).length(), centre_dir.dot(d)));
    }
    TileGeom { centre, radius, angular_radius: worst * 1.001 }
}

/// Memo of [`TileGeom`] by tile for one selection (only ever looked up by key, so hash order cannot leak into results).
struct GeomCache<'a> {
    map: &'a dyn FaceMapping,
    p: &'a LodParams,
    tiles: std::collections::HashMap<TileId, TileGeom, std::hash::BuildHasherDefault<IdHasher>>,
}

impl GeomCache<'_> {
    fn get(&mut self, tile: TileId) -> TileGeom {
        let (map, p) = (self.map, self.p);
        *self.tiles.entry(tile).or_insert_with(|| tile_geom(map, tile, p))
    }
}

fn surface_distance_geom(g: &TileGeom, camera: PlanetFixed) -> f64 {
    ((camera.0 - g.centre).length() - g.radius).max(0.0)
}

fn children_of(t: TileId) -> [TileId; 4] {
    t.children().expect("below the maximum level")
}

/// Select the nodes to draw for a camera position, ignoring view direction (whole visible hemisphere).
pub fn select_nodes(map: &dyn FaceMapping, camera: PlanetFixed, p: &LodParams) -> NodeSelection {
    select_impl(map, camera, None, p)
}

/// Select the nodes to draw for a camera, also dropping nodes outside its side frustum planes (padded by the terrain height).
pub fn select_nodes_in_view(map: &dyn FaceMapping, camera: &Camera, p: &LodParams) -> NodeSelection {
    select_impl(map, camera.position, Some(camera.frustum()), p)
}

fn select_impl(map: &dyn FaceMapping, camera: PlanetFixed, frustum: Option<Frustum>, p: &LodParams) -> NodeSelection {
    let mut geoms = GeomCache { map, p, tiles: Default::default() };
    let mut leaves: BTreeSet<TileId> = BTreeSet::new();
    let mut stack: Vec<TileId> = Face::ALL.iter().map(|&f| TileId::new(f, 0, 0, 0).expect("root")).collect();
    while let Some(t) = stack.pop() {
        let g = geoms.get(t);
        if p.cull_horizon && below_horizon_at(g.centre, g.angular_radius, camera, p) {
            continue;
        }
        if let Some(fr) = &frustum {
            if fr.culls_sphere(g.centre, g.radius + p.max_height_m) {
                continue;
            }
        }
        if t.level() < p.max_level && surface_distance_geom(&g, camera) < p.split_distance(t.level()) {
            stack.extend(children_of(t));
        } else {
            leaves.insert(t);
        }
    }
    balance_leaves(map, &mut leaves);
    let nodes = leaves
        .iter()
        .map(|&tile| {
            let distance_m = surface_distance_geom(&geoms.get(tile), camera);
            let morph = match tile.parent() {
                None => 0.0,
                Some(parent) => {
                    let d = surface_distance_geom(&geoms.get(parent), camera) / p.split_distance(parent.level());
                    ((d - (1.0 - p.morph_band)) / p.morph_band).clamp(0.0, 1.0)
                }
            };
            SelectedNode { tile, morph: morph as f32, distance_m }
        })
        .collect();
    NodeSelection { nodes }
}

/// Hasher for tile ids (already well-mixed bit patterns): one multiply, no SipHash.
#[derive(Default)]
struct IdHasher(u64);

impl std::hash::Hasher for IdHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    }
    fn write_u64(&mut self, v: u64) {
        self.0 = v.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    }
}

type IdSet = std::collections::HashSet<TileId, std::hash::BuildHasherDefault<IdHasher>>;

/// Split leaves until neighbouring leaves differ by at most one level. `leaves` must be a quadtree partition (what
/// selection produces: no leaf is an ancestor of another).
///
/// A leaf that is too coarse for a neighbour of leaf `t` is the one leaf that is an ancestor of that neighbour two or
/// more levels up. The walk up from the neighbour stops at the first ancestor that already has leaf descendants (an
/// "internal" tile): in a partition none of its ancestors can be a leaf. The first round checks every leaf; later
/// rounds only the children created by the previous one and the finer leaves that triggered a split (a split can only
/// make those violate the rule). The result is the unique minimal refinement, independent of the order of splits.
pub fn balance_leaves(map: &dyn FaceMapping, leaves: &mut BTreeSet<TileId>) {
    let mut set: IdSet = leaves.iter().copied().collect();
    let mut internal = IdSet::default();
    for &leaf in set.iter() {
        let mut a = leaf.parent();
        while let Some(x) = a {
            if !internal.insert(x) {
                break;
            }
            a = x.parent();
        }
    }
    debug_assert!(set.iter().all(|l| !internal.contains(l)), "balance_leaves needs a partition: a leaf is an ancestor of another");
    let mut work: Vec<TileId> = leaves.iter().copied().collect();
    while !work.is_empty() {
        // (too-coarse leaf, the finer leaf that found it)
        let mut found: Vec<(TileId, TileId)> = Vec::new();
        for &t in &work {
            if t.level() < 2 || !set.contains(&t) {
                continue;
            }
            for side in Side::ALL {
                let mut a = t.neighbor(map, side).parent().and_then(|p| p.parent());
                while let Some(x) = a {
                    if set.contains(&x) {
                        found.push((x, t));
                        break;
                    }
                    if internal.contains(&x) {
                        break;
                    }
                    a = x.parent();
                }
            }
        }
        work.clear();
        found.sort_unstable();
        found.dedup();
        for (x, finer) in found {
            if set.remove(&x) {
                internal.insert(x);
                let kids = children_of(x);
                set.extend(kids);
                work.extend(kids);
            }
            // The finer leaf is checked again: the children of `x` may still be two levels coarser than it.
            work.push(finer);
        }
        work.sort_unstable();
        work.dedup();
    }
    *leaves = set.into_iter().collect();
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
    let (centre, _) = bounding_sphere(map, tile, p);
    below_horizon_at(centre, angular_radius(map, tile), camera, p)
}

fn below_horizon_at(centre: Vec3, angular_radius: f64, camera: PlanetFixed, p: &LodParams) -> bool {
    // A camera at or below the reference sphere is treated as sitting on it: the terrain horizon still applies.
    let d = camera.0.length().max(p.radius_m * (1.0 + 1e-12));
    let (c_hat, n_hat) = (camera.0.normalized(), centre.normalized());
    let theta = libm::atan2(c_hat.cross(n_hat).length(), c_hat.dot(n_hat));
    let alpha_camera = libm::acos(p.radius_m / d);
    let alpha_terrain = libm::acos(p.radius_m / (p.radius_m + p.max_height_m));
    theta - angular_radius > alpha_camera + alpha_terrain
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
