//! CPU data products as images, so generator work can be looked at without any GPU (report §16).

use crate::height::TileData;
use planet_core::cube::{direction_to_face, face_to_direction, FaceMapping};
use planet_core::Face;
use std::io::BufWriter;
use std::path::Path;

/// Tightly packed RGBA8.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl Rgba {
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [self.data[i], self.data[i + 1], self.data[i + 2], self.data[i + 3]]
    }

    pub fn write_png(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut enc = png::Encoder::new(BufWriter::new(std::fs::File::create(path)?), self.width, self.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().map_err(std::io::Error::other)?.write_image_data(&self.data).map_err(std::io::Error::other)
    }
}

/// Flat colour per face (also the face debug view colours).
pub const FACE_COLORS: [[u8; 4]; 6] = [
    [230, 60, 60, 255],  // +X red
    [120, 20, 20, 255],  // -X dark red
    [60, 200, 80, 255],  // +Y green
    [20, 100, 30, 255],  // -Y dark green
    [70, 110, 240, 255], // +Z blue
    [20, 40, 120, 255],  // -Z dark blue
];
pub const NET_BACKGROUND: [u8; 4] = [255, 0, 255, 255];

/// Height range mapped to grey in dumps and the height debug view.
pub const HEIGHT_MIN_M: f32 = -4000.0;
pub const HEIGHT_MAX_M: f32 = 4000.0;

pub fn height_to_grey(h: f32) -> u8 {
    let t = ((h - HEIGHT_MIN_M) / (HEIGHT_MAX_M - HEIGHT_MIN_M)).clamp(0.0, 1.0);
    (t * 255.0 + 0.5).floor() as u8
}

/// Grey-scale height map of one tile, one pixel per sample (`res + 1` square).
pub fn tile_height_map(tile: &TileData) -> Rgba {
    let n = tile.res + 1;
    let mut data = Vec::with_capacity((n * n * 4) as usize);
    for l in 0..n {
        for k in 0..n {
            let g = height_to_grey(tile.at(k, l));
            data.extend([g, g, g, 255]);
        }
    }
    Rgba { width: n, height: n, data }
}

/// Cell of each face in the unfolded cross (4 columns by 3 rows). +Z sits above -Y and -Z below it, the only
/// placements where U and V continue across the shared edge (checked by a continuity test).
pub const NET_CELLS: [(u32, u32); 6] = [(1, 1), (3, 1), (2, 1), (0, 1), (0, 0), (0, 2)];

/// Unfolded six-face cube net, `cell` pixels per face. Pixels outside the six cells are `NET_BACKGROUND`.
/// Each face pixel is coloured by the face found from its own 3D direction, so a wrong mapping shows up as a wrong colour;
/// dark lines mark the tile boundaries of a level-`grid_level` grid.
pub fn face_net(map: &dyn FaceMapping, cell: u32, grid_level: u8) -> Rgba {
    let (w, h) = (cell * 4, cell * 3);
    let mut data = NET_BACKGROUND.repeat((w * h) as usize);
    let n = f64::from(1u32 << grid_level);
    for (f, &(cx, cy)) in NET_CELLS.iter().enumerate() {
        for py in 0..cell {
            for px in 0..cell {
                // Pixel centre in face coordinates; t grows upwards in the image.
                let s = (f64::from(px) + 0.5) / f64::from(cell) * 2.0 - 1.0;
                let t = 1.0 - (f64::from(py) + 0.5) / f64::from(cell) * 2.0;
                let (face, _, _) = direction_to_face(map, face_to_direction(map, Face(f as u8), s, t));
                let mut colour = FACE_COLORS[face.0 as usize];
                let (gs, gt) = ((s + 1.0) * 0.5 * n, (t + 1.0) * 0.5 * n);
                let line = 1.0 / f64::from(cell) * n;
                if gs - libm::floor(gs) < line || gt - libm::floor(gt) < line {
                    colour = [colour[0] / 2, colour[1] / 2, colour[2] / 2, 255];
                }
                let i = (((cy * cell + py) * w + cx * cell + px) * 4) as usize;
                data[i..i + 4].copy_from_slice(&colour);
            }
        }
    }
    Rgba { width: w, height: h, data }
}
