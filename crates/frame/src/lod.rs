//! CDLOD-style terrain node selection on the cube-sphere quadtree (CON-03, ADR 0009).
//!
//! A pure function of the camera, the parameters and the tree: no GPU, no I/O, no clock. A node is split while
//! the screen-space size of its sample spacing exceeds a pixel threshold; the leaves are then balanced so that
//! neighbouring leaves differ by at most one level, and each leaf gets a morph interval (distances between which it
//! blends towards its parent's geometry) so that a merge or split never pops.

use crate::camera::{Camera, Frustum};
use planet_core::cube::FaceMapping;
use planet_core::{Face, PlanetFixed, Side, TileId, Vec3};
use planet_generators::height::HeightField;
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq)]
pub struct LodParams {
    pub radius_m: f64,
    /// Highest terrain height above the sphere, metres (padding for horizon and frustum culling).
    pub max_height_m: f64,
    pub fov_y_rad: f64,
    pub viewport_h_px: f64,
    /// Split while the projected sample spacing is larger than this many pixels.
    pub tau_px: f64,
    /// Cells per tile edge (samples are `cells + 1` per edge).
    pub cells: u32,
    /// The finest geometry level: the one source of truth for the cap (LOD-010). Selection, the residency set R, prefetch, the
    /// cover plan, the draw set and the compact tile format's CON-13 guard all read it from here. The production value is
    /// [`production_finest_level`]`(cells)`; `--finest-level` and tests set it explicitly.
    pub max_level: u8,
    /// Fraction of the split distance over which a node morphs (0.3 = last 30 % before it merges into its parent).
    pub morph_band: f64,
    /// Drop nodes that lie entirely below the horizon (padded by `max_height_m`).
    pub cull_horizon: bool,
}

/// SPIKE-12 ONLY (never merged): the render floor, the lowest level a leaf may have (0: none).
static SPIKE_FLOOR: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

pub fn spike_set_floor(level: u8) {
    SPIKE_FLOOR.store(level, std::sync::atomic::Ordering::SeqCst);
}

pub fn spike_floor() -> u8 {
    SPIKE_FLOOR.load(std::sync::atomic::Ordering::SeqCst)
}

/// log2 of the cells per face edge at the production finest level: a face edge is `2^level` tiles of `cells` cells, and the
/// production cap fixes `2^level * cells = 2^27` (134 million cells; `radius * pi/2 / 2^27` = 7.46 cm of vertex spacing on an
/// Earth-sized body), so level 22 at 32 cells and 21 at 64 (ADR 0033 point 6, owner 2026-10-15).
pub const PRODUCTION_FACE_CELLS_LOG2: u32 = 27;

/// The deepest level the production cap ever names (the level before SPIKE-07, kept as the reference cap).
pub const REFERENCE_FINEST_LEVEL: u8 = 26;

