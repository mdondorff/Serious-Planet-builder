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
use std::collections::{BTreeMap, BTreeSet};

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
    /// The render floor (LOD-011, ADR 0062 point 3): no node of a lower level is selected, drawn or resident; the leaves at the floor
    /// are the roots of the draw set. The production value is [`DEFAULT_RENDER_FLOOR`]; 0 reproduces the selection that starts at the
    /// six face tiles. The effective floor is [`LodParams::floor_level`].
    pub min_level: u8,
    /// Fraction of the split distance over which a node morphs (0.3 = last 30 % before it merges into its parent).
    pub morph_band: f64,
    /// Drop nodes that lie entirely below the horizon (padded by `max_height_m`).
    pub cull_horizon: bool,
}

/// How much of the morph interval of every level [`LodParams::coarse`] keeps beyond what [`LodParams::validate`] needs, as a share of the
/// level's end distance: the plan adds the height extent of a tile's neighbours to the interval's start, and a coarse view that sat exactly
/// on the limit was refused by it (SPIKE-19).
pub const COARSE_MARGIN: f64 = 0.01;

/// The levels [`LodParams::coarse`] checks against the plan's own start are those whose node radius is at least this many times the
/// body's greatest height: there a tile is large against the heights, the volume of its neighbourhood is bounded by the field's global
/// range (the only bound a field has for tiles that big), and the plan's start is the worst case the check takes. At finer levels the plan
/// reads the tile's own height bounds, far below the global one, and the check by the node radius alone is the one that matters. Measured
/// on the production 1080p parameters (the coarse factor is 2.59 at 4, 2.28 at 1, 1.74 at 0.5 and 1.06 at 0.25 or less) and on a sweep of
/// poses from 20 km up over a field with only a global bound (SPIKE-19 review): at 4 and at 1 the plan still refused the coarse view at
/// levels 8 to 10; 0.5 includes them and keeps a coarse view of about 1.7 times the error bound. A refusal that remains falls back to
/// the base parameters (VIEW-014) and no longer ends the window.
pub const COARSE_WORST_CASE_RADIUS_FACTOR: f64 = 0.5;

/// The factor of the editor's coarse-LOD inspection switch (VIEW-014): the error bound `tau_px` times this, so the mesh edges are about
/// four times longer on the screen, where the parameters allow it (see [`LodParams::coarse`]).
pub const COARSE_LOD_FACTOR: f64 = 4.0;

impl LodParams {
    /// These parameters with the error bound multiplied by [`COARSE_LOD_FACTOR`], or by the largest factor below it that still
    /// satisfies [`validate`](LodParams::validate) (the morph interval of every level must stay non-empty, which needs `viewport_h_px /
    /// tau_px` above about twice the cells: the production 1080p parameters, error bound 6 px and 32 cells, allow about 2.6): a view for
    /// inspecting the real triangles, not a change of the data. Decided here, in the plan's inputs, never in a shader. The result
    /// is valid whenever `self` is.
    pub fn coarse(&self) -> LodParams {
        let at = |factor: f64| LodParams { tau_px: self.tau_px * factor, ..self.clone() };
        let valid = |p: &LodParams| self.coarse_keeps_the_plan_valid(p);
        if valid(&at(COARSE_LOD_FACTOR)) {
            return at(COARSE_LOD_FACTOR);
        }
        // The largest valid factor in [1, 4): the validity is monotone in the error bound, so a bisection finds it.
        let (mut ok, mut bad) = (1.0, COARSE_LOD_FACTOR);
        for _ in 0..40 {
            let mid = 0.5 * (ok + bad);
            if valid(&at(mid)) {
                ok = mid;
            } else {
                bad = mid;
            }
        }
        at(ok)
    }

    /// The levels at which the plan's start with the neighbourhood's height extent is the one that matters ([`COARSE_WORST_CASE_RADIUS_FACTOR`]).
    pub fn worst_case_levels(&self) -> Vec<u8> {
        (1..=self.max_level).filter(|l| self.node_radius_bound(*l) >= COARSE_WORST_CASE_RADIUS_FACTOR * self.max_height_m).collect()
    }

    /// Whether `coarse` (these parameters with a larger error bound) is one the plan accepts wherever it accepts these (SPIKE-19): the
    /// morph interval of every level stays non-empty with [`COARSE_MARGIN`] to spare ([`validate`](LodParams::validate)'s rule), and at every
    /// large level ([`COARSE_WORST_CASE_RADIUS_FACTOR`]) where these parameters meet the plan's own start (the neighbourhood's height extent
    /// added, [`morph_interval_worst`](LodParams::morph_interval_worst)) `coarse` meets it too, with the margin.
    fn coarse_keeps_the_plan_valid(&self, coarse: &LodParams) -> bool {
        if coarse.check(COARSE_MARGIN).is_err() {
            return false;
        }
        self.worst_case_levels().into_iter().all(|level| {
            let (s, e) = self.morph_interval_worst(level);
            if e - s < 0.05 * e {
                return true;
            }
            let (s, e) = coarse.morph_interval_worst(level);
            e - s >= (0.05 + COARSE_MARGIN) * e
        })
    }

    /// The factor [`coarse`](LodParams::coarse) applies to the error bound for these parameters (1 when nothing can be coarser).
    pub fn coarse_factor(&self) -> f64 {
        self.coarse().tau_px / self.tau_px
    }
}

/// The production render floor (LOD-011, ADR 0062 point 3): levels 0 to 2 are not drawn, the 384 tiles of level 3 are the roots.
pub const DEFAULT_RENDER_FLOOR: u8 = 3;

/// The effective render floor of a floor asked for and a finest level: the lower of the two (LOD-011), so a body whose finest level
/// is 0 to 2 draws down to its finest level without a flag. The one place the rule is written (`LodParams::floor_level` and the
/// application's options take it from here).
pub fn effective_floor(min_level: u8, max_level: u8) -> u8 {
    min_level.min(max_level)
}

/// An explicit render floor (`--floor-level N`) must not lie above the finest level (LOD-011): it is refused before any work, with an
/// error naming both values. The default floor needs no check: the effective floor is [`LodParams::floor_level`], the lower of the two.
pub fn check_explicit_floor(floor: u8, max_level: u8) -> Result<(), String> {
    if floor > max_level {
        return Err(format!(
            "--floor-level {floor} is above the finest level {max_level}: no tile of a level below the floor is drawn, so nothing would be; \
             lower --floor-level or raise --finest-level"
        ));
    }
    Ok(())
}

/// How many of the selection's screen-space tolerances the shore ramp must span at least ([`LodParams::shore_ramp_per_m`], REND-030).
pub const SHORE_RAMP_K: f64 = 2.0;

/// The tallest ramp the tolerance asks for (m), REND-030: the water pass absorbs 0.5 per metre of depth by default, so below 8 m
/// (four e-folds, 98 % absorbed) the seabed's own colour is hidden whatever the ramp is, and the flights of the flash test need
/// 3 m. Above it the ramp would only turn the colour of the deep seabed into the beach's in terrain-only views.
pub const SHORE_RAMP_MAX_M: f64 = 8.0;

