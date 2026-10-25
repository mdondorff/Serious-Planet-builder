//! SPIKE-43 throwaway (spike branch only, never merged): dump a square window of base heights round a chunk.
use std::io::Write;
use std::sync::Arc;

use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_generators::chunkfield::{Cell, ChunkLattice};

use crate::anetworld::{cells_per_chunk_for, WorldBase, ANET_CELL_M};
use crate::worldfield::WorldTerrain;
use planet_generators::anet::BaseDetail;

impl WorldTerrain {
    /// Writes `<out>/win_<tag>.bin`: per (u32), cell_m (f64), halo (u32), width (u32), origin face/i/j of the window's first cell (3 x i32), then width^2 base heights (f32, j outer).
    /// The window is the chunk over (lat, lon) with `halo` cells all round.
    pub fn fine_dump(self: &Arc<Self>, lat_deg: f64, lon_deg: f64, halo: u32, out: &std::path::Path, tag: &str) -> Result<String, String> {
        let radius = self.radius_m();
        let per = cells_per_chunk_for(radius, ANET_CELL_M) as usize;
        let clat = ChunkLattice::new(per as u32).ok_or("no chunk lattice")?;
        let cell_m = clat.cell_m(radius);
        let base = Arc::new(WorldBase::new(self));
        let dir = Vec3::from_lat_lon(lat_deg.to_radians(), lon_deg.to_radians());
        let tile = TileId::from_direction(&TangentWarp, dir, planet_generators::chunk::CHUNK_GRID_LEVEL);
        let (i0, j0) = (i64::from(tile.x()) * per as i64 - i64::from(halo), i64::from(tile.y()) * per as i64 - i64::from(halo));
        let w = per as i64 + 2 * i64::from(halo);
        if i0 < 0 || j0 < 0 || i0 + w > clat.n() || j0 + w > clat.n() {
            return Err("window leaves its face".into());
        }
        let mut dirs = Vec::with_capacity((w * w) as usize);
        for j in 0..w {
            for i in 0..w {
                dirs.push(clat.direction(Cell { face: tile.face().0, i: (i0 + i) as u32, j: (j0 + j) as u32 }));
            }
        }
        let z = base.base_heights(&dirs);
        let mut f = std::fs::File::create(out.join(format!("win_{tag}.bin"))).map_err(|e| e.to_string())?;
        f.write_all(&(per as u32).to_le_bytes()).unwrap();
        f.write_all(&cell_m.to_le_bytes()).unwrap();
        f.write_all(&halo.to_le_bytes()).unwrap();
        f.write_all(&(w as u32).to_le_bytes()).unwrap();
        for v in [i32::from(tile.face().0), i0 as i32, j0 as i32] {
            f.write_all(&v.to_le_bytes()).unwrap();
        }
        for v in &z {
            f.write_all(&(*v as f32).to_le_bytes()).unwrap();
        }
        let (mn, mx) = z.iter().fold((f64::MAX, f64::MIN), |a, &v| (a.0.min(v), a.1.max(v)));
        Ok(format!("window {tag}: lat {lat_deg} lon {lon_deg}, per {per}, cell {cell_m:.3} m, width {w}, face {} origin ({i0},{j0}), height {mn:.0} to {mx:.0} m", tile.face().0))
    }
}