/// The production finest level for tiles of `cells` cells: `27 - ceil(log2(cells))`, so equal vertex spacing (about 7.5 cm at
/// Earth's radius) whatever the tile size; at most [`REFERENCE_FINEST_LEVEL`]. 32 cells give 22, 64 give 21, 16 give 23.
pub fn production_finest_level(cells: u32) -> u8 {
    let log2 = cells.max(1).checked_next_power_of_two().map_or(32, u32::trailing_zeros);
    (PRODUCTION_FACE_CELLS_LOG2.saturating_sub(log2) as u8).min(REFERENCE_FINEST_LEVEL)
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
            max_level: production_finest_level(32),
            morph_band: 0.3,
            cull_horizon: true,
        }
    }

    /// Distance (to the node's bounding volume, LOD-008) below which a node of `level` is split.
    pub fn split_distance(&self, level: u8) -> f64 {
        self.sample_spacing(level) * self.viewport_h_px / (2.0 * libm::tan(self.fov_y_rad / 2.0) * self.tau_px)
    }

    /// Upper bound of a node's bounding-sphere radius at `level`: 0.82 of the tile edge at the face centre. The half
    /// diagonal is 0.71 there, and tangent-warp tiles near cube corners are up to 9 % larger than at the face centre
    /// (measured chord radius over this edge: 0.816 at most; a test checks the bound on random tiles).
    pub fn node_radius_bound(&self, level: u8) -> f64 {
        0.82 * self.sample_spacing(level) * f64::from(self.cells)
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
    /// Distance from the camera to the node's bounding volume (height bounds included), metres.
    pub distance_m: f64,
    /// Vertex distances `(start, end)` between which the node morphs to its parent's grid (see
    /// [`LodParams::morph_interval`]; the start also clears the height extent of its neighbours).
    pub morph_start_m: f64,
    pub morph_end_m: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NodeSelection {
    /// Leaves in ascending tile-id order (deterministic).
    pub nodes: Vec<SelectedNode>,
}

/// Bounding sphere of a tile on the reference sphere: centre on the surface, radius covering corners and edge midpoints.
/// Terrain height is not included: the first frustum test pads it by `max_height_m`, and selection uses the height-aware volume of the
/// node (`tile_volume`).
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
#[derive(Clone, Copy, Debug, PartialEq)]
struct TileGeom {
    /// Unit direction of the tile centre.
    dir: Vec3,
    /// Tile centre on the reference sphere.
    centre: Vec3,
    radius: f64,
    angular_radius: f64,
}

/// Bounding volume of a tile including its height bounds (LOD-008): the sphere around the tile's column at mid-height
/// that covers its horizontal extent (stretched to the highest point of the column) and half of its height range.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Volume {
    centre: Vec3,
    radius: f64,
    /// Half the height range, metres.
    half_range: f64,
    /// Upper height bound, metres.
    max_height: f64,
    /// The sphere the frustum cut tests: the volume above, with its upper bound raised to the field's water level (ADR 0053); the
    /// volume itself when the field names none.
    cull_centre: Vec3,
    cull_radius: f64,
}

fn tile_volume(g: &TileGeom, field: &dyn HeightField, p: &LodParams) -> Volume {
    let (lo, hi) = field.height_bounds(g.dir, g.angular_radius);
    let half_range = 0.5 * (hi - lo);
    let centre = g.dir * (p.radius_m + 0.5 * (hi + lo));
    // Terrain points sit further from the planet axis than reference points by up to the factor 1 + hi/R.
    let horizontal = g.radius * (1.0 + hi.max(0.0) / p.radius_m);
    let radius = extent(horizontal, half_range);
    let (cull_centre, cull_radius) = match field.water_cull_level().filter(|w| *w > hi) {
        Some(w) => (g.dir * (p.radius_m + 0.5 * (w + lo)), extent(g.radius * (1.0 + w.max(0.0) / p.radius_m), 0.5 * (w - lo))),
        None => (centre, radius),
    };
    Volume { centre, radius, half_range, max_height: hi, cull_centre, cull_radius }
}

/// Skirt depth of a tile with 5 % to spare (the mesh builder's rule, shared): the skirts are drawn, so the frustum cut must
/// keep their triangles in view.
fn skirt_depth(tile: TileId, p: &LodParams) -> f64 {
    1.05 * planet_generators::mesh::skirt_depth(tile.level(), p.radius_m)
}

/// Radius of a sphere covering a horizontal extent `a` and a vertical half range `b` (exactly `a` when `b` is zero).
fn extent(a: f64, b: f64) -> f64 {
    if b == 0.0 {
        a
    } else {
        (a * a + b * b).sqrt()
    }
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
    TileGeom { dir: centre_dir, centre, radius, angular_radius: worst * 1.001 }
}

type IdMap<V> = std::collections::HashMap<TileId, V, std::hash::BuildHasherDefault<IdHasher>>;

/// Memo of the per-tile geometry and height-aware volume across selections. Both are pure functions of the tile, the
/// face mapping, the radius and the height field, so a frame-to-frame cache changes no result (it is only looked up by
/// key, so hash order cannot leak into results); it is keyed by (face mapping, field implementation id, version and seed, radius): a `HeightField` with inputs beyond those must change one of them when it changes, or its entries go stale. It is emptied when the key changes and when it grows past
/// [`SelectionCache::MAX_TILES`]. Consecutive frames of a moving camera share most of their tiles, which is what
/// makes planning fit a frame.
#[derive(Default)]
pub struct SelectionCache {
    key: u64,
    misses: u64,
    tiles: IdMap<TileGeom>,
    volumes: IdMap<Volume>,
    /// Field height at each tile's centre direction (the plan's tile origin), computed on first use by the plan builder.
    origin_heights: IdMap<f64>,
    /// Mesh key of each tile for the grid size and finest level `mesh_key_params` (the key chains the cell count and the
    /// finest-level class of the tile, `finest_class_key`; a change of either starts it over).
    mesh_keys: IdMap<u64>,
    mesh_key_params: (u32, u8),
}

