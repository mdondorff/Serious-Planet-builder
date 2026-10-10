//! `FramePlan`: a plain, serialisable description of one frame (ADR 0009), and its builder.

use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::hash::hash_u64s;
use planet_core::{PlanetFixed, TileId};
use planet_generators::dump::FACE_COLORS;

pub const PLAN_FORMAT_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugView {
    /// Flat colour per cube face.
    Face,
    /// Flat colour derived from the tile id.
    TileId,
    /// Grey ramp of the height samples.
    Height,
}

impl DebugView {
    pub fn name(self) -> &'static str {
        match self {
            DebugView::Face => "face",
            DebugView::TileId => "tile-id",
            DebugView::Height => "height",
        }
    }

    pub fn parse(s: &str) -> Option<DebugView> {
        [DebugView::Face, DebugView::TileId, DebugView::Height].into_iter().find(|v| v.name() == s)
    }
}

/// One tile to draw into a pixel rectangle of the frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileDraw {
    pub tile: TileId,
    /// Tile centre on the planet, float64 planet-fixed (CON-11).
    pub origin: PlanetFixed,
    /// `origin - camera`, float32: what a shader would receive.
    pub camera_relative_origin: [f32; 3],
    /// Pixel rectangle `[x, y, width, height]` inside the frame.
    pub rect: [u32; 4],
    /// Flat colour for the face and tile-id views.
    pub color: [u8; 4],
}

#[derive(Clone, Debug, PartialEq)]
pub struct FramePlan {
    pub size: (u32, u32),
    pub view: DebugView,
    pub camera: PlanetFixed,
    pub clear_color: [u8; 4],
    /// Height range mapped onto the grey ramp of the height view, metres.
    pub height_range: [f32; 2],
    pub draws: Vec<TileDraw>,
}

/// Everything the builder needs; stored in repro bundles so a plan can be rebuilt.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanRequest {
    pub tiles: Vec<TileId>,
    pub camera: PlanetFixed,
    pub size: (u32, u32),
    pub view: DebugView,
    pub radius_m: f64,
    pub face_mapping: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PlanError {
    InvalidLod(String),
    UnknownFaceMapping(String),
    NoTiles,
    FrameTooSmall { size: (u32, u32), tiles: usize },
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanError::UnknownFaceMapping(m) => write!(f, "unknown face mapping '{m}' (known: tangent-v1)"),
            PlanError::InvalidLod(m) => write!(f, "invalid LOD parameters: {m}"),
            PlanError::NoTiles => write!(f, "a plan needs at least one tile"),
            PlanError::FrameTooSmall { size, tiles } => write!(f, "frame {}x{} is too small for {tiles} tiles", size.0, size.1),
        }
    }
}

impl std::error::Error for PlanError {}

pub fn face_mapping_by_id(id: &str) -> Option<Box<dyn FaceMapping + Sync>> {
    (id == TangentWarp.id()).then(|| Box::new(TangentWarp) as Box<dyn FaceMapping + Sync>)
}

pub const CLEAR_COLOR: [u8; 4] = [255, 0, 255, 255];
pub const HEIGHT_RANGE: [f32; 2] = [planet_generators::dump::HEIGHT_MIN_M, planet_generators::dump::HEIGHT_MAX_M];

/// Flat colour of the tile-id view: stable per id, never the clear colour.
pub fn tile_id_color(id: TileId) -> [u8; 4] {
    let h = hash_u64s(0x7469_6c65_6964, &[id.raw()]);
    [60 + (h & 0x7f) as u8, 60 + ((h >> 8) & 0x7f) as u8, 60 + ((h >> 16) & 0x7f) as u8, 255]
}