/// The ramp height at a fragment `distance_m` from the camera, REND-030: `per_m * distance_m` capped at [`SHORE_RAMP_MAX_M`], where `per_m` is
/// [`LodParams::shore_ramp_per_m`] (carried in the plan). It depends on the fragment's distance and on nothing about the node that draws it,
/// so one ground point gets one ramp whichever level of tile is drawn there (the cover, stale plans and prefetched tiles draw tiles the
/// exact plan would not). The WGSL `shore_ramp_at` is the same function.
pub fn shore_ramp_at(per_m: f64, distance_m: f64) -> f64 {
    (per_m * distance_m).clamp(0.0, SHORE_RAMP_MAX_M)
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
            radius_m: planet_core::EARTH_RADIUS_M,
            max_height_m: 9_000.0,
            fov_y_rad: 60f64.to_radians(),
            viewport_h_px: 1080.0,
            tau_px: 6.0,
            cells: 32,
            max_level: production_finest_level(32),
            min_level: DEFAULT_RENDER_FLOOR,
            morph_band: 0.3,
            cull_horizon: true,
        }
    }

    /// The effective render floor (LOD-011): `min(min_level, max_level)`, so a body whose finest level is 0 to 2 draws down to its
    /// finest level without a flag. Every tile of this level is a root; there are `6 * 4^floor` of them (384 at floor 3).
    pub fn floor_level(&self) -> u8 {
        effective_floor(self.min_level, self.max_level)
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

    /// The morph interval the plan makes at worst for a node of `level >= 1` on a body whose height is at most `max_height_m`
    /// (`morph_interval_for`): the start also clears the volume of the node's neighbours, whose radius is at most the node radius bound
    /// grown by the heights (`extent(r (1 + H / R), H)`, the largest the plan ever takes). [`LodParams::morph_interval`] is the same
    /// without that term.
    pub fn morph_interval_worst(&self, level: u8) -> (f64, f64) {
        let h = self.max_height_m;
        let rho = extent(self.node_radius_bound(level) * (1.0 + h / self.radius_m), h);
        let end = self.split_distance(level - 1);
        let start = ((1.0 - self.morph_band) * end).max(self.split_distance(level) + 2.0 * rho);
        (start, end)
    }

    /// Reject parameters for which crack-free morphing is impossible (the morph interval would vanish at some level), by the node radius
    /// alone. The plan adds the height extent of a tile's neighbours to the start where the tile's height bounds are wide (the coarse
    /// levels over a body with a global bound), so it can still refuse parameters accepted here; [`coarse`](LodParams::coarse) keeps
    /// that out of the view it derives.
    pub fn validate(&self) -> Result<(), String> {
        self.check(0.0)
    }

    /// [`validate`](LodParams::validate) with `margin` (a share of each level's end distance) kept in addition to the 5 % the interval needs.
    fn check(&self, margin: f64) -> Result<(), String> {
        let per_m = self.shore_ramp_per_m();
        if !(per_m.is_finite() && per_m > 0.0) {
            return Err(format!(
                "tau {} px, viewport {} px and field of view {} rad give a shore ramp growth of {per_m} per metre; all three must be finite and positive",
                self.tau_px, self.viewport_h_px, self.fov_y_rad
            ));
        }
        for level in 1..=self.max_level {
            let (start, end) = self.morph_interval(level);
            if !morph_room(start, end, margin) {
                return Err(format!(
                    "level {level}: morph interval [{start:.1}, {end:.1}] m is empty; increase viewport_h_px / tau_px or reduce cells (need viewport_h_px / tau_px > about {:.0} for {} cells)",
                    2.0 * self.cells as f64,
                    self.cells
                ));
            }
        }
        Ok(())
    }

    /// The growth of the shore ramp with the distance from the camera, in metres per metre (REND-030, ATMO-020): `SHORE_RAMP_K` times the world
    /// size of one pixel of the selection's screen-space tolerance per metre of distance, `tau_px * pixel_angle`, with `pixel_angle =
    /// 2 tan(fov_y / 2) / viewport_h_px` (the same quantity [`split_distance`](LodParams::split_distance) divides by). A node is drawn only
    /// where its mesh's error is below that tolerance, so at a fragment `d` metres away the height of one ground point differs between a
    /// mesh, its parent's and the morph between them by a fraction of `tau_px * pixel_angle * d`. The ramp is `SHORE_RAMP_K` times it ([`shore_ramp_at`]):
    /// k = 1 left 4.3 % of the shore pixels of the glide flashing, k = 1.5, 2 and 3 none; 2 is taken (measured with the former per-node rule, which is about 1.4 to 2 times this one; the distance rule is checked by the same flights, REND-030). A ramp narrower than the error makes the
    /// seabed-to-shore albedo, which differs by a factor of eight in luminance, flash with decimetre height changes between frames. Near
    /// the camera the product is centimetres and the ramp stays at the look's `shore_blend_m`; it is capped at [`SHORE_RAMP_MAX_M`].
    pub fn shore_ramp_per_m(&self) -> f64 {
        SHORE_RAMP_K * self.tau_px * 2.0 * libm::tan(self.fov_y_rad / 2.0) / self.viewport_h_px
    }

    /// The steepest slope (rise over run) that the ground of a tile's neighbourhood can have and still leave the leaf a morph interval
    /// (LOD-008): the interval's start is `split(L) + 2 rho` with `rho = hypot(r, H)` for the node radius bound `r` and the half range `H`
    /// of a neighbour's height bounds, and it must stay within 95 % of the end `split(L - 1)`. Solving for `H`, the neighbour's extent `2 H`
    /// over its cap's diameter `2 r` gives `H / r`. Every term scales with the sample spacing, so one slope serves all levels; 0 for
    /// parameters that leave no room even on flat ground. The editor's brush guard reads it (VIEW-043).
    pub fn drawable_slope(&self) -> f64 {
        let level = self.max_level.max(1);
        let r = self.node_radius_bound(level);
        if !(r.is_finite() && r > 0.0) {
            return 0.0;
        }
        self.drawable_half_range(level) / r
    }

    /// The largest half range `H` of a neighbour's height bounds that leaves a leaf of `level` its morph interval (the equation of
    /// [`drawable_slope`](LodParams::drawable_slope) solved for `H`); 0 where there is no room even on flat ground.
    pub fn drawable_half_range(&self, level: u8) -> f64 {
        let level = level.max(1);
        let (split, end) = (self.split_distance(level), self.split_distance(level - 1));
        let rho_max = (0.95 * end - split) / 2.0;
        let r = self.node_radius_bound(level);
        if !(rho_max.is_finite() && r.is_finite() && r > 0.0) || rho_max <= r {
            return 0.0;
        }
        (rho_max * rho_max - r * r).sqrt()
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
    /// The node is a tile kept from splitting because the tiles under it are too steep to morph (ADR 0074): it is an ancestor of leaves of
    /// the uncapped selection.
    pub capped: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NodeSelection {
    /// Leaves in ascending tile-id order (deterministic).
    pub nodes: Vec<SelectedNode>,
    /// The ceiling of the levels that the steep tiles of the position-only selection set (ADR 0074), empty where nothing is too steep.
    pub ceiling: std::sync::Arc<Ceiling>,
}

impl NodeSelection {
    /// The chunks under the selected nodes at or finer than tile level `top` (LOD-013), each once, in the order of the nodes: the chunk of the
    /// grid (level 11) that holds the node. Sampling heights never asks for a chunk; this is the only request a plan makes.
    pub fn chunk_requests(&self, top: u8) -> Vec<TileId> {
        self.chunk_requests_with(|tile| {
            let (level, shift) = (tile.level(), tile.level().saturating_sub(planet_generators::chunk::CHUNK_GRID_LEVEL));
            if level < top.max(planet_generators::chunk::CHUNK_GRID_LEVEL) {
                return Vec::new();
            }
            vec![(tile.face().0, tile.x() >> shift, tile.y() >> shift)]
        })
    }

    /// [`NodeSelection::chunk_requests`] with the chunks each node reads decided by `reads` (a chunk field's `chunks_read_by`, which adds the
    /// neighbours a node near a border reads): the union in the order of the nodes, each chunk once.
    pub fn chunk_requests_with(&self, reads: impl Fn(TileId) -> Vec<(u8, u32, u32)>) -> Vec<TileId> {
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for node in &self.nodes {
            for key in reads(node.tile) {
                if seen.insert(key) {
                    out.push(
                        TileId::new(planet_core::Face(key.0), planet_generators::chunk::CHUNK_GRID_LEVEL, key.1, key.2)
                            .expect("a chunk of the grid is a tile"),
                    );
                }
            }
        }
        out
    }
}
/// The test a morph interval `[start, end]` must pass to leave a crack-free morph (LOD-008): finite, and starting within `0.95 - margin` of
/// its end. One predicate for [`LodParams::validate`], the selection's morphable test and the plan's backstop.
pub fn morph_room(start: f64, end: f64, margin: f64) -> bool {
    start.is_finite() && end.is_finite() && start <= (0.95 - margin) * end
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
        worst = worst.max(centre_dir.angle_to(d));
    }
    TileGeom { dir: centre_dir, centre, radius, angular_radius: worst * 1.001 }
}

/// What the water pass's cut needs of a tile (ATMO-020): the direction of its centre, the chord radius of its bounding sphere on the
/// reference sphere and the height bounds of the tile's column, exactly the values the node selection culls with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileExtent {
    pub dir: Vec3,
    /// Chord radius of the tile's bounding sphere on the sphere of radius `LodParams::radius_m` (m).
    pub chord_radius_m: f64,
    /// Angular radius (radians) of the cap that covers the tile, the argument of `HeightField::height_bounds`.
    pub angular_radius: f64,
}

/// The extent of `tile` (a pure function of the tile, the face mapping and the radius).
pub fn tile_extent(map: &dyn FaceMapping, tile: TileId, p: &LodParams) -> TileExtent {
    let g = tile_geom(map, tile, p);
    TileExtent { dir: g.dir, chord_radius_m: g.radius, angular_radius: g.angular_radius }
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
    /// Mesh key of each tile for the grid size and finest level `mesh_key_params` (the key chains the cell count; a change of
    /// either parameter starts it over).
    mesh_keys: IdMap<u64>,
    mesh_key_params: (u32, u8),
    /// The content keys of the tiles of a field that reads authored items (STRM-027).
    hashes: TileHashes,
    /// Whether the subtree of a tile holds no tile too steep to morph (`certified`), by tile; valid for `certs_stamp` only: the parameters it
    /// was judged with and the field's item revision (an edit starts it over, so nothing stale survives one).
    certs: IdMap<bool>,
    /// The bounds of the field over the cap of each tile that [`none_steep`] covered with (`cap_of`), valid for `certs_stamp` like `certs`.
    caps: IdMap<(f64, f64)>,
    certs_stamp: Option<(LodParams, u64, u64)>,
    /// The item changes the app announced between two item revisions of the field (`note_item_change`), oldest first: what lets the cache keep what
    /// the changes cannot have touched when the revision moves (STRM-034).
    transitions: Vec<(u64, u64, std::sync::Arc<Vec<ItemChange>>)>,
}