impl SelectionCache {
    /// Upper bound on cached tiles (some tens of MB) before the cache starts over.
    pub const MAX_TILES: usize = 400_000;

    pub fn new() -> SelectionCache {
        SelectionCache::default()
    }

    pub fn clear(&mut self) {
        self.tiles.clear();
        self.volumes.clear();
        self.origin_heights.clear();
        self.mesh_keys.clear();
    }

    /// Field height at the centre of `tile` (the plan's tile origin): `compute` runs only the first time a tile is asked
    /// for since the cache last started over (LOD-009). Valid only after a selection with the same map, field and radius
    /// has prepared the cache, which the plan builder guarantees.
    pub(crate) fn origin_height(&mut self, tile: TileId, compute: impl FnOnce() -> f64) -> f64 {
        if let Some(h) = self.origin_heights.get(&tile) {
            return *h;
        }
        let h = compute();
        if h.is_finite() {
            self.origin_heights.insert(tile, h);
        }
        h
    }

    /// The memoised origin height of `tile`, if any (the plan evaluates the missing ones of a selection as one batch).
    pub(crate) fn origin_height_if_known(&self, tile: TileId) -> Option<f64> {
        self.origin_heights.get(&tile).copied()
    }

    /// Mesh key of `tile` for a grid of `cells` cells per edge and a view whose finest level is `finest_level`: `compute` runs once
    /// per tile, grid size and finest level (LOD-009).
    pub(crate) fn mesh_key(&mut self, tile: TileId, cells: u32, finest_level: u8, compute: impl FnOnce() -> u64) -> u64 {
        if self.mesh_key_params != (cells, finest_level) {
            self.mesh_keys.clear();
            self.mesh_key_params = (cells, finest_level);
        }
        *self.mesh_keys.entry(tile).or_insert_with(compute)
    }

    /// Tiles whose geometry had to be computed (cache misses) since the cache was created.
    pub fn misses(&self) -> u64 {
        self.misses
    }

    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    fn prepare(&mut self, map: &dyn FaceMapping, field: &dyn HeightField, p: &LodParams) {
        let text = |s: &str| s.bytes().map(u64::from).collect::<Vec<_>>();
        let key = planet_core::hash::hash_u64s(
            0x0063_6163,
            &[
                planet_core::hash::hash_u64s(1, &text(map.id())),
                planet_core::hash::hash_u64s(2, &text(field.implementation_id())),
                u64::from(field.version()),
                field.seed(),
                // What the heights (and so the bounds) depend on beyond the seed: the body and the recorded read set.
                field.inputs_hash(),
                u64::from(field.body().0),
                p.radius_m.to_bits(),
                // The cull spheres of the cached volumes reach up to the field's water level.
                field.water_cull_level().map_or(0, |w| planet_core::hash::hash_u64s(5, &[w.to_bits()]) | 1),
                // The mesh key chains the reference surface (GEN-012): a field on another surface must not reuse keys.
                planet_core::hash::hash_u64s(3, &text(field.surface_id())),
            ],
        );
        if key != self.key
            || self.tiles.len() > Self::MAX_TILES
            || self.origin_heights.len() > Self::MAX_TILES
            || self.mesh_keys.len() > Self::MAX_TILES
        {
            self.clear();
            self.key = key;
        }
    }
}

/// One selection's view of a [`SelectionCache`].
struct GeomCache<'a> {
    map: &'a dyn FaceMapping,
    field: &'a dyn HeightField,
    p: &'a LodParams,
    tiles: &'a mut IdMap<TileGeom>,
    volumes: &'a mut IdMap<Volume>,
    misses: &'a mut u64,
}