/// Plan a row of equal-width cells, one tile per cell, left to right in the order given.
pub fn plan_tiles(req: &PlanRequest) -> Result<FramePlan, PlanError> {
    let map = face_mapping_by_id(&req.face_mapping).ok_or_else(|| PlanError::UnknownFaceMapping(req.face_mapping.clone()))?;
    if req.tiles.is_empty() {
        return Err(PlanError::NoTiles);
    }
    let n = req.tiles.len() as u32;
    let cell_w = req.size.0 / n;
    if cell_w == 0 || req.size.1 == 0 {
        return Err(PlanError::FrameTooSmall { size: req.size, tiles: req.tiles.len() });
    }
    let draws = req
        .tiles
        .iter()
        .enumerate()
        .map(|(i, &tile)| {
            let origin = PlanetFixed(tile.center_direction(map.as_ref()) * req.radius_m);
            let color = match req.view {
                DebugView::Face => FACE_COLORS[tile.face().0 as usize],
                DebugView::TileId => tile_id_color(tile),
                DebugView::Height => [0, 0, 0, 255],
            };
            TileDraw {
                tile,
                origin,
                camera_relative_origin: origin.camera_relative(req.camera).0,
                rect: [i as u32 * cell_w, 0, cell_w, req.size.1],
                color,
            }
        })
        .collect();
    Ok(FramePlan { size: req.size, view: req.view, camera: req.camera, clear_color: CLEAR_COLOR, height_range: HEIGHT_RANGE, draws })
}

fn hex(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}

fn hex32(v: f32) -> String {
    format!("{:08x}", v.to_bits())
}

fn parse_hex64(s: &str) -> Result<f64, String> {
    u64::from_str_radix(s, 16).map(f64::from_bits).map_err(|e| format!("bad f64 bits '{s}': {e}"))
}

fn parse_hex32(s: &str) -> Result<f32, String> {
    u32::from_str_radix(s, 16).map(f32::from_bits).map_err(|e| format!("bad f32 bits '{s}': {e}"))
}

impl FramePlan {
    /// Stable text form: one record per line, floats as hexadecimal bit patterns (exact, diffable).
    pub fn to_text(&self) -> String {
        let mut s = format!("planet-frameplan {PLAN_FORMAT_VERSION}\n");
        s += &format!("size {} {}\nview {}\n", self.size.0, self.size.1, self.view.name());
        s += &format!("camera {} {} {}\n", hex(self.camera.0.x), hex(self.camera.0.y), hex(self.camera.0.z));
        let c = self.clear_color;
        s += &format!(
            "clear {} {} {} {}\nheight_range {} {}\n",
            c[0],
            c[1],
            c[2],
            c[3],
            hex32(self.height_range[0]),
            hex32(self.height_range[1])
        );
        for d in &self.draws {
            let o = d.origin.0;
            let r = d.camera_relative_origin;
            s += &format!(
                "draw {:016x} origin {} {} {} rel {} {} {} rect {} {} {} {} color {} {} {} {}\n",
                d.tile.raw(),
                hex(o.x),
                hex(o.y),
                hex(o.z),
                hex32(r[0]),
                hex32(r[1]),
                hex32(r[2]),
                d.rect[0],
                d.rect[1],
                d.rect[2],
                d.rect[3],
                d.color[0],
                d.color[1],
                d.color[2],
                d.color[3]
            );
        }
        s
    }

