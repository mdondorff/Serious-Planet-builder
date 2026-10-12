//! Terrain part of the FramePlan: the nodes to draw, their camera-relative origins, morph intervals and the camera matrix.
//! Everything the terrain renderer needs is decided here (CON-03).

use crate::camera::Camera;
use crate::lod::{select_nodes_in_view_cached, LodParams, SelectionCache};
use crate::plan::{face_mapping_by_id, PlanError, CLEAR_COLOR, HEIGHT_RANGE};
use planet_core::hash::hash_u64s;
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_generators::dump::FACE_COLORS;
use planet_generators::height::HeightField;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerrainView {
    Face,
    TileId,
    Level,
    Morph,
    Height,
    Normals,
    Depth,
    /// Fixed-sun hillshade with a height tint, an adaptive metric grid and fine contours: stays readable from orbit down
    /// to 1 m, where the flat-colour views are a single colour. Appended last so the other views keep their shader index.
    Relief,
    /// Grey ramp of the height the shader morphs to minus the vertex's own height, over the plan's height range used as the
    /// delta window (a test or a human sets a narrow range to see centimetres): the observable of the height geomorph.
    MorphHeight,
}

impl TerrainView {
    pub const ALL: [TerrainView; 9] = [
        TerrainView::Face,
        TerrainView::TileId,
        TerrainView::Level,
        TerrainView::Morph,
        TerrainView::Height,
        TerrainView::Normals,
        TerrainView::Depth,
        TerrainView::Relief,
        TerrainView::MorphHeight,
    ];