/// Where an authored item was or is, and how far it reaches: a change of the items between two revisions, as the footprint of what changed
/// (STRM-034). `dir` is a unit direction on the body.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemChange {
    pub dir: Vec3,
    pub reach_m: f64,
}

impl ItemChange {
    /// The change as the one reach test of the footprint module (R3), without strokes: a plan's reads take the whole disc.
    fn edit_reach(&self) -> planet_world_def::footprint::EditReach<'_> {
        planet_world_def::footprint::EditReach { dir: self.dir, reach_m: self.reach_m, strokes: None }
    }
}

/// The most announced transitions kept.
pub(crate) const MAX_TRANSITIONS: usize = 32;

/// Whether a change can alter the bounds over the neighbourhood of the tile `geom` (the pads of the steepness proof and the caps of `none_steep`):
/// the angle from the tile's centre to the change is within eight radii of the tile and the change's reach (an item changes bounds over a cap only
/// when its reach touches it, so no more is needed).
fn change_reaches_bounds(geom: &TileGeom, change: &ItemChange, radius_m: f64) -> bool {
    change.edit_reach().within_angle(geom.dir, 8.0 * geom.angular_radius, radius_m)
}

/// The angle from the centre of `tile` within which an item can alter what the tile reads (`inputs_hash_for`), reach not included; infinite when the
/// tile reads the whole body. A tile below the cell level by `SUMMARY_LEVEL_GAP` or more records the footprint of the tile and its halo (the
/// items that touch the cap of the tile with the largest halo); a coarser one records the summary nodes that cover the tile and a margin of rings
/// of cells (`summary::cover`), or the six faces when the margin needs more rings than the index allows. Every cell of a cover alters the hash,
/// so the reach is the tile's own extent, the rings of cells at their widest (one and a half nominal edges, with two cells to spare), and no more.
fn tile_reads_reach(tile: TileId, geom: &TileGeom, radius_m: f64) -> f64 {
    let cell_level = planet_world_def::cells::cell_level(radius_m);
    if planet_world_def::summary::is_coarse(tile, cell_level) {
        match planet_world_def::summary::margin_rings(tile, 1.0, radius_m, cell_level) {
            None => f64::INFINITY,
            Some(rings) => {
                let nominal_edge_rad = core::f64::consts::FRAC_PI_2 / f64::from(1u32 << cell_level);
                geom.angular_radius + (f64::from(rings) + 2.0) * 1.5 * nominal_edge_rad
            }
        }
    } else {
        planet_world_def::footprint::tile_cap(&planet_core::TangentWarp, tile, 1.0).angle
    }
}

/// Whether a change can alter what the tile reads, given the angle `reach` from [`tile_reads_reach`].
fn change_reaches_reads(geom: &TileGeom, change: &ItemChange, reach: f64, radius_m: f64) -> bool {
    change.edit_reach().within_angle(geom.dir, reach, radius_m)
}

/// The angle from the centre of `tile` within which an item (its own reach not counted) can alter what the tile reads; infinite when it reads the
/// whole body. A hook for the tests.
pub fn tile_reads_reach_rad(map: &dyn FaceMapping, p: &LodParams, tile: TileId) -> f64 {
    tile_reads_reach(tile, &tile_geom(map, tile, p), p.radius_m)
}

/// Whether any announced change can alter the reads of `tile` (the test of [`SelectionCache::note_item_change`] for the tile keys): a hook for the
/// tests that check it against the real keys.
pub fn changes_may_alter_tile(map: &dyn FaceMapping, p: &LodParams, tile: TileId, changes: &[ItemChange]) -> bool {
    if changes.is_empty() {
        return false;
    }
    let geom = tile_geom(map, tile, p);
    let reach = tile_reads_reach(tile, &geom, p.radius_m);
    changes.iter().any(|c| change_reaches_reads(&geom, c, reach, p.radius_m))
}

/// The levels whose morph intervals read height bounds for a field of largest height `h` (`reads_bounds`), as a bit mask: what the certificates
/// depend on of `h`.
fn bounds_mask(p: &LodParams, h: f64) -> u64 {
    (0..=p.max_level.min(63)).fold(0u64, |m, level| m | (u64::from(reads_bounds(p, level, h)) << level))
}

/// The content key of each tile the cache holds values of, for a field whose tiles read authored items: the field's hash of the tile's
/// reads ([`HeightField::inputs_hash_for`]), checked again only when the field's item revision changes (it is process-unique, so a
/// reload into a fresh definition cannot match). A tile whose hash differs has its origin height, mesh key and volume dropped, whoever
/// changed the items and whether or not the editor named the tile (`invalidate_tiles` is the fast path, not the guarantee).
#[derive(Default)]
struct TileHashes {
    /// The field's item revision (0: it reads no items and nothing is tracked).
    revision: u64,
    /// Hash and revision it was taken at, by tile.
    seen: IdMap<(u64, u64)>,
    /// Tiles whose hash changed since the last drain.
    stale: Vec<TileId>,
    /// Footprint hashes computed since the cache was created (the cost of the check).
    computed: u64,
}

impl TileHashes {
    /// Whether the values cached of `tile` are stale. Free while the revision is the one the tile was last checked at.
    fn verify(&mut self, tile: TileId, map: &dyn FaceMapping, field: &dyn HeightField) -> bool {
        if self.revision == 0 {
            return false;
        }
        if let Some((_, at)) = self.seen.get(&tile) {
            if *at == self.revision {
                return false;
            }
        }
        let hash = field.inputs_hash_for(tile, map);
        self.computed += 1;
        let changed = self.seen.insert(tile, (hash, self.revision)).is_some_and(|(old, _)| old != hash);
        if changed {
            self.stale.push(tile);
        }
        changed
    }
}

impl SelectionCache {
    /// Upper bound on cached tiles (some tens of MB) before the cache starts over.
    pub const MAX_TILES: usize = 400_000;

    pub fn new() -> SelectionCache {
        SelectionCache::default()
    }

    /// Forget what is cached of `tiles`: an item edit changed what they read (STRM-026, E1), so their origin height, mesh key and
    /// height-aware volume are computed again at the next plan. The geometry memo (`tiles`) is a function of the tile alone and stays.
    /// Returns how many origin heights were dropped.
    pub fn invalidate_tiles(&mut self, tiles: &[TileId]) -> usize {
        let mut dropped = 0;
        for t in tiles {
            dropped += usize::from(self.origin_heights.remove(t).is_some());
            self.mesh_keys.remove(t);
            self.volumes.remove(t);
        }
        // The certificates and cap bounds near an edit are dropped at the next plan (`keep_across`) when the change was announced, and all of them when
        // it was not; only an edit nobody announced clears them here.
        if self.transitions.is_empty() {
            self.certs.clear();
            self.caps.clear();
        }
        dropped
    }

    /// Forget what is cached of the tiles a chunk that has just landed can change (STRM-040): those whose reads, decided by `reads` (a chunk
    /// field's `chunks_read_by`), include it. Returns how many origin heights were dropped.
    pub fn invalidate_chunk(&mut self, landed: (u8, u32, u32), reads: impl Fn(TileId) -> Vec<(u8, u32, u32)>) -> usize {
        self.invalidate_chunks(&[landed], reads)
    }

    /// [`SelectionCache::invalidate_chunk`] for several landed chunks in one pass over the cache (the pass costs `reads` for every tile it holds).
    pub fn invalidate_chunks(&mut self, landed: &[(u8, u32, u32)], reads: impl Fn(TileId) -> Vec<(u8, u32, u32)>) -> usize {
        let mut tiles: Vec<TileId> = self.origin_heights.keys().chain(self.mesh_keys.keys()).chain(self.volumes.keys()).copied().collect();
        tiles.sort_unstable_by_key(|t| t.raw());
        tiles.dedup();
        tiles.retain(|&t| reads(t).iter().any(|k| landed.contains(k)));
        self.invalidate_tiles(&tiles)
    }

    /// Whether the cache holds an origin height for `tile`; for tests of what a landing drops.
    #[doc(hidden)]
    pub fn has_origin_for_test(&self, tile: TileId) -> bool {
        self.origin_heights.contains_key(&tile)
    }

    pub fn clear(&mut self) {
        self.tiles.clear();
        self.volumes.clear();
        self.origin_heights.clear();
        self.mesh_keys.clear();
        self.hashes.seen.clear();
        self.hashes.stale.clear();
        self.certs.clear();
        self.caps.clear();
        self.certs_stamp = None;
        self.transitions.clear();
    }

    /// Announce a change of the authored items: the field's item revision went from `from` to `to`, and `changes` is where the items that
    /// differ were and are (`WorldDefinition::item_changes_from`; every item that differs, none left out). The next plan then keeps the keys,
    /// volumes, certificates and cap bounds of the tiles those changes cannot reach, and checks the others (STRM-034). A revision that is
    /// not announced, or whose chain of announcements is broken, starts the checks from all tiles, as before.
    pub fn note_item_change(&mut self, from: u64, to: u64, changes: Vec<ItemChange>) {
        if from == to || from == 0 || to == 0 {
            return;
        }
        self.transitions.push((from, to, std::sync::Arc::new(changes)));
        if self.transitions.len() > MAX_TRANSITIONS {
            self.transitions.remove(0);
        }
    }