    pub fn from_text(text: &str) -> Result<FramePlan, String> {
        let mut lines = text.lines();
        let head = lines.next().ok_or("empty plan")?;
        if head != format!("planet-frameplan {PLAN_FORMAT_VERSION}") {
            return Err(format!("unsupported plan header '{head}' (this build reads planet-frameplan {PLAN_FORMAT_VERSION})"));
        }
        let (mut size, mut view, mut camera, mut clear, mut range) = (None, None, None, None, None);
        let mut draws = Vec::new();
        for line in lines.filter(|l| !l.trim().is_empty()) {
            let t: Vec<&str> = line.split_whitespace().collect();
            let num = |s: &str| s.parse::<u32>().map_err(|e| format!("bad number '{s}' in '{line}': {e}"));
            let byte = |s: &str| s.parse::<u8>().map_err(|e| format!("bad colour value '{s}' in '{line}': {e}"));
            match t.as_slice() {
                ["size", w, h] => size = Some((num(w)?, num(h)?)),
                ["view", v] => view = Some(DebugView::parse(v).ok_or(format!("unknown view '{v}'"))?),
                ["camera", x, y, z] => {
                    camera = Some(PlanetFixed(planet_core::Vec3::new(parse_hex64(x)?, parse_hex64(y)?, parse_hex64(z)?)))
                }
                ["clear", r, g, b, a] => clear = Some([byte(r)?, byte(g)?, byte(b)?, byte(a)?]),
                ["height_range", a, b] => range = Some([parse_hex32(a)?, parse_hex32(b)?]),
                ["draw", id, "origin", ox, oy, oz, "rel", rx, ry, rz, "rect", x, y, w, h, "color", r, g, b, a] => {
                    let raw = u64::from_str_radix(id, 16).map_err(|e| format!("bad tile id '{id}': {e}"))?;
                    draws.push(TileDraw {
                        tile: TileId::from_raw(raw).ok_or(format!("invalid tile id {raw:#x}"))?,
                        origin: PlanetFixed(planet_core::Vec3::new(parse_hex64(ox)?, parse_hex64(oy)?, parse_hex64(oz)?)),
                        camera_relative_origin: [parse_hex32(rx)?, parse_hex32(ry)?, parse_hex32(rz)?],
                        rect: [num(x)?, num(y)?, num(w)?, num(h)?],
                        color: [byte(r)?, byte(g)?, byte(b)?, byte(a)?],
                    });
                }
                _ => return Err(format!("cannot parse plan line '{line}'")),
            }
        }
        Ok(FramePlan {
            size: size.ok_or("plan has no size")?,
            view: view.ok_or("plan has no view")?,
            camera: camera.ok_or("plan has no camera")?,
            clear_color: clear.ok_or("plan has no clear colour")?,
            height_range: range.ok_or("plan has no height range")?,
            draws,
        })
    }

    /// Hash of the text form, for quick comparison and repro bundles.
    pub fn plan_hash(&self) -> u64 {
        let bytes: Vec<u64> = self.to_text().bytes().map(u64::from).collect();
        hash_u64s(0x706c_616e, &bytes)
    }

    /// Human-readable differences to `other`, with numbers; empty when equal.
    pub fn diff(&self, other: &FramePlan) -> Vec<String> {
        let mut d = Vec::new();
        if self.size != other.size {
            d.push(format!("size {:?} vs {:?}", self.size, other.size));
        }
        if self.view != other.view {
            d.push(format!("view {} vs {}", self.view.name(), other.view.name()));
        }
        let v3 = |p: &PlanetFixed| [p.0.x.to_bits(), p.0.y.to_bits(), p.0.z.to_bits()];
        if v3(&self.camera) != v3(&other.camera) {
            d.push(format!("camera {:?} vs {:?}", self.camera.0, other.camera.0));
        }
        if self.clear_color != other.clear_color {
            d.push(format!("clear colour {:?} vs {:?}", self.clear_color, other.clear_color));
        }
        let r32 = |r: &[f32; 2]| [r[0].to_bits(), r[1].to_bits()];
        if r32(&self.height_range) != r32(&other.height_range) {
            d.push(format!("height range {:?} vs {:?}", self.height_range, other.height_range));
        }
        if self.draws.len() != other.draws.len() {
            d.push(format!("{} draws vs {}", self.draws.len(), other.draws.len()));
        }
        for (i, (a, b)) in self.draws.iter().zip(&other.draws).enumerate() {
            if a.tile != b.tile {
                d.push(format!("draw {i}: tile {:?} vs {:?}", a.tile, b.tile));
            }
            if v3(&a.origin) != v3(&b.origin) {
                d.push(format!("draw {i}: origin differs by {:e} m", a.origin.0.distance(b.origin.0)));
            }
            if a.camera_relative_origin.map(f32::to_bits) != b.camera_relative_origin.map(f32::to_bits) {
                d.push(format!("draw {i}: camera-relative origin {:?} vs {:?}", a.camera_relative_origin, b.camera_relative_origin));
            }
            if a.rect != b.rect || a.color != b.color {
                d.push(format!("draw {i}: rect/colour {:?}/{:?} vs {:?}/{:?}", a.rect, a.color, b.rect, b.color));
            }
        }
        d
    }
}