impl GeomCache<'_> {
    fn get(&mut self, tile: TileId) -> TileGeom {
        let (map, p) = (self.map, self.p);
        let misses = &mut *self.misses;
        *self.tiles.entry(tile).or_insert_with(|| {
            *misses += 1;
            tile_geom(map, tile, p)
        })
    }

    /// Height-aware volume of a tile, computed on first use (the height bounds cost lattice lookups per octave).
    fn volume(&mut self, tile: TileId) -> Volume {
        if let Some(v) = self.volumes.get(&tile) {
            return *v;
        }
        let g = self.get(tile);
        let v = tile_volume(&g, self.field, self.p);
        self.volumes.insert(tile, v);
        v
    }

    /// Distance from the camera to the tile's volume, metres (0 inside).
    fn distance(&mut self, tile: TileId, camera: PlanetFixed) -> f64 {
        let v = self.volume(tile);
        ((camera.0 - v.centre).length() - v.radius).max(0.0)
    }

    /// A lower bound of [`GeomCache::distance`] that needs no height bounds: every volume lies within the field's global
    /// height range around the reference sphere.
    fn distance_lower_bound(&mut self, tile: TileId, camera: PlanetFixed) -> f64 {
        let g = self.get(tile);
        let h = self.field.max_abs_height_m();
        ((camera.0 - g.centre).length() - g.radius * (1.0 + h / self.p.radius_m) - h).max(0.0)
    }

    /// Radius of the volume of a tile of `level` with height extent `v` (the level's radius bound stands in for the exact
    /// horizontal extent, which keeps the margin of [`LodParams::morph_interval`] where terrain is flat).
    fn radius_bound(&self, level: u8, v: &Volume) -> f64 {
        extent(self.p.node_radius_bound(level) * (1.0 + v.max_height.max(0.0) / self.p.radius_m), v.half_range)
    }
}

fn children_of(t: TileId) -> [TileId; 4] {
    t.children().expect("below the maximum level")
}

/// Terrain at height zero everywhere: selection by distance to the reference sphere (what selection did before LOD-008).
struct Flat;

impl HeightField for Flat {
    fn height(&self, _: Vec3) -> f64 {
        0.0
    }
    fn seed(&self) -> u64 {
        0
    }
    fn implementation_id(&self) -> &'static str {
        "flat"
    }
    fn version(&self) -> u32 {
        1
    }
    fn max_abs_height_m(&self) -> f64 {
        0.0
    }
    fn height_bounds(&self, _: Vec3, _: f64) -> (f64, f64) {
        (0.0, 0.0)
    }
}

/// Select the nodes to draw for a camera position over flat terrain, ignoring view direction (whole visible hemisphere).
pub fn select_nodes(map: &dyn FaceMapping, camera: PlanetFixed, p: &LodParams) -> NodeSelection {
    select_impl(map, camera, None, &Flat, p, &mut SelectionCache::new())
}

/// Select the nodes to draw for a camera over flat terrain, also dropping nodes outside its side frustum planes (the
/// reference sphere plus the skirts: for real terrain use the `_field` variants, which cut with the tiles' height bounds).
pub fn select_nodes_in_view(map: &dyn FaceMapping, camera: &Camera, p: &LodParams) -> NodeSelection {
    select_impl(map, camera.position, Some(camera.frustum()), &Flat, p, &mut SelectionCache::new())
}

/// [`select_nodes`] over the terrain of `field`: distances go to bounding volumes that include each node's height
/// bounds (LOD-008).
pub fn select_nodes_field(map: &dyn FaceMapping, camera: PlanetFixed, field: &dyn HeightField, p: &LodParams) -> NodeSelection {
    select_impl(map, camera, None, field, p, &mut SelectionCache::new())
}

/// [`select_nodes`] reusing the per-tile work of earlier selections in `cache`: the result is identical.
pub fn select_nodes_cached(map: &dyn FaceMapping, camera: PlanetFixed, p: &LodParams, cache: &mut SelectionCache) -> NodeSelection {
    select_impl(map, camera, None, &Flat, p, cache)
}