    /// The announced changes that take the revision `from` to `to`, by following the transitions; `None` when the chain is broken.
    fn changes_between(&self, from: u64, to: u64) -> Option<Vec<ItemChange>> {
        let mut out = Vec::new();
        let mut at = from;
        for _ in 0..=MAX_TRANSITIONS {
            if at == to {
                return Some(out);
            }
            let (_, next, changes) = self.transitions.iter().find(|(f, ..)| *f == at)?;
            out.extend(changes.iter().copied());
            at = *next;
        }
        None
    }

    /// The revision moved from `previous` to `revision` by the announced `changes`: the tiles they cannot reach are stamped as checked at the
    /// new revision, and the certificates and cap bounds near them are dropped, the others kept for it.
    fn keep_across(&mut self, changes: &[ItemChange], previous: u64, revision: u64, p: &LodParams) {
        let radius = p.radius_m;
        let tiles = &self.tiles;
        let mut reached: Vec<TileId> = Vec::new();
        for (tile, seen) in self.hashes.seen.iter_mut() {
            if seen.1 != previous {
                continue;
            }
            let Some(g) = tiles.get(tile) else { continue };
            let reach = tile_reads_reach(*tile, g, radius);
            seen.1 = revision;
            if changes.iter().any(|c| change_reaches_reads(g, c, reach, radius)) {
                // A tile a change can reach is taken as changed, without the hash of its reads (the cover of a coarse tile is some 20,000 nodes):
                // its volume, origin height and mesh key are dropped, and its hash is not known: the hash of 0 differs from any, so the next check at a
                // later revision takes the tile as changed once more (a recompute, never a stale value).
                seen.0 = 0;
                reached.push(*tile);
            }
        }
        for tile in reached {
            self.volumes.remove(&tile);
            self.hashes.stale.push(tile);
        }
        if matches!(&self.certs_stamp, Some((q, r, _)) if q == p && *r == previous) {
            let keep = |tile: &TileId| tiles.get(tile).is_some_and(|g| !changes.iter().any(|c| change_reaches_bounds(g, c, radius)));
            self.certs.retain(|tile, _| keep(tile));
            self.caps.retain(|tile, _| keep(tile));
            if let Some(stamp) = self.certs_stamp.as_mut() {
                stamp.1 = revision;
            }
        }
        // What was announced is used up (and what led to a revision no plan will ask for again goes with it).
        self.transitions.retain(|(from, ..)| *from != previous);
    }

    /// Whether the cache tracks the items the prepared field reads (it has a revision to check): the origin heights and mesh keys of
    /// its tiles can be reused after [`SelectionCache::verify_tiles`].
    pub(crate) fn tracks_items(&self) -> bool {
        self.hashes.revision != 0
    }

    /// Footprint hashes computed since the cache was created (STRM-027).
    pub fn hashes_computed(&self) -> u64 {
        self.hashes.computed
    }

    /// Drop what is cached of the tiles whose content key changed (the check of STRM-027) among `tiles` and those the selection found
    /// changed. Valid after a selection with the same map and field has prepared the cache.
    pub(crate) fn verify_tiles(&mut self, tiles: impl IntoIterator<Item = TileId>, map: &dyn FaceMapping, field: &dyn HeightField) {
        for t in tiles {
            if self.hashes.verify(t, map, field) {
                self.volumes.remove(&t);
            }
        }
        // The tiles a selection found changed already had their volume recomputed there: only the values it does not hold are dropped.
        let stale = std::mem::take(&mut self.hashes.stale);
        for t in stale {
            self.origin_heights.remove(&t);
            self.mesh_keys.remove(&t);
        }
    }

