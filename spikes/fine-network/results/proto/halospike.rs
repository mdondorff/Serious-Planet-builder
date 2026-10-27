//! SPIKE-43 step 2 throwaway (spike branch only, never merged): how far the solved heights in a chunk's own halo differ from the east neighbour's own core heights.
use std::sync::Arc;

use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_generators::anet::{ANet, ANetParams};
use planet_generators::chunkfield::{region, ChunkLattice, ChunkStage};

use crate::anetworld::{cells_per_chunk_for, WorldBase, ANET_CELL_M, ANET_HALO_M};
use crate::coupledworld::{run_cells, COUPLED_STEPS};
use crate::worldfield::WorldTerrain;

impl WorldTerrain {
    pub fn halo_diff(self: &Arc<Self>, lat_deg: f64, lon_deg: f64) -> Result<String, String> {
        let run = crate::coupledworld::load_stored(&self.definition(), self.store_dir(), self.lattice_source(), run_cells(self.res()), COUPLED_STEPS)?.ok_or("no stored coupled run")?;
        let radius = self.radius_m();
        let per = cells_per_chunk_for(radius, ANET_CELL_M) as usize;
        let clat = ChunkLattice::new(per as u32).ok_or("no chunk lattice")?;
        let params = ANetParams::spike(ANET_HALO_M, clat.cell_m(radius));
        let halo = params.halo_rings;
        let stage = ANet { run: run.maps, base: Arc::new(WorldBase::new(self)), radius_m: radius, params, memo: Default::default() };
        let bound = stage.bound_m();
        let dir = Vec3::from_lat_lon(lat_deg.to_radians(), lon_deg.to_radians());
        let tile = TileId::from_direction(&TangentWarp, dir, planet_generators::chunk::CHUNK_GRID_LEVEL);
        let east = TileId::new(tile.face(), tile.level(), tile.x() + 1, tile.y()).ok_or("tile")?;
        let (rc, re) = (region(&clat, tile, halo), region(&clat, east, halo));
        let (sc, se) = (stage.solve(&clat, &rc), stage.solve(&clat, &re));
        let g = |s: &planet_generators::anet::Solution, c: usize| s.z_base[c] + (s.z[c] - s.z_base[c]).clamp(-bound, bound);
        let (i0, j0) = (east.x() as i64 * per as i64, east.y() as i64 * per as i64);
        let bands = [(0i64, 1i64), (1, 10), (10, 50), (50, 150), (150, 376)];
        let mut out = format!("halo {halo} rings, per {per}; centre chunk's halo cells inside the east neighbour's core: |own halo height - neighbour core height| in m\n");
        let mut v: Vec<Vec<f64>> = vec![vec![]; bands.len()];
        let mut base_diff = 0.0f64;
        for c in rc.core..rc.cells.len() {
            let cell = rc.cells[c];
            let (di, dj) = (i64::from(cell.i) - i0, i64::from(cell.j) - j0);
            if cell.face != tile.face().0 || di < 0 || dj < 0 || di >= per as i64 || dj >= per as i64 {
                continue;
            }
            let k = (dj as usize) * per + di as usize;
            let d = (g(&sc, c) - g(&se, k)).abs();
            base_diff = base_diff.max((sc.z_base[c] - se.z_base[k]).abs());
            for (b, &(lo, hi)) in bands.iter().enumerate() {
                if di >= lo && di < hi {
                    v[b].push(d);
                }
            }
        }
        out += &format!("base heights differ by at most {base_diff:.4} m\n");
        for (b, &(lo, hi)) in bands.iter().enumerate() {
            let a = &mut v[b];
            a.sort_by(f64::total_cmp);
            let n = a.len().max(1);
            let over = a.iter().filter(|x| **x > 0.01).count();
            out += &format!(
                "distance {lo} to {} cells from the border: {} cells, mean {:.3}, p50 {:.3}, p99 {:.3}, max {:.3}, over 1 cm: {} ({:.1} %)\n",
                hi - 1,
                a.len(),
                a.iter().sum::<f64>() / n as f64,
                a.get(n / 2).copied().unwrap_or(0.0),
                a.get(n * 99 / 100).copied().unwrap_or(0.0),
                a.last().copied().unwrap_or(0.0),
                over,
                over as f64 / n as f64 * 100.0
            );
        }
        Ok(out)
    }
}