/// [`select_nodes_field`] reusing the per-tile work of earlier selections in `cache`: the result is identical.
pub fn select_nodes_field_cached(
    map: &dyn FaceMapping,
    camera: PlanetFixed,
    field: &dyn HeightField,
    p: &LodParams,
    cache: &mut SelectionCache,
) -> NodeSelection {
    select_impl(map, camera, None, field, p, cache)
}

/// [`select_nodes_in_view`] over the terrain of `field`.
pub fn select_nodes_in_view_field(map: &dyn FaceMapping, camera: &Camera, field: &dyn HeightField, p: &LodParams) -> NodeSelection {
    select_impl(map, camera.position, Some(camera.frustum()), field, p, &mut SelectionCache::new())
}

/// [`select_nodes_in_view_field`] reusing the per-tile work of earlier selections in `cache`: the result is identical.
pub fn select_nodes_in_view_cached(
    map: &dyn FaceMapping,
    camera: &Camera,
    field: &dyn HeightField,
    p: &LodParams,
    cache: &mut SelectionCache,
) -> NodeSelection {
    select_impl(map, camera.position, Some(camera.frustum()), field, p, cache)
}

fn select_impl(
    map: &dyn FaceMapping,
    camera: PlanetFixed,
    frustum: Option<Frustum>,
    field: &dyn HeightField,
    p: &LodParams,
    cache: &mut SelectionCache,
) -> NodeSelection {
    cache.prepare(map, field, p);
    let SelectionCache { tiles, volumes, misses, .. } = cache;
    let mut geoms = GeomCache { map, field, p, tiles, volumes, misses };
    let mut leaves: BTreeSet<TileId> = BTreeSet::new();
    let mut stack: Vec<TileId> = Face::ALL.iter().map(|&f| TileId::new(f, 0, 0, 0).expect("root")).collect();
    while let Some(t) = stack.pop() {
        let g = geoms.get(t);
        if p.cull_horizon && below_horizon_at(g.centre, g.angular_radius, camera, p) {
            continue;
        }
        if let Some(fr) = &frustum {
            // The padded reference sphere rules out most tiles without height bounds; the tile's own volume (LOD-007)
            // then decides; both add the skirts' depth so that no drawn triangle of the tile can lie outside them.
            if fr.culls_sphere(g.centre, g.radius + p.max_height_m + skirt_depth(t, p)) {
                continue;
            }
            let v = geoms.volume(t);
            if fr.culls_sphere(v.cull_centre, v.cull_radius + skirt_depth(t, p)) {
                continue;
            }
        }
        let split = p.split_distance(t.level());
        // SPIKE-12: a render floor: tiles above it are always split.
        let forced = t.level() < spike_floor();
        // The bound that needs no height bounds rules most tiles out before the bounds are computed.
        if t.level() < p.max_level && (forced || (geoms.distance_lower_bound(t, camera) < split && geoms.distance(t, camera) < split)) {
            stack.extend(children_of(t));
        } else {
            leaves.insert(t);
        }
    }
    balance_leaves(map, &mut leaves);
    let mut nodes: Vec<SelectedNode> = leaves
        .iter()
        .map(|&tile| {
            let distance_m = geoms.distance(tile, camera);
            // SPIKE-12: a leaf at the render floor has no drawn parent: it does not morph (like a root).
            let (morph, morph_start_m, morph_end_m) = match tile.parent().filter(|_| tile.level() > spike_floor()) {
                None => (0.0, 1.0e30, 1.0e30),
                Some(parent) => {
                    let d = geoms.distance(parent, camera) / p.split_distance(parent.level());
                    let morph = ((d - (1.0 - p.morph_band)) / p.morph_band).clamp(0.0, 1.0);
                    let (start, end) = morph_interval_for(&mut geoms, tile);
                    (morph, start, end)
                }
            };
            SelectedNode { tile, morph: morph as f32, distance_m, morph_start_m, morph_end_m }
        })
        .collect();
    // Two leaves of one level share border vertices, which both morph by the shader formula: their intervals must be
    // equal or the border opens. One start per level, the largest any leaf of the level needs. Where the terrain keeps
    // every leaf at the base start (the reference field at production parameters) this is the level constant of old.
    let mut level_start = [0.0f64; 32];
    for n in nodes.iter().filter(|n| n.tile.level() > 0) {
        let s = &mut level_start[n.tile.level() as usize];
        *s = s.max(n.morph_start_m);
    }
    for n in nodes.iter_mut().filter(|n| n.tile.level() > 0) {
        n.morph_start_m = level_start[n.tile.level() as usize];
    }
    NodeSelection { nodes }
}

