//! SPIKE-43 step 2 throwaway (spike branch only, never merged): dump the A-net's solved ground (base + clamped change) of the 3x3 chunks round a chunk.
use std::io::Write;
use std::sync::Arc;

use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_generators::anet::{ANet, ANetParams};
use planet_generators::chunkfield::{region, ChunkLattice, ChunkStage};

use crate::anetworld::{cells_per_chunk_for, WorldBase, ANET_CELL_M, ANET_HALO_M};
use crate::coupledworld::{run_cells, COUPLED_STEPS};
use crate::worldfield::WorldTerrain;

impl WorldTerrain {
    /// Writes `<out>/gnd_<tag>.bin`: per (u32), cell_m (f64), width (u32, 3 x per), origin face/i/j (3 x i32), then width^2 solved ground (f32, j outer) and width^2 base (f32).
    pub fn ground_dump(self: &Arc<Self>, lat_deg: f64, lon_deg: f64, out: &std::path::Path, tag: &str) -> Result<String, String> {
        let run = crate::coupledworld::load_stored(&self.definition(), self.store_dir(), self.lattice_source(), run_cells(self.res()), COUPLED_STEPS)?.ok_or("no stored coupled run")?;
        let radius = self.radius_m();
        let per = cells_per_chunk_for(radius, ANET_CELL_M) as usize;
        let clat = ChunkLattice::new(per as u32).ok_or("no chunk lattice")?;
        let cell_m = clat.cell_m(radius);
        let params = ANetParams::spike(ANET_HALO_M, cell_m);
        let halo = params.halo_rings;
        let stage = ANet { run: run.maps, base: Arc::new(WorldBase::new(self)), radius_m: radius, params, memo: Default::default() };
        let bound = stage.bound_m();
        let dir = Vec3::from_lat_lon(lat_deg.to_radians(), lon_deg.to_radians());
        let tile = TileId::from_direction(&TangentWarp, dir, planet_generators::chunk::CHUNK_GRID_LEVEL);
        let w = 3 * per;
        let mut gz = vec![0f32; w * w];
        let mut bz = vec![0f32; w * w];
        let t0 = std::time::Instant::now();
        for dy in 0..3u32 {
            for dx in 0..3u32 {
                let t = TileId::new(tile.face(), tile.level(), tile.x() + dx - 1, tile.y() + dy - 1).ok_or("tile")?;
                let reg = region(&clat, t, halo);
                let s = stage.solve(&clat, &reg);
                for j in 0..per {
                    for i in 0..per {
                        let c = j * per + i;
                        let ch = (s.z[c] - s.z_base[c]).clamp(-bound, bound);
                        let o = (dy as usize * per + j) * w + dx as usize * per + i;
                        gz[o] = (s.z_base[c] + ch) as f32;
                        bz[o] = s.z_base[c] as f32;
                    }
                }
            }
        }
        let i0 = i64::from(tile.x() - 1) * per as i64;
        let j0 = i64::from(tile.y() - 1) * per as i64;
        let mut f = std::fs::File::create(out.join(format!("gnd_{tag}.bin"))).map_err(|e| e.to_string())?;
        f.write_all(&(per as u32).to_le_bytes()).unwrap();
        f.write_all(&cell_m.to_le_bytes()).unwrap();
        f.write_all(&(w as u32).to_le_bytes()).unwrap();
        for v in [i32::from(tile.face().0), i0 as i32, j0 as i32] {
            f.write_all(&v.to_le_bytes()).unwrap();
        }
        for v in gz.iter().chain(bz.iter()) {
            f.write_all(&v.to_le_bytes()).unwrap();
        }
        let dmax = gz.iter().zip(&bz).map(|(a, b)| (a - b).abs()).fold(0f32, f32::max);
        Ok(format!("ground {tag}: 9 chunks in {:.0} s, width {w}, max |change| {dmax:.1} m, bound {bound:.0} m", t0.elapsed().as_secs_f64()))
    }
}