    pub fn name(self) -> &'static str {
        match self {
            TerrainView::Face => "face",
            TerrainView::TileId => "tile-id",
            TerrainView::Level => "level",
            TerrainView::Morph => "morph",
            TerrainView::Height => "height",
            TerrainView::Normals => "normals",
            TerrainView::Depth => "depth",
            TerrainView::Relief => "relief",
            TerrainView::MorphHeight => "morph-height",
        }
    }

    pub fn parse(s: &str) -> Option<TerrainView> {
        TerrainView::ALL.into_iter().find(|v| v.name() == s)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainDraw {
    pub tile: TileId,
    pub level: u8,
    pub origin: PlanetFixed,
    /// `origin - camera`, float64 difference rounded once to f32.
    pub camera_relative_origin: [f32; 3],
    /// Selection-level morph (0..1) of the node centre as the CPU selection computed it: a diagnostic carried in the
    /// plan text and snapshots. The renderer does not upload it; the shader derives the per-vertex morph from the
    /// interval below.
    pub morph: f32,
    /// Vertex distances (metres) between which a vertex morphs to the parent grid.
    pub morph_start_m: f32,
    pub morph_end_m: f32,
    /// Flat colour of the face, tile-id and level views.
    pub color: [u8; 4],
    /// Key of the mesh this draw expects ([`planet_generators::mesh::mesh_key`]): a resident buffer with another key is
    /// stale and must not be drawn.
    pub mesh_key: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TerrainPlan {
    pub size: (u32, u32),
    pub view: TerrainView,
    pub camera: Camera,
    /// Column-major, camera-relative, reversed-Z infinite far (f32).
    pub view_projection: [f32; 16],
    pub clear_color: [u8; 4],
    pub height_range: [f32; 2],
    pub cells: u32,
    pub nodes: Vec<TerrainDraw>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TerrainRequest {
    pub camera: Camera,
    pub size: (u32, u32),
    pub view: TerrainView,
    pub lod: LodParams,
    pub face_mapping: String,
    /// Generator seed: tile origins sit at their centre terrain height, so the plan needs the height function.
    pub seed: u64,
}

pub fn level_color(level: u8) -> [u8; 4] {
    let l = u32::from(level);
    [(55 + (l * 53) % 200) as u8, (55 + (l * 97 + 30) % 200) as u8, (55 + (l * 31 + 10) % 200) as u8, 255]
}

fn tile_id_color(id: TileId) -> [u8; 4] {
    crate::plan::tile_id_color(id)
}

pub fn plan_terrain(req: &TerrainRequest) -> Result<TerrainPlan, PlanError> {
    plan_terrain_field(req, &planet_generators::height::Reference(req.seed))
}

/// [`plan_terrain`] with tile origins and mesh keys from `field` (stress tests). `field.seed()` must equal `req.seed`,
/// and the field's height bound must fit `req.lod.max_height_m` (horizon and frustum padding).
pub fn plan_terrain_field(req: &TerrainRequest, field: &dyn HeightField) -> Result<TerrainPlan, PlanError> {
    plan_terrain_cached(req, field, &mut SelectionCache::new())
}

/// [`plan_terrain_field`] reusing the per-tile selection work of earlier plans in `cache` (a renderer plans every frame;
/// the plan is identical to the uncached one).
pub fn plan_terrain_cached(req: &TerrainRequest, field: &dyn HeightField, cache: &mut SelectionCache) -> Result<TerrainPlan, PlanError> {
    if field.seed() != req.seed {
        return Err(PlanError::HeightField(format!("field seed {} differs from request seed {}", field.seed(), req.seed)));
    }
    if field.max_abs_height_m() > req.lod.max_height_m {
        return Err(PlanError::HeightField(format!(
            "heights up to {} m exceed max_height_m {} m used for horizon and frustum padding",
            field.max_abs_height_m(),
            req.lod.max_height_m
        )));
    }
    let map = face_mapping_by_id(&req.face_mapping).ok_or_else(|| PlanError::UnknownFaceMapping(req.face_mapping.clone()))?;
    if req.size.0 == 0 || req.size.1 == 0 {
        return Err(PlanError::FrameTooSmall { size: req.size, tiles: 0 });
    }
    req.lod.validate().map_err(PlanError::InvalidLod)?;
    let selection = select_nodes_in_view_cached(map.as_ref(), &req.camera, field, &req.lod, cache);
    // A morph interval that cannot be crack-free (the neighbourhood's height extent leaves no room) is an error, never a
    // narrowed interval: relief this steep at this tile size needs a larger viewport_h_px / tau_px or fewer cells.
    if let Some(n) = selection.nodes.iter().find(|n| n.tile.level() > 0 && n.morph_start_m > 0.95 * n.morph_end_m) {
        return Err(PlanError::InvalidLod(format!(
            "tile {:?} (level {}): morph interval [{:.1}, {:.1}] m is empty because the height extent of its neighbourhood leaves no room; \
             this terrain is too steep for cells {} and viewport_h_px / tau_px {:.0}: raise viewport_h_px / tau_px or reduce cells",
            n.tile,
            n.tile.level(),
            n.morph_start_m,
            n.morph_end_m,
            req.lod.cells,
            req.lod.viewport_h_px / req.lod.tau_px
        )));
    }
    let keyer = planet_generators::mesh::MeshKeyer::new(field, map.as_ref(), req.lod.cells, req.lod.radius_m);
    let nodes = selection
        .nodes
        .iter()
        .map(|n| {
            let centre = n.tile.center_direction(map.as_ref());
            let origin = PlanetFixed(centre * (req.lod.radius_m + field.height(centre)));
            let rel = origin.0 - req.camera.position.0;
            let color = match req.view {
                TerrainView::Face => FACE_COLORS[n.tile.face().0 as usize],
                TerrainView::TileId => tile_id_color(n.tile),
                TerrainView::Level => level_color(n.tile.level()),
                _ => [0, 0, 0, 255],
            };
            TerrainDraw {
                tile: n.tile,
                level: n.tile.level(),
                origin,
                camera_relative_origin: [rel.x as f32, rel.y as f32, rel.z as f32],
                morph: n.morph,
                morph_start_m: n.morph_start_m as f32,
                morph_end_m: n.morph_end_m as f32,
                color,
                mesh_key: keyer.key(n.tile),
            }
        })
        .collect();
    Ok(TerrainPlan {
        size: req.size,
        view: req.view,
        camera: req.camera,
        view_projection: req.camera.view_projection(),
        clear_color: CLEAR_COLOR,
        height_range: HEIGHT_RANGE,
        cells: req.lod.cells,
        nodes,
    })
}

/// Counts a plan implies, derived from the plan alone so that tier A can assert budgets on any machine and tier C can
/// check that the renderer matches them (TEST-011, CON-20).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlanCounts {
    pub draw_calls: u32,
    pub triangles: u64,
    /// Distinct tiles that must be resident to draw the plan.
    pub resident_tiles: u32,
    /// Vertex bytes uploaded to draw the plan from a cold start (one buffer per resident tile).
    pub upload_bytes: u64,
}

/// Upper limits for [`PlanCounts`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CountBudget {
    pub max_draw_calls: u32,
    pub max_triangles: u64,
    pub max_resident_tiles: u32,
    pub max_upload_bytes: u64,
}

impl CountBudget {
    /// Budget for the 1440p production-like workload (cells 32, tau 6 px; ADR 0013): the measured worst case is 8,687 draws at
    /// 1 m above the terrain (LOD-008: 5,108 while selection mistook the terrain height for altitude), so 10,000 leaves
    /// headroom without hiding a doubling.
    pub const REFERENCE_1440P: CountBudget = CountBudget {
        max_draw_calls: 10_000,
        max_triangles: 10_000 * 2_304,
        max_resident_tiles: 10_000,
        max_upload_bytes: 10_000 * 1_221 * 44,
    };
}

impl PlanCounts {
    /// `Err` names every exceeded count with its value and budget.
    pub fn check(&self, budget: &CountBudget) -> Result<(), String> {
        let mut over = Vec::new();
        if self.draw_calls > budget.max_draw_calls {
            over.push(format!("draw calls {} > budget {}", self.draw_calls, budget.max_draw_calls));
        }
        if self.triangles > budget.max_triangles {
            over.push(format!("triangles {} > budget {}", self.triangles, budget.max_triangles));
        }
        if self.resident_tiles > budget.max_resident_tiles {
            over.push(format!("resident tiles {} > budget {}", self.resident_tiles, budget.max_resident_tiles));
        }
        if self.upload_bytes > budget.max_upload_bytes {
            over.push(format!("upload bytes {} > budget {}", self.upload_bytes, budget.max_upload_bytes));
        }
        if over.is_empty() {
            Ok(())
        } else {
            Err(over.join("; "))
        }
    }
}

impl TerrainPlan {
    /// The deterministic counts this plan implies: one draw call and one index set per node, one vertex buffer per
    /// distinct tile.
    pub fn counts(&self) -> PlanCounts {
        let per_tile_triangles = (planet_generators::mesh::grid_indices(self.cells).len() / 3) as u64;
        let tiles: std::collections::BTreeSet<_> = self.nodes.iter().map(|n| n.tile).collect();
        let vertex_bytes = planet_generators::mesh::vertex_count(self.cells) as u64 * planet_generators::mesh::MeshVertex::STRIDE;
        PlanCounts {
            draw_calls: self.nodes.len() as u32,
            triangles: self.nodes.len() as u64 * per_tile_triangles,
            resident_tiles: tiles.len() as u32,
            upload_bytes: tiles.len() as u64 * vertex_bytes,
        }
    }

    /// Compact text for snapshots: node count, per-level counts and a hash over everything numeric in the plan.
    pub fn summary(&self) -> String {
        let mut hist = [0usize; 29];
        let mut v: Vec<u64> = self.view_projection.iter().map(|f| u64::from(f.to_bits())).collect();
        for n in &self.nodes {
            hist[n.level as usize] += 1;
            v.push(n.tile.raw());
            v.extend(n.camera_relative_origin.iter().map(|f| u64::from(f.to_bits())));
            v.extend([n.morph, n.morph_start_m, n.morph_end_m].map(|f| u64::from(f.to_bits())));
        }
        let levels: Vec<String> = hist.iter().enumerate().filter(|(_, c)| **c > 0).map(|(l, c)| format!("L{l}:{c}")).collect();
        format!("nodes {} [{}] hash {:016x}", self.nodes.len(), levels.join(" "), hash_u64s(0x7465_7272, &v))
    }
}

/// Point `h` metres above the terrain of the generator with `seed` (not above the reference sphere) at a latitude and
/// longitude in degrees. Shared by the scripted views and the flight.
fn above_terrain(radius_m: f64, field: &dyn HeightField, lat_deg: f64, lon_deg: f64, h: f64) -> PlanetFixed {
    let (la, lo) = (lat_deg.to_radians(), lon_deg.to_radians());
    let dir = Vec3::new(libm::cos(la) * libm::cos(lo), libm::cos(la) * libm::sin(lo), libm::sin(la));
    PlanetFixed(dir * (radius_m + field.height(dir) + h))
}

/// Ten scripted cameras (orbit down to 1 m, poles, a cube corner and a face edge), shared by golden tests,
/// the performance harness and manual review (report §16). Looking direction is horizontal-north pitched down,
/// except at orbit where the camera looks at the planet centre.
pub fn scripted_views(radius_m: f64, aspect: f64, seed: u64) -> Vec<(&'static str, Camera)> {
    scripted_views_field(radius_m, aspect, &planet_generators::height::Reference(seed))
}

/// [`scripted_views`] with camera heights measured above `field` (stress tests).
pub fn scripted_views_field(radius_m: f64, aspect: f64, field: &dyn HeightField) -> Vec<(&'static str, Camera)> {
    let fov = 60f64.to_radians();
    let at = |lat: f64, lon: f64, h: f64| above_terrain(radius_m, field, lat, lon, h);
    let north = Vec3::new(0.0, 0.0, 1.0);
    let near = 0.1;
    let surf = |lat, lon, h, pitch_deg: f64| Camera::at_surface_point(at(lat, lon, h), north, pitch_deg.to_radians(), fov, aspect, near);
    vec![
        ("orbit-20000km", Camera::looking_at_centre(at(20.0, 30.0, 2.0e7), north, fov, aspect, near)),
        ("high-2000km", surf(20.0, 30.0, 2.0e6, 70.0)),
        ("aerial-200km", surf(20.0, 30.0, 2.0e5, 45.0)),
        ("aerial-20km", surf(20.0, 30.0, 2.0e4, 30.0)),
        ("low-2km", surf(20.0, 30.0, 2.0e3, 20.0)),
        ("ground-200m", surf(20.0, 30.0, 200.0, 12.0)),
        ("ground-1m", surf(20.0, 30.0, 1.0, 8.0)),
        ("north-pole-1km", surf(90.0, 0.0, 1000.0, 15.0)),
        ("cube-corner-1km", surf(35.264_389_682_754_654, 45.0, 1000.0, 15.0)),
        ("face-edge-1km", surf(0.0, 45.0, 1000.0, 15.0)),
    ]
}

/// Altitude at the start of [`flight_path`] (above the terrain).
pub const FLIGHT_START_M: f64 = 2.0e7;
/// Altitude of the final glide of [`flight_path`] (above the terrain).
pub const FLIGHT_END_M: f64 = 1.0;
/// Distance per frame during the final glide, measured on the reference sphere (at the camera's radius the step is
/// longer by (R + terrain + 1 m) / R, below 1e-3 relative): small enough that vertex swimming shows between frames.
pub const FLIGHT_GLIDE_STEP_M: f64 = 0.05;

/// The scripted flight for the real-GPU jitter review (tier D): `frames` cameras, the first 80 % diving from
/// [`FLIGHT_START_M`] to [`FLIGHT_END_M`] above the terrain at 20° N 30° E (log-spaced altitude, pitch from 85° to 8°,
/// heading north), the rest gliding north at [`FLIGHT_END_M`] by [`FLIGHT_GLIDE_STEP_M`] per frame. Pure and
/// deterministic. `frames` must be at least 10.
pub fn flight_path(radius_m: f64, aspect: f64, seed: u64, frames: usize) -> Vec<Camera> {
    assert!(frames >= 10, "a flight needs at least 10 frames, got {frames}");
    let fov = 60f64.to_radians();
    let north = Vec3::new(0.0, 0.0, 1.0);
    let reference = planet_generators::height::Reference(seed);
    let at = |lat: f64, lon: f64, h: f64| above_terrain(radius_m, &reference, lat, lon, h);
    let dive = frames * 4 / 5;
    let ratio = FLIGHT_END_M / FLIGHT_START_M;
    (0..frames)
        .map(|i| {
            let (lat, h, pitch_deg) = if i < dive {
                let t = i as f64 / (dive - 1) as f64;
                (20.0, FLIGHT_START_M * libm::pow(ratio, t), 85.0 - 77.0 * t)
            } else {
                let s = (i - dive + 1) as f64 * FLIGHT_GLIDE_STEP_M;
                (20.0 + (s / radius_m).to_degrees(), FLIGHT_END_M, 8.0)
            };
            Camera::at_surface_point(at(lat, 30.0, h), north, pitch_deg.to_radians(), fov, aspect, 0.1)
        })
        .collect()
}
