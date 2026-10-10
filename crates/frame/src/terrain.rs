//! Terrain part of the FramePlan: the nodes to draw, their camera-relative origins, morph intervals and the camera matrix.
//! Everything the terrain renderer needs is decided here (CON-03).

use crate::camera::Camera;
use crate::lod::{select_nodes_in_view, LodParams};
use crate::plan::{face_mapping_by_id, PlanError, CLEAR_COLOR, HEIGHT_RANGE};
use planet_core::hash::hash_u64s;
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_generators::dump::FACE_COLORS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerrainView {
    Face,
    TileId,
    Level,
    Morph,
    Height,
    Normals,
    Depth,
}

impl TerrainView {
    pub const ALL: [TerrainView; 7] = [
        TerrainView::Face,
        TerrainView::TileId,
        TerrainView::Level,
        TerrainView::Morph,
        TerrainView::Height,
        TerrainView::Normals,
        TerrainView::Depth,
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
    /// Selection-level morph (0..1), shown by the morph view for the node centre.
    pub morph: f32,
    /// Vertex distances (metres) between which a vertex morphs to the parent grid.
    pub morph_start_m: f32,
    pub morph_end_m: f32,
    /// Flat colour of the face, tile-id and level views.
    pub color: [u8; 4],
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
}

pub fn level_color(level: u8) -> [u8; 4] {
    let l = u32::from(level);
    [(55 + (l * 53) % 200) as u8, (55 + (l * 97 + 30) % 200) as u8, (55 + (l * 31 + 10) % 200) as u8, 255]
}

fn tile_id_color(id: TileId) -> [u8; 4] {
    crate::plan::tile_id_color(id)
}

pub fn plan_terrain(req: &TerrainRequest) -> Result<TerrainPlan, PlanError> {
    let map = face_mapping_by_id(&req.face_mapping).ok_or_else(|| PlanError::UnknownFaceMapping(req.face_mapping.clone()))?;
    if req.size.0 == 0 || req.size.1 == 0 {
        return Err(PlanError::FrameTooSmall { size: req.size, tiles: 0 });
    }
    let selection = select_nodes_in_view(map.as_ref(), &req.camera, &req.lod);
    let nodes = selection
        .nodes
        .iter()
        .map(|n| {
            let origin = PlanetFixed(n.tile.center_direction(map.as_ref()) * req.lod.radius_m);
            let rel = origin.0 - req.camera.position.0;
            let (start, end) = match n.tile.level() {
                0 => (1.0e30, 1.0e30),
                l => {
                    let end = req.lod.split_distance(l - 1);
                    (end * (1.0 - req.lod.morph_band), end)
                }
            };
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
                morph_start_m: start as f32,
                morph_end_m: end as f32,
                color,
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

impl TerrainPlan {
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

/// Ten scripted cameras (orbit down to 1 m, poles, a cube corner and a face edge), shared by golden tests,
/// the performance harness and manual review (report §16). Looking direction is horizontal-north pitched down,
/// except at orbit where the camera looks at the planet centre.
pub fn scripted_views(radius_m: f64, aspect: f64, seed: u64) -> Vec<(&'static str, Camera)> {
    let fov = 60f64.to_radians();
    let at = |lat: f64, lon: f64, h: f64| {
        let (la, lo) = (lat.to_radians(), lon.to_radians());
        let dir = Vec3::new(libm::cos(la) * libm::cos(lo), libm::cos(la) * libm::sin(lo), libm::sin(la));
        // `h` is the height above the terrain of the generator with `seed`, not above the reference sphere.
        PlanetFixed(dir * (radius_m + planet_generators::height::height_at(seed, dir) + h))
    };
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