/// The per-tile bounds the selection cached for a set of tiles, handed to the frame path (STRM-016): the tile's geometry
/// (centre, chord radius, angular radius) and its height-aware volume (LOD-008), exactly the values the plan's own selection
/// computed. The frame path evaluates no height field, so it cannot compute a volume; it reads them from here. A tile that is
/// not in the set is judged by the geometry alone (looser, never tighter than the plan's cut).
///
/// The bounds are those of the residency set R of one camera position and one set of LOD parameters ([`TileBounds::complete_for`]).
/// For that position R holds every tile the plan of any view direction can select or descend through, so a tile that is not in
/// it is one the plan never needs: it has no tile of the selection under it (it lies beyond the horizon, or all its children do).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TileBounds {
    /// The position and parameters R was selected for (`None`: bounds made by hand, complete for nothing).
    for_view: Option<(PlanetFixed, LodParams)>,
    tiles: IdMap<(TileGeom, Volume)>,
}

impl TileBounds {
    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    /// Whether these are the bounds of R for exactly `position` and `p`: a tile they do not hold is then one the plan of any
    /// view direction from `position` never selects (STRM-015, LOD-007: the draw set is a subset of R).
    pub fn complete_for(&self, position: PlanetFixed, p: &LodParams) -> bool {
        self.for_view.as_ref().is_some_and(|(pos, lod)| *pos == position && lod == p)
    }
}

impl SelectionCache {
    /// The bounds of `tiles` (a cache hit for every tile a selection has visited), for the frame path's cover (see
    /// [`TileBounds`]).
    pub fn bounds_of(
        &mut self,
        tiles: impl IntoIterator<Item = TileId>,
        position: PlanetFixed,
        map: &dyn FaceMapping,
        field: &dyn HeightField,
        p: &LodParams,
    ) -> TileBounds {
        self.prepare(map, field, p);
        let SelectionCache { tiles: geom_memo, volumes, misses, .. } = self;
        let mut geoms = GeomCache { map, field, p, tiles: geom_memo, volumes, misses };
        let mut out = TileBounds { for_view: Some((position, p.clone())), tiles: IdMap::default() };
        for t in tiles {
            let v = geoms.volume(t);
            out.tiles.insert(t, (geoms.get(t), v));
        }
        out
    }
}

/// What the cover and the coverage test ask of a tile for one camera: the plan's own horizon and frustum culling (the padded
/// reference sphere, then the tight cut with the tile's height-bounds volume when the bounds hold it, LOD-007/008).
struct ViewTest<'a> {
    map: &'a dyn FaceMapping,
    camera: PlanetFixed,
    frustum: Frustum,
    p: &'a LodParams,
    bounds: &'a TileBounds,
    /// The bounds are R's for this camera position: a tile outside them is never needed.
    complete: bool,
    geoms: IdMap<TileGeom>,
}

impl<'a> ViewTest<'a> {
    fn new(map: &'a dyn FaceMapping, camera: &Camera, p: &'a LodParams, bounds: &'a TileBounds) -> Self {
        Self {
            map,
            camera: camera.position,
            frustum: camera.frustum(),
            p,
            bounds,
            complete: bounds.complete_for(camera.position, p),
            geoms: IdMap::default(),
        }
    }

    fn geom(&mut self, t: TileId) -> TileGeom {
        if let Some((g, _)) = self.bounds.tiles.get(&t) {
            return *g;
        }
        let (map, p) = (self.map, self.p);
        *self.geoms.entry(t).or_insert_with(|| tile_geom(map, t, p))
    }