    /// Store an origin height as the plan builder does; for tests of what a landing drops.
    #[doc(hidden)]
    pub fn origin_height_for_test(&mut self, tile: TileId, height_m: f64) {
        self.origin_heights.insert(tile, height_m);
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
                // The reused values start over when the render floor changes (LOD-009, LOD-011).
                planet_core::hash::hash_u64s(6, &[u64::from(p.min_level)]),
            ],
        );
        let (previous, revision) = (self.hashes.revision, field.item_revision());
        self.hashes.revision = revision;
        if key != self.key
            || self.tiles.len() > Self::MAX_TILES
            || self.origin_heights.len() > Self::MAX_TILES
            || self.mesh_keys.len() > Self::MAX_TILES
        {
            self.clear();
            self.key = key;
        } else if previous != 0 && revision != 0 && previous != revision {
            if let Some(changes) = self.changes_between(previous, revision) {
                self.keep_across(&changes, previous, revision, p);
            }
        }
        // The certificates depend on the field's largest height only through the levels that read bounds (`reads_bounds`): they are kept while
        // those are the same; the cap bounds depend on the parameters and the items alone.
        let mask = bounds_mask(p, field.max_abs_height_m());
        match &self.certs_stamp {
            Some((q, r, m)) if q == p && *r == revision => {
                if *m != mask {
                    self.certs.clear();
                }
            }
            _ => {
                self.certs.clear();
                self.caps.clear();
            }
        }
        self.certs_stamp = Some((p.clone(), revision, mask));
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
    hashes: &'a mut TileHashes,
    certs: &'a mut IdMap<bool>,
    caps: &'a mut IdMap<(f64, f64)>,
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
        if self.hashes.verify(tile, self.map, self.field) {
            self.volumes.remove(&tile);
        }
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
    fn bounds_monotone(&self) -> bool {
        true
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

/// The roots of the draw set (LOD-011): every tile of the effective render floor level, `6 * 4^floor` of them (the six face tiles at
/// floor 0, 384 at floor 3), in ascending tile-id order.
pub fn floor_tiles(p: &LodParams) -> Vec<TileId> {
    let level = p.floor_level();
    let side = 1u32 << level;
    let mut out = Vec::with_capacity(6 * (side as usize) * (side as usize));
    for f in Face::ALL {
        for x in 0..side {
            for y in 0..side {
                out.push(TileId::new(f, level, x, y).expect("a tile of the floor level"));
            }
        }
    }
    out.sort_unstable();
    out
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
    let SelectionCache { tiles, volumes, misses, hashes, certs, caps, .. } = cache;
    let mut geoms = GeomCache { map, field, p, tiles, volumes, misses, hashes, certs, caps };
    let floor = p.floor_level();
    // ADR 0074: tiles whose neighbourhood is too steep to morph are not leaves; the ceiling they set is found on the position-only selection
    // (no frustum), so the residency set and the plan of any view agree on it and the draw set stays inside R.
    let mut ceiling = Ceiling::default();
    let mut uncapped: Option<BTreeSet<TileId>> = None;
    // Where the field's bounds nest, a position-only walk that meets only certified subtrees (`certified`) proves that no tile is too steep
    // without balancing or judging the tiles of the selection: the ceiling is empty and the walk below is the only one.
    if steep_possible(field, p) && !(field.bounds_monotone() && none_steep(&mut geoms, camera)) {
        let position_leaves = walk_leaves(&mut geoms, camera, None, &Ceiling::default()).0;
        ceiling = Ceiling::of_unmorphable(&mut geoms, &position_leaves);
        if ceiling.is_empty() && frustum.is_none() {
            uncapped = Some(position_leaves);
        }
    }
    let (mut leaves, mut capped) = match uncapped {
        Some(leaves) => (leaves, BTreeSet::new()),
        None => walk_leaves(&mut geoms, camera, frustum.as_ref(), &ceiling),
    };
    if !ceiling.is_empty() {
        // The walk keeps the tiles whose children the ceiling forbids and the balance splits what a finer neighbour asks; whatever is still
        // finer than the ceiling is merged (a safety: the ceiling is 1-Lipschitz, so this changes nothing in the usual case).
        let (clamped, more) = ceiling.clamp(&leaves);
        leaves = clamped;
        capped.extend(more);
        capped.retain(|t| leaves.contains(t));
    }
    let mut nodes: Vec<SelectedNode> = leaves
        .iter()
        .map(|&tile| {
            let distance_m = geoms.distance(tile, camera);
            // A leaf at the render floor is a root: its parent is not drawn, so it never morphs (LOD-004, LOD-011).
            let (morph, morph_start_m, morph_end_m) = match tile.parent() {
                Some(parent) if tile.level() > floor => {
                    let d = geoms.distance(parent, camera) / p.split_distance(parent.level());
                    let morph = ((d - (1.0 - p.morph_band)) / p.morph_band).clamp(0.0, 1.0);
                    let (start, end) = morph_interval_for(&mut geoms, tile);
                    (morph, start, end)
                }
                _ => (0.0, 1.0e30, 1.0e30),
            };
            SelectedNode { tile, morph: morph as f32, distance_m, morph_start_m, morph_end_m, capped: capped.contains(&tile) }
        })
        .collect();
    // Two leaves of one level share border vertices, which both morph by the shader formula: their intervals must be
    // equal or the border opens. One start per level, the largest any leaf of the level needs. Where the terrain keeps
    // every leaf at the base start (the reference field at production parameters) this is the level constant of old.
    let mut level_start = [0.0f64; 32];
    for n in nodes.iter().filter(|n| n.tile.level() > floor) {
        let s = &mut level_start[n.tile.level() as usize];
        *s = s.max(n.morph_start_m);
    }
    for n in nodes.iter_mut().filter(|n| n.tile.level() > floor) {
        n.morph_start_m = level_start[n.tile.level() as usize];
    }
    NodeSelection { nodes, ceiling: std::sync::Arc::new(ceiling) }
}

/// The leaves of the selection for `camera`: the walk by distance, culled by the horizon and, when given, the frustum, then balanced.
///
/// A tile that the distance says to split but whose children the `ceiling` forbids (ADR 0074) stays a leaf and is returned among the capped
/// tiles with the leaves.
fn walk_leaves(
    geoms: &mut GeomCache<'_>,
    camera: PlanetFixed,
    frustum: Option<&Frustum>,
    ceiling: &Ceiling,
) -> (BTreeSet<TileId>, BTreeSet<TileId>) {
    let (map, p) = (geoms.map, geoms.p);
    let mut leaves: BTreeSet<TileId> = BTreeSet::new();
    let mut capped: BTreeSet<TileId> = BTreeSet::new();
    let floor = p.floor_level();
    let mut stack: Vec<TileId> = Face::ALL.iter().map(|&f| TileId::new(f, 0, 0, 0).expect("root")).collect();
    while let Some(t) = stack.pop() {
        let g = geoms.get(t);
        if p.cull_horizon && below_horizon_at(g.centre, g.angular_radius, camera, p) {
            continue;
        }
        if let Some(fr) = frustum {
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
        // A tile below the render floor is split whatever the distance (LOD-011); the floor level is never above the finest level.
        // The bound that needs no height bounds rules most tiles out before the bounds are computed.
        let wanted = t.level() < floor
            || (t.level() < p.max_level && geoms.distance_lower_bound(t, camera) < split && geoms.distance(t, camera) < split);
        if wanted && t.level() >= floor && ceiling.forbids_split(t) {
            leaves.insert(t);
            capped.insert(t);
        } else if wanted {
            stack.extend(children_of(t));
        } else {
            leaves.insert(t);
        }
    }
    balance_leaves(map, &mut leaves);
    capped.retain(|t| leaves.contains(t));
    (leaves, capped)
}

/// Whether any tile can be too steep to morph, or have bounds that are not numbers: only where the interval of a leaf of the finest level reads
/// the bounds of its neighbours ([`reads_bounds`]); where it does not (a flat field) no tile has an interval that depends on them.
fn steep_possible(field: &dyn HeightField, p: &LodParams) -> bool {
    reads_bounds(p, p.max_level, field.max_abs_height_m())
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
    /// The ceiling R was selected with (ADR 0074): the cover applies it as the plan does, so that with every tile of R resident the two are
    /// the same nodes.
    ceiling: std::sync::Arc<Ceiling>,
}

impl TileBounds {
    /// These bounds with the ceiling R was selected with (ADR 0074).
    pub fn with_ceiling(mut self, ceiling: std::sync::Arc<Ceiling>) -> TileBounds {
        self.ceiling = ceiling;
        self
    }

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
        let SelectionCache { tiles: geom_memo, volumes, misses, hashes, certs, caps, .. } = self;
        let mut geoms = GeomCache { map, field, p, tiles: geom_memo, volumes, misses, hashes, certs, caps };
        let mut out = TileBounds { for_view: Some((position, p.clone())), tiles: IdMap::default(), ceiling: Default::default() };
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
/// distance) and a child that is in view is resident (see [`cover_selection_masked`]: the tile then stands in over the quadrants of
/// the children that are not), otherwise it is drawn itself. In view means what the plan means: not beyond the horizon, not
/// outside the frustum (the padded sphere), not outside the tight cut (the tile's height-bounds volume, LOD-007/008). A child that
/// the horizon, the frustum or the tight cut culls never stops its parent's descent. With every tile of the residency set resident
/// the cover equals the plan of the camera, node for node (the bounds hold the tiles the plan visits). Always a subset of the
/// resident tiles, in ascending tile-id order; a tile may lie under another only beside the quadrants the other draws. Tiles that
/// `bounds` does not hold are judged by their geometry alone (a looser cull, the origin-based distance).
pub fn cover_selection_bounded(
    map: &dyn FaceMapping,
    camera: &Camera,
    p: &LodParams,
    resident: &dyn Fn(TileId) -> Option<PlanetFixed>,
    bounds: &TileBounds,
) -> Vec<TileId> {
    cover_selection_masked(map, camera, p, resident, bounds).into_iter().map(|(t, _)| t).collect()
}

/// The bit of child `child_index` (the index in [`TileId::children`]) in a quadrant mask; `ALL_QUADRANTS` is all four.
pub const fn quadrant_bit(child_index: usize) -> u8 {
    1 << child_index
}

/// The mask of a tile drawn whole.
pub const ALL_QUADRANTS: u8 = 0b1111;

/// The index in [`TileId::children`] of the child of `ancestor` that is `tile` or holds it.
pub fn child_index_toward(ancestor: TileId, tile: TileId) -> usize {
    let mut c = tile;
    while c.parent() != Some(ancestor) {
        c = c.parent().expect("`ancestor` is an ancestor of `tile`");
    }
    ancestor.children().expect("an ancestor has children").iter().position(|x| *x == c).expect("a child of its parent")
}

/// The cover of a view (STRM-016) as the tiles to draw, each with the quadrants of it to draw ([`ALL_QUADRANTS`] for a tile drawn whole).
/// As [`cover_selection_bounded`] said before a missing tile no longer took its resident siblings back to their parent: a tile that the
/// plan's LOD test splits is split when at least one of its children in view is resident; the resident children go on down the walk and
/// the parent stands in for the in-view children that are not resident, drawn only over their quadrants (its other quadrants are the
/// children's). A tile with no resident child in view is drawn whole, as before. The leaves, the missing children among them, are
/// balanced as the plan balances them; a leaf that is not resident is drawn by its nearest resident ancestor over the quadrant that holds
/// it, and a resident tile under a quadrant an ancestor covers is not drawn (no two drawn parts overlap). With every tile of the
/// residency set resident every mask is whole and the result is the plan, node for node. Sorted by tile id.
pub fn cover_selection_masked(
    map: &dyn FaceMapping,
    camera: &Camera,
    p: &LodParams,
    resident: &dyn Fn(TileId) -> Option<PlanetFixed>,
    bounds: &TileBounds,
) -> Vec<(TileId, u8)> {
    cover_selection_exact(map, camera, p, resident, bounds).into_iter().map(|(t, m, _)| (t, m)).collect()
}

/// [`cover_selection_masked`] with, for each drawn tile, whether it is exact: a tile the plan of the camera would draw itself (resident, drawn
/// whole, not standing in for a child of the plan that is missing). A tile that is not exact is a stand-in: the plan wants finer tiles under
/// it, and the quadrants it draws belong to children that are not resident. With every tile of the residency set resident every tile is exact.
pub fn cover_selection_exact(
    map: &dyn FaceMapping,
    camera: &Camera,
    p: &LodParams,
    resident: &dyn Fn(TileId) -> Option<PlanetFixed>,
    bounds: &TileBounds,
) -> Vec<(TileId, u8, bool)> {
    let mut view = ViewTest::new(map, camera, p, bounds);
    // Tiles the walk drew whole although the plan splits them (no child in view is resident).
    let mut standing: BTreeSet<TileId> = BTreeSet::new();
    let mut leaves: BTreeSet<TileId> = BTreeSet::new();
    let mut stack: Vec<TileId> = Vec::new();
    for root in floor_tiles(p) {
        if resident(root).is_some() && !view.culled(root) {
            stack.push(root);
        }
    }
    while let Some(t) = stack.pop() {
        let origin = resident(t).expect("the walk visits resident tiles only");
        // A tile whose children the ceiling forbids is a leaf (ADR 0074): the residency set does not hold them, and the walk would find nothing to draw.
        let forbidden = bounds.ceiling.forbids_split(t);
        let split = t.level() < p.max_level && !forbidden && view.distance(t, origin) < p.split_distance(t.level());
        if split {
            let seen: Vec<TileId> = children_of(t).into_iter().filter(|&c| !view.culled(c)).collect();
            let (here, missing): (Vec<TileId>, Vec<TileId>) = seen.iter().partition(|&&c| resident(c).is_some());
            if missing.is_empty() {
                // With every child out of view nothing of the tile can be seen, and the plan draws nothing there either.
                stack.extend(seen);
                continue;
            }
            if !here.is_empty() {
                // Some of the children are here: they go on, and the ones that are not stand for themselves as leaves the parent covers.
                stack.extend(here);
                leaves.extend(missing);
                continue;
            }
            standing.insert(t);
        }
        leaves.insert(t);
    }
    balance_leaves(map, &mut leaves);
    // The plan's ceiling (ADR 0074): the cover keeps what the plan keeps, so that with every tile of R resident the two are the same nodes.
    if !bounds.ceiling.is_empty() {
        leaves = bounds.ceiling.clamp(&leaves).0;
    }
    // A leaf that is not resident (a missing child, or a balancing split) is drawn by the nearest resident ancestor over the quadrant
    // that holds it (that ancestor is a leaf of the walk or an ancestor of one); a leaf that is resident is drawn whole.
    let mut drawn: BTreeMap<TileId, u8> = BTreeMap::new();
    // The tiles that stand in for a tile that is not resident, or for finer ones the walk could not draw.
    let mut standing_in: BTreeSet<TileId> = standing.clone();
    for leaf in leaves {
        if resident(leaf).is_some() {
            drawn.insert(leaf, ALL_QUADRANTS);
            // A tile the balancing made by splitting a tile that stands in is part of the stand-in (it may lie outside the view the plan draws).
            if std::iter::successors(leaf.parent(), |a| a.parent()).any(|a| standing.contains(&a)) {
                standing_in.insert(leaf);
            }
            continue;
        }
        let mut x = leaf;
        while resident(x).is_none() {
            x = x.parent().expect("a root of the walk is resident");
        }
        // The ancestor is a standing-in tile that is not itself a leaf: a leaf it was split from by the balancing is a stand-in over all
        // four quadrants through its four children.
        *drawn.entry(x).or_insert(0) |= quadrant_bit(child_index_toward(x, leaf));
        standing_in.insert(x);
    }
    // A tile under a quadrant that an ancestor draws is covered by it.
    let covered = |t: TileId| {
        let mut up = t.parent();
        while let Some(a) = up {
            if let Some(mask) = drawn.get(&a) {
                if mask & quadrant_bit(child_index_toward(a, t)) != 0 {
                    return true;
                }
            }
            up = a.parent();
        }
        false
    };
    drawn.iter().filter(|(t, _)| !covered(**t)).map(|(t, m)| (*t, *m, *m == ALL_QUADRANTS && !standing_in.contains(t))).collect()
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

/// Whether the morph interval of a leaf of `level` reads the bounds of its neighbours: not where even the largest possible volume radius (the
/// field's largest height `h` on both sides) leaves the base start in force. The one test of [`morph_interval_for`], of the tiles the
/// selection looks at for steep ground and of [`steep_possible`], so that every tile whose interval depends on a bound is looked at (a bound
/// that is not a number is then no room, not a skipped tile).
fn reads_bounds(p: &LodParams, level: u8, h: f64) -> bool {
    let end = p.split_distance(level.saturating_sub(1));
    let base = (1.0 - p.morph_band) * end;
    let largest = p.split_distance(level) + 2.0 * (p.node_radius_bound(level) * (1.0 + h / p.radius_m) + h);
    // Not a number is not room: such a field reads its bounds.
    !matches!(largest.partial_cmp(&base), Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal))
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
    if !reads_bounds(p, level, h) {
        return (base, end);
    }
    let mut rho = 0.0f64;
    for side in Side::ALL {
        let v = geoms.volume(tile.neighbor(geoms.map, side));
        let r = geoms.radius_bound(level, &v);
        // `f64::max` drops a NaN: a bound that is not a number leaves no room instead of being ignored.
        if !r.is_finite() {
            return (f64::INFINITY, end);
        }
        rho = rho.max(r);
    }
    (base.max(split + 2.0 * rho), end)
}

/// Whether no tile of the subtree of `a` (`a` included, at any level from `a`'s down to the finest, and whatever balancing adds) is too steep to
/// morph, from one bounds query on a cap that covers the subtree and the same-level neighbours its morph intervals read. Only for a field whose
/// bounds nest ([`HeightField::bounds_monotone`], LOD-012); `false` is always safe (the caller then judges the tiles).
///
/// The cap is centred on `a` with the angle to the farthest of `a`'s side neighbours `s` plus three of their angular radii (and three of
/// `a`'s own): a tile `n` of a level at or below `a`'s that a descendant of `a` reads as a neighbour lies in `a` or in a side neighbour of `a`;
/// its centre is within the angular radius of that tile's centre, and its own cap (the argument of its volume, radius at most twice that of
/// the tile it lies in, since a tile's angular radius is at most its diameter, which is at most twice the angular radius of a tile that holds
/// it) reaches no further. So every volume `morph_interval_for` reads in the subtree comes from bounds inside those of the cap (up to the
/// slack of the contract), whose range is therefore no smaller than any of theirs and whose upper bound is no lower: their radius bound
/// (`radius_bound`, increasing in the half range and in the upper bound) is at most the one computed from the cap's, and so is the start of
/// the interval, at every level; the check is then the interval's own test at each level that reads bounds.
fn certified(geoms: &mut GeomCache<'_>, a: TileId) -> bool {
    if let Some(&c) = geoms.certs.get(&a) {
        return c;
    }
    let c = certify(geoms, a);
    geoms.certs.insert(a, c);
    c
}

fn certify(geoms: &mut GeomCache<'_>, a: TileId) -> bool {
    let (map, p, field) = (geoms.map, geoms.p, geoms.field);
    let h = field.max_abs_height_m();
    let ga = geoms.get(a);
    let mut pad = 3.0 * ga.angular_radius;
    for side in Side::ALL {
        let gs = geoms.get(a.neighbor(map, side));
        let between = ga.dir.angle_to(gs.dir);
        pad = pad.max(between + 3.0 * gs.angular_radius);
    }
    let pad = pad * 1.05 + 1e-9;
    let (lo, hi) = field.height_bounds(ga.dir, pad);
    if !(lo.is_finite() && hi.is_finite() && lo <= hi && pad.is_finite()) {
        return false;
    }
    let (lo, hi) = (lo - planet_generators::height::MONOTONE_SLACK_M, hi + planet_generators::height::MONOTONE_SLACK_M);
    let half = 0.5 * (hi - lo);
    for level in a.level().max(1)..=p.max_level {
        if !reads_bounds(p, level, h) {
            continue;
        }
        let end = p.split_distance(level - 1);
        let rho = extent(p.node_radius_bound(level) * (1.0 + hi.max(0.0) / p.radius_m), half);
        let start = ((1.0 - p.morph_band) * end).max(p.split_distance(level) + 2.0 * rho);
        if !morph_room(start, end, 0.0) {
            return false;
        }
    }
    true
}

/// Whether no tile of the position-only selection (the balanced walk of `walk_leaves` without a frustum or a ceiling) can be too steep to
/// morph, for a field whose bounds nest, from the cached bounds of the tiles round the camera and no walk or balancing. `false` means "not
/// proved" (the caller then takes the full path, which is exact).
///
/// A tile `x` of level `k` above the floor is in the selection only if its parent was split by distance (`geoms.distance(parent) <
/// split(k - 1)`), or balancing made it: a leaf `y` at least a level finer than `x` lies within a chain of tiles of shrinking sizes next to
/// it (the chain runs through balance-split tiles of shrinking sizes, at most `7 r` from `x`'s centre to the centre of the tile distance split;
/// `r` the largest angular radius of a tile of level `k`), and that tile (level `k` or finer) was split by distance, `< split(k)`, which is half
/// of `split(k - 1)`; a level that passes `morph_room` has `split(k)` of at least `2.2 nrb(k)`, which is the slack the `7 r` needs. Either way the volume sphere of a tile that was split by distance has a point
/// within `split(k - 1)` of the camera, and the sphere's radius is at most `extent(node_radius_bound (1 + h/R), h)`, so its centre, on the
/// direction of the tile at a norm of at least `R - h`, is within `D = split(k - 1) + that radius` of the camera (the half range and the upper
/// bound of the sphere being those of the covering caps of level `k - 1`, whose tiles' volumes they hold, or `h` where that level reads none), and so the direction of
/// `x` is within `2 asin(D / (2 m))` of the camera's (`m = min(|camera|, R - h)`; the chord is at least `m` times the angle's chord) plus
/// the `7 r` of the chain. The tiles `morph_interval_for` reads for such a tile are `x`'s four side neighbours, whose centres are within
/// `2.2 r` of its centre and whose caps (the argument of their volumes) have radius at most `1.01 r`. Each such cap has its centre in a tile
/// `a` of a level at most `k - 1` (`a`'s centre within `r_a` of it), so it lies inside the cap of `a` of `cap_of` (`r_a` plus the radius
/// bound of the level below `a`'s, which is at least `1.01 r`); the bounds of that cap, cached by tile, hold the bounds of every volume read,
/// up to the slack of the contract.
/// The interval of the level is tested on them exactly as `morph_interval_for` would (`radius_bound` and the start are increasing in the half
/// range and in the upper bound). The tiles `a` are those within `D' + r_a` of the camera, `D'` the angle above plus `7.5 r` (found by a
/// flood from the camera's tile, which keeps going `3.5 r_a` beyond so that it cannot miss a tile), first at level `k - 3`; a tile whose cap
/// has bounds too wide for the interval is replaced by its children, down to level `k - 1`, where it fails the test. A level where no tile can be within
/// `D` of the camera (the altitude above the highest ground is more than `D`) has none, and nor has any finer level.
fn none_steep(geoms: &mut GeomCache<'_>, camera: PlanetFixed) -> bool {
    /// Tiles one call may look at before it gives up (the cover of a level is a few dozen tiles where it works).
    const MAX_VISITS: u32 = 4_000;
    let (map, p, field) = (geoms.map, geoms.p, geoms.field);
    let h = field.max_abs_height_m();
    let floor = p.floor_level();
    let (norm, dir) = (camera.0.length(), camera.0.normalized());
    let altitude = norm - (p.radius_m + h);
    let m = norm.min(p.radius_m - h);
    if !(norm.is_finite() && m > 0.0) {
        return false;
    }
    let angle_to = |v: Vec3| dir.angle_to(v);
    // The half range and the upper bound the previous level's covering caps allowed (the volumes of its tiles, the parents of this level's,
    // lie inside them), or the field's global bound where that level read no bounds.
    let mut previous: Option<(f64, f64)> = None;
    let mut visits = 0u32;
    for k in (floor + 1).max(1)..=p.max_level {
        if !reads_bounds(p, k, h) {
            previous = None;
            continue;
        }
        let end = p.split_distance(k - 1);
        let (parent_half, parent_hi) = previous.unwrap_or((h, h));
        let parent_radius = extent(p.node_radius_bound(k - 1) * (1.0 + parent_hi.max(0.0) / p.radius_m), parent_half);
        let (mut level_half, mut level_hi) = (0.0f64, f64::NEG_INFINITY);
        let d = end + parent_radius;
        // No tile of this level is within reach of the camera, nor is any of the finer levels (their reach is shorter).
        if altitude > d {
            break;
        }
        let r = 1.02 * p.node_radius_bound(k) / p.radius_m;
        let theta = if d >= 2.0 * m { std::f64::consts::PI } else { 2.0 * libm::asin(d / (2.0 * m)) };
        let reach = (theta + 7.5 * r) * 1.0001 + 1e-9;
        if reach >= 0.5 {
            return false;
        }
        // The tiles round the camera at level `k - 3`, each halved where the bounds of its cap are too wide, down to level `k - 1`.
        let j = k.saturating_sub(3);
        let start = TileId::from_direction(map, dir, j);
        let mut seen: IdSet = IdSet::default();
        seen.insert(start);
        let mut flood = vec![start];
        let mut covering: Vec<TileId> = Vec::new();
        while let Some(a) = flood.pop() {
            visits += 1;
            if visits > MAX_VISITS {
                return false;
            }
            let ga = geoms.get(a);
            let between = angle_to(ga.dir);
            if between <= reach + ga.angular_radius {
                covering.push(a);
            }
            if between <= reach + 3.5 * ga.angular_radius {
                for side in Side::ALL {
                    let n = a.neighbor(map, side);
                    if seen.insert(n) {
                        flood.push(n);
                    }
                }
            }
        }
        while let Some(a) = covering.pop() {
            visits += 1;
            if visits > MAX_VISITS {
                return false;
            }
            let (lo, hi) = cap_of(geoms, a);
            let ok = lo.is_finite() && hi.is_finite() && lo <= hi && {
                let (lo, hi) = (lo - planet_generators::height::MONOTONE_SLACK_M, hi + planet_generators::height::MONOTONE_SLACK_M);
                let rho = extent(p.node_radius_bound(k) * (1.0 + hi.max(0.0) / p.radius_m), 0.5 * (hi - lo));
                morph_room(((1.0 - p.morph_band) * end).max(p.split_distance(k) + 2.0 * rho), end, 0.0)
            };
            if ok {
                level_half = level_half.max(0.5 * (hi - lo) + planet_generators::height::MONOTONE_SLACK_M);
                level_hi = level_hi.max(hi + planet_generators::height::MONOTONE_SLACK_M);
                continue;
            }
            // Halve it: the children serve this level while they are no finer than `k - 1`.
            if a.level() + 1 >= k {
                return false;
            }
            for c in children_of(a) {
                let gc = geoms.get(c);
                if angle_to(gc.dir) <= reach + gc.angular_radius {
                    covering.push(c);
                }
            }
        }
        previous = Some((level_half, level_hi));
    }
    true
}

/// The bounds of the field over the cap round the centre of `a` of radius its angular radius plus 1.1 of the angular radius bound of a tile one
/// level finer (the caps of the volumes of tiles of that level and finer, whose centres lie in `a`, are inside it), cached by tile (valid while
/// the cache's field and parameters are those it was filled for).
fn cap_of(geoms: &mut GeomCache<'_>, a: TileId) -> (f64, f64) {
    if let Some(&b) = geoms.caps.get(&a) {
        return b;
    }
    let g = geoms.get(a);
    let small = 1.1 * 1.02 * geoms.p.node_radius_bound((a.level() + 1).min(geoms.p.max_level)) / geoms.p.radius_m;
    let b = geoms.field.height_bounds(g.dir, g.angular_radius + small);
    geoms.caps.insert(a, b);
    b
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

// ----- tiles too steep to morph (ADR 0074)

/// The ancestor of `tile` at `level` (`tile` itself at its own level).
fn ancestor_at(tile: TileId, level: u8) -> TileId {
    let mut a = tile;
    while a.level() > level {
        a = a.parent().expect("a tile above the root level has a parent");
    }
    a
}

/// The levels the tiles too steep to morph leave, as a ceiling on the level of a leaf (ADR 0074). `u` is such a tile (its morph interval, from
/// its four same-level neighbours' volumes, has no room, [`morph_room`]) with no such ancestor, and `c` its parent: `c` stays a leaf, and with
/// it the siblings of `u`, whatever their own bounds say. A leaf over any area may be no finer than `min over u of (level(c) + d)`, with `d` the
/// number of tile edges between the level-`level(c)` tiles (the least over the descendants of a coarser tile, that of the ancestor for a finer
/// one). The ceiling is 1-Lipschitz across edges, so a selection clamped to it stays balanced (2:1), and it is a function of the unmorphable
/// tiles alone: the selection of any view and the residency set clamp to the same.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ceiling {
    /// Per level of an unmorphable tile: the ceiling's value at the tiles within reach of those tiles and at their ancestors.
    by_level: Vec<(u8, IdMap<u8>)>,
}

impl Ceiling {
    /// Whether no tile is too steep to morph.
    pub fn is_empty(&self) -> bool {
        self.by_level.is_empty()
    }

    /// Whether the ceiling forbids splitting `t`: a child would be finer than the ceiling over it.
    pub fn forbids_split(&self, t: TileId) -> bool {
        !self.is_empty() && t.children().is_some_and(|kids| kids.iter().any(|c| self.of(*c) < c.level()))
    }

    /// The finest level a leaf over `tile`'s area may have (`u8::MAX` where nothing limits it).
    pub fn of(&self, tile: TileId) -> u8 {
        let mut best = u8::MAX;
        for (level, caps) in &self.by_level {
            let key = if tile.level() <= *level { tile } else { ancestor_at(tile, *level) };
            if let Some(&c) = caps.get(&key) {
                best = best.min(c);
            }
        }
        best
    }

    /// The ceiling of the tiles `unmorphable` (the topmost ones, none below another) for leaves down to `max_level`.
    fn build(map: &dyn FaceMapping, unmorphable: &[TileId], max_level: u8) -> Ceiling {
        let mut by_level: Vec<(u8, IdMap<u8>)> = Vec::new();
        for &u in unmorphable {
            // The parent is the tile that stays a leaf (the siblings of `u` with it), and the distances are between its level's tiles.
            let Some(c) = u.parent() else { continue };
            let level = c.level();
            let caps = match by_level.iter().position(|(l, _)| *l == level) {
                Some(i) => &mut by_level[i].1,
                None => {
                    by_level.push((level, IdMap::default()));
                    &mut by_level.last_mut().expect("just pushed").1
                }
            };
            // A tile at edge distance `d` from `c` has the ceiling `level + d`, which says nothing from `max_level` on.
            let reach = max_level.saturating_sub(level + 1);
            let mut seen: IdSet = IdSet::default();
            seen.insert(c);
            let mut frontier = vec![c];
            for d in 0..=reach {
                let cap = level + d;
                let mut next = Vec::new();
                for &t in &frontier {
                    // The tile and its ancestors, down the chain while it lowers what they hold.
                    let mut a = Some(t);
                    while let Some(x) = a {
                        let slot = caps.entry(x).or_insert(u8::MAX);
                        if *slot <= cap {
                            break;
                        }
                        *slot = cap;
                        a = x.parent();
                    }
                    if d < reach {
                        for side in Side::ALL {
                            let n = t.neighbor(map, side);
                            if seen.insert(n) {
                                next.push(n);
                            }
                        }
                    }
                }
                frontier = next;
            }
        }
        by_level.sort_by_key(|(l, _)| *l);
        Ceiling { by_level }
    }

    /// The ceiling of the tiles of the selection `leaves` (and their ancestors) that are too steep to morph.
    fn of_unmorphable(geoms: &mut GeomCache<'_>, leaves: &BTreeSet<TileId>) -> Ceiling {
        let (map, p) = (geoms.map, geoms.p);
        let floor = p.floor_level();
        let h = geoms.field.max_abs_height_m();
        // Every tile of the selection above the floor, coarsest first.
        let mut seen: IdSet = IdSet::default();
        let mut tree: Vec<TileId> = Vec::new();
        for &leaf in leaves {
            let mut t = Some(leaf);
            while let Some(x) = t {
                if x.level() <= floor || !seen.insert(x) {
                    break;
                }
                tree.push(x);
                t = x.parent();
            }
        }
        tree.sort_unstable_by_key(|t| (t.level(), *t));
        // A level whose intervals read no bounds needs no look at its tiles.
        let always = |level: u8| !reads_bounds(p, level, h);
        let monotone = geoms.field.bounds_monotone();
        let mut dead: IdSet = IdSet::default();
        let mut unmorphable: Vec<TileId> = Vec::new();
        for t in tree {
            // The parent was judged first: under an unmorphable tile nothing is looked at.
            if t.parent().is_some_and(|a| dead.contains(&a)) {
                dead.insert(t);
                continue;
            }
            if always(t.level()) {
                continue;
            }
            // A certified subtree holds no unmorphable tile: neither it nor anything below it is judged.
            if monotone && certified(geoms, t) {
                dead.insert(t);
                continue;
            }
            let (start, end) = morph_interval_for(geoms, t);
            if !morph_room(start, end, 0.0) {
                unmorphable.push(t);
                dead.insert(t);
            }
        }
        if unmorphable.is_empty() {
            return Ceiling::default();
        }
        Ceiling::build(map, &unmorphable, p.max_level)
    }

    /// `leaves` clamped to the ceiling: a leaf finer than the ceiling over it is replaced by its ancestor at the ceiling, repeatedly (the ancestor's
    /// own ceiling is the least over its area), and a tile under another of the set is covered by it. Returns the leaves and the tiles among them
    /// that replaced finer ones (ADR 0074: the capped tiles).
    pub fn clamp(&self, leaves: &BTreeSet<TileId>) -> (BTreeSet<TileId>, BTreeSet<TileId>) {
        let mut cur = leaves.clone();
        let mut capped: BTreeSet<TileId> = BTreeSet::new();
        loop {
            let mut next: BTreeSet<TileId> = BTreeSet::new();
            let mut changed = false;
            for &z in &cur {
                let c = self.of(z);
                if z.level() > c {
                    let a = ancestor_at(z, c);
                    capped.insert(a);
                    next.insert(a);
                    changed = true;
                } else {
                    next.insert(z);
                }
            }
            if !changed {
                break;
            }
            let all: IdSet = next.iter().copied().collect();
            next.retain(|t| {
                let mut up = t.parent();
                while let Some(a) = up {
                    if all.contains(&a) {
                        return false;
                    }
                    up = a.parent();
                }
                true
            });
            cur = next;
        }
        capped.retain(|t| cur.contains(t));
        (cur, capped)
    }
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
    let theta = c_hat.angle_to(n_hat);
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
        worst = worst.max(c.angle_to(d));
    }
    // Margin for the sampled boundary: the maximum is attained at a corner for these tiles; keep a small relative safety.
    worst * 1.001
}

#[cfg(test)]
mod reuse_tests {
    use super::*;

    // spec: VIEW-043
    #[test]
    fn the_drawable_slope_is_the_neighbourhood_extent_at_which_the_morph_interval_has_no_room_and_is_one_number_for_all_levels() {
        let p = LodParams::earth_1080p();
        let slope = p.drawable_slope();
        assert!((2.3..2.7).contains(&slope), "about 68 degrees at the defaults: {slope}");
        for level in [10u8, 16, 19, 22] {
            let (a, end) = (p.node_radius_bound(level), p.split_distance(level - 1));
            // A neighbour of half range `slope * r` (the extent 2 H over the diameter 2 r) leaves the interval its 5 %; a little more does not.
            let start_at = |h: f64| p.split_distance(level) + 2.0 * (a * a + h * h).sqrt();
            assert!(start_at(slope * a * 0.999) <= 0.95 * end, "level {level}");
            assert!(start_at(slope * a * 1.001) > 0.95 * end, "level {level}");
        }
        // A tighter tolerance leaves room for less; parameters with no room give 0.
        assert!(LodParams { tau_px: 3.0, ..p }.drawable_slope() > slope);
        assert_eq!(LodParams { tau_px: 60.0, ..p }.drawable_slope(), 0.0);
    }

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

#[cfg(test)]
mod capping_tests {
    use super::*;
    use planet_core::cube::TangentWarp;
    use planet_core::hash::SplitMix64;

    const FINEST: u8 = 5;

    fn tiles_at(level: u8) -> Vec<TileId> {
        let side = 1u32 << level;
        let mut out = Vec::new();
        for f in Face::ALL {
            for x in 0..side {
                for y in 0..side {
                    out.push(TileId::new(f, level, x, y).expect("a tile"));
                }
            }
        }
        out
    }

    /// `n` tiles of levels 2 to 4, none under another (the topmost unmorphable tiles).
    fn some_unmorphable(rng: &mut SplitMix64, n: usize) -> Vec<TileId> {
        let mut out: Vec<TileId> = Vec::new();
        while out.len() < n {
            let level = 2 + (rng.next_u64() % 3) as u8;
            let all = tiles_at(level);
            let t = all[(rng.next_u64() % all.len() as u64) as usize];
            let nested =
                out.iter().any(|o| ancestor_at(t, o.level().min(t.level())) == *o || ancestor_at(*o, t.level().min(o.level())) == t);
            if !nested {
                out.push(t);
            }
        }
        out
    }

    // spec: LOD-008
    #[test]
    fn the_ceiling_of_unmorphable_tiles_gives_the_unique_finest_balanced_partition_in_any_order() {
        let mut rng = SplitMix64(41);
        for round in 0..6 {
            let unmorphable = some_unmorphable(&mut rng, 1 + round % 4);
            let ceiling = Ceiling::build(&TangentWarp, &unmorphable, FINEST);
            let all: BTreeSet<TileId> = tiles_at(FINEST).into_iter().collect();
            let (leaves, capped) = ceiling.clamp(&all);
            // A partition of the sphere: no leaf under another, the areas add up.
            let area: u64 = leaves.iter().map(|t| 4u64.pow(u32::from(FINEST - t.level()))).sum();
            assert_eq!(area, 6 * 4u64.pow(u32::from(FINEST)), "round {round}: the leaves tile the sphere");
            for t in &leaves {
                let mut up = t.parent();
                while let Some(a) = up {
                    assert!(!leaves.contains(&a), "round {round}: {t:?} lies under {a:?}");
                    up = a.parent();
                }
                assert!(ceiling.of(*t) >= t.level(), "round {round}: {t:?} is finer than its ceiling");
            }
            // None of the unmorphable tiles, nor anything under them, is a leaf; their parents are.
            for u in &unmorphable {
                assert!(leaves.iter().all(|l| ancestor_at(*l, u.level().min(l.level())) != *u || l.level() < u.level()), "round {round}");
                assert!(
                    leaves.contains(&u.parent().unwrap())
                        || leaves.iter().any(|l| l.level() < u.level() - 1 && ancestor_at(*u, l.level()) == *l)
                );
            }
            // Balanced: the splits of the balance find nothing to do.
            let mut balanced = leaves.clone();
            balance_leaves(&TangentWarp, &mut balanced);
            assert_eq!(balanced, leaves, "round {round}: balanced");
            // Finest: splitting any coarsened leaf puts a child over its ceiling or breaks the balance.
            for z in leaves.iter().filter(|z| z.level() < FINEST).step_by(3).take(16) {
                let kids = children_of(*z);
                let mut trial = leaves.clone();
                trial.remove(z);
                trial.extend(kids);
                let kids_fit = kids.iter().all(|k| ceiling.of(*k) >= k.level());
                let mut again = trial.clone();
                balance_leaves(&TangentWarp, &mut again);
                assert!(!kids_fit || again != trial, "round {round}: {z:?} could be split: the partition is not the finest");
            }
            assert!(capped.iter().all(|c| leaves.contains(c)));
            // Order and duplication of the unmorphable tiles change nothing.
            let mut shuffled = unmorphable.clone();
            shuffled.reverse();
            shuffled.extend(unmorphable.iter().copied());
            let (again_leaves, again_capped) = Ceiling::build(&TangentWarp, &shuffled, FINEST).clamp(&all);
            assert_eq!((again_leaves, again_capped), (leaves, capped), "round {round}: the order does not matter");
        }
    }

    // spec: LOD-008
    #[test]
    fn one_predicate_decides_the_room_of_an_interval_and_a_bound_that_is_not_a_number_leaves_none() {
        assert!(morph_room(100.0, 200.0, 0.0));
        assert!(morph_room(190.0, 200.0, 0.0), "exactly 95 %");
        assert!(!morph_room(190.000_000_000_000_03, 200.0, 0.0));
        assert!(!morph_room(f64::NAN, 200.0, 0.0) && !morph_room(100.0, f64::NAN, 0.0));
        assert!(!morph_room(f64::INFINITY, 200.0, 0.0), "no start is no room");
        // The margin of `validate` is the same rule with a smaller share.
        assert!(morph_room(179.0, 200.0, 0.05) && !morph_room(181.0, 200.0, 0.05));
    }
}