    /// The plan's cull of the tile: beyond the horizon, or outside the frustum (the padded sphere, then the tight volume); or, when
    /// the bounds are complete for the camera, not in R at all (the plan has nothing under it).
    fn culled(&mut self, t: TileId) -> bool {
        if self.complete && !self.bounds.tiles.contains_key(&t) {
            return true;
        }
        let g = self.geom(t);
        let p = self.p;
        if p.cull_horizon && below_horizon_at(g.centre, g.angular_radius, self.camera, p) {
            return true;
        }
        let skirt = skirt_depth(t, p);
        if self.frustum.culls_sphere(g.centre, g.radius + p.max_height_m + skirt) {
            return true;
        }
        self.bounds.tiles.get(&t).is_some_and(|(_, v)| self.frustum.culls_sphere(v.cull_centre, v.cull_radius + skirt))
    }

    /// Distance from the camera to the tile: to its height-aware volume when the bounds hold it (what the plan measures),
    /// otherwise to its resident origin less its radius (the relief within a tile is small against the tile's own radius).
    fn distance(&mut self, t: TileId, origin: PlanetFixed) -> f64 {
        match self.bounds.tiles.get(&t) {
            Some((_, v)) => ((self.camera.0 - v.centre).length() - v.radius).max(0.0),
            None => {
                let g = self.geom(t);
                ((self.camera.0 - origin.0).length() - g.radius).max(0.0)
            }
        }
    }
}

/// The cover of a view (STRM-002, STRM-016, ADR 0032 point 5) with no cached bounds: [`cover_selection_bounded`] judging every
/// tile by its geometry alone (the padded-sphere cull, the origin-based distance).
pub fn cover_selection(
    map: &dyn FaceMapping,
    camera: &Camera,
    p: &LodParams,
    resident: &dyn Fn(TileId) -> Option<PlanetFixed>,
) -> Vec<TileId> {
    cover_selection_bounded(map, camera, p, resident, &TileBounds::default())
}

/// The cover of a view: the plan's own selection with "resident" as the condition for descending, a pure function of the
/// camera, the LOD parameters, the resident set and the cached tile bounds that evaluates no height field. Walk down from the
/// resident roots; a tile is split while the plan's LOD test says so (the distance to its volume, below the level's split
/// distance) and every child that is in view is resident, otherwise it is drawn itself. In view means what the plan means: not
/// beyond the horizon, not outside the frustum (the padded sphere), not outside the tight cut (the tile's height-bounds
/// volume, LOD-007/008). A child that the horizon, the frustum or the tight cut culls never stops its parent's descent. The
/// leaves are then balanced as the plan balances them (the 2:1 rule), where the tiles of the split are resident; a split with a
/// tile missing is undone. With every tile of the residency set resident the cover equals the plan of the camera, node for node
/// (the bounds hold the tiles the plan visits). Always a subset of the resident tiles, in ascending tile-id order, with no tile
/// an ancestor of another. Tiles that `bounds` does not hold are judged by their geometry alone (a looser cull, the
/// origin-based distance).
pub fn cover_selection_bounded(
    map: &dyn FaceMapping,
    camera: &Camera,
    p: &LodParams,
    resident: &dyn Fn(TileId) -> Option<PlanetFixed>,
    bounds: &TileBounds,
) -> Vec<TileId> {
    let mut view = ViewTest::new(map, camera, p, bounds);
    let mut leaves: BTreeSet<TileId> = BTreeSet::new();
    let mut stack: Vec<TileId> = Vec::new();
    for f in Face::ALL {
        let root = TileId::new(f, 0, 0, 0).expect("root");
        if resident(root).is_some() && !view.culled(root) {
            stack.push(root);
        }
    }
    while let Some(t) = stack.pop() {
        let origin = resident(t).expect("the walk visits resident tiles only");
        if t.level() < p.max_level && view.distance(t, origin) < p.split_distance(t.level()) {
            let seen: Vec<TileId> = children_of(t).into_iter().filter(|&c| !view.culled(c)).collect();
            if seen.iter().all(|&c| resident(c).is_some()) {
                // With every child out of view nothing of the tile can be seen, and the plan draws nothing there either.
                stack.extend(seen);
                continue;
            }
        }
        leaves.insert(t);
    }
    balance_leaves(map, &mut leaves);
    // A balancing split whose tiles are not all resident is undone: the nearest resident ancestor stands in (it is a leaf of
    // the walk or an ancestor of one), and a stand-in covers the finer tiles under it.
    let mut stand_ins: BTreeSet<TileId> = BTreeSet::new();
    for leaf in leaves {
        let mut x = leaf;
        while resident(x).is_none() {
            x = x.parent().expect("a root of the walk is resident");
        }
        stand_ins.insert(x);
    }
    stand_ins
        .iter()
        .copied()
        .filter(|t| {
            let mut up = t.parent();
            while let Some(a) = up {
                if stand_ins.contains(&a) {
                    return false;
                }
                up = a.parent();
            }
            true
        })
        .collect()
}

/// Whether a plan that draws `drawn` serves the view of `camera` (STRM-016): it draws every tile of the view's own cover
/// (`view_cover`, [`cover_selection_bounded`] for `camera`) or, in its place, finer tiles that fill all of its children that
/// are in view, recursively. A tile of the cover that the plan covers only with a coarser ancestor does not count (the plan is
/// coarser than the view can be drawn, which would blur it), and nor does one the plan does not draw at all (a sliver that
/// turned into view). A conservative test: the plan then has, for every node the camera sees, a tile at that node's level or a
/// finer one; its cost is a walk over the cover with cached bounds. Pure.
pub fn plan_covers_view(
    map: &dyn FaceMapping,
    camera: &Camera,
    p: &LodParams,
    bounds: &TileBounds,
    view_cover: &[TileId],
    drawn: &BTreeSet<TileId>,
) -> bool {
    let mut above: IdSet = IdSet::default();
    for &d in drawn {
        let mut up = d.parent();
        while let Some(a) = up {
            if !above.insert(a) {
                break;
            }
            up = a.parent();
        }
    }
    let mut view = ViewTest::new(map, camera, p, bounds);
    let mut stack: Vec<TileId> = view_cover.to_vec();
    while let Some(t) = stack.pop() {
        if drawn.contains(&t) {
            continue;
        }
        if !above.contains(&t) {
            return false;
        }
        stack.extend(children_of(t).into_iter().filter(|&c| !view.culled(c)));
    }
    true
}

/// The morph interval a leaf needs: [`LodParams::morph_interval`] with the margin for the extent of its four same-level
/// neighbours taken from their height bounds (the selection then equalises the start over each level). A vertex of the leaf on a border with a finer neighbour is
/// up to two bounding radii of the neighbour's volume further away than that volume's surface, and must not have started
/// to morph by then.
fn morph_interval_for(geoms: &mut GeomCache, tile: TileId) -> (f64, f64) {
    let p = geoms.p;
    let level = tile.level();
    let end = p.split_distance(level - 1);
    let base = (1.0 - p.morph_band) * end;
    let split = p.split_distance(level);
    let h = geoms.field.max_abs_height_m();
    // Even the largest possible volume radius leaves the base start in force: no bounds needed.
    if split + 2.0 * (p.node_radius_bound(level) * (1.0 + h / p.radius_m) + h) <= base {
        return (base, end);
    }
    let mut rho = 0.0f64;
    for side in Side::ALL {
        let v = geoms.volume(tile.neighbor(geoms.map, side));
        rho = rho.max(geoms.radius_bound(level, &v));
    }
    (base.max(split + 2.0 * rho), end)
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

#[cfg(test)]
mod reuse_tests {
    use super::*;

    // spec: LOD-009
    #[test]
    fn the_mesh_key_is_computed_once_per_tile_grid_size_and_finest_level() {
        let mut cache = SelectionCache::new();
        let tile = TileId::new(Face::ALL[0], 3, 1, 1).expect("tile");
        let mut calls = 0;
        for cells in [16, 16, 16, 32, 32, 16] {
            cache.mesh_key(tile, cells, 22, || {
                calls += 1;
                u64::from(cells)
            });
        }
        // 16 (miss), 16, 16 (hits), 32 (miss, restart), 32 (hit), 16 (miss, restart).
        assert_eq!(calls, 3);
        assert_eq!(cache.mesh_key(tile, 16, 22, || unreachable!("served from the memo")), 16);
        // A new finest level starts the memo over too (the key carries the finest-level class of the tile).
        assert_eq!(cache.mesh_key(tile, 16, 23, || 7), 7);
        assert_eq!(cache.mesh_key(tile, 16, 23, || unreachable!("served from the memo")), 7);
    }
}
