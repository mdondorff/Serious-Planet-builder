//! SPIKE-42 throwaway (spike branch only, never merged): dump the inputs of a chunk's river routing (core base heights, corridor, areas, today's beds).
use std::io::Write;
use std::sync::Arc;

use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_generators::anet::{ANetParams, Network, NO_NEXT};
use planet_generators::chunkfield::{region, ChunkLattice};

use crate::anetworld::{cells_per_chunk_for, WorldBase, ANET_CELL_M, ANET_HALO_M};
use crate::coupledworld::{run_cells, COUPLED_STEPS};
use crate::worldfield::WorldTerrain;

fn put_f64(w: &mut impl Write, v: &[f64]) {
    for x in v {
        w.write_all(&x.to_le_bytes()).unwrap();
    }
}
fn put_i32(w: &mut impl Write, v: &[i32]) {
    for x in v {
        w.write_all(&x.to_le_bytes()).unwrap();
    }
}

impl WorldTerrain {
    /// Writes `<out>/chunk_<tag>.bin`: per (u32), cell_m (f64), origin face/i/j (3 x i32), then per^2 each of base_z, anchor_z, river_area (f64), corridor (i32),
    /// next (i32: core index, -1 none, -2 outside the core), then n_exit (i32) and n_exit x (core index, face, i, j of the next cell, all i32),
    /// n_entry (i32) and n_entry x (core index, face, i, j of the upstream cell outside the core). Returns a text line.
    pub fn route_dump(self: &Arc<Self>, lat_deg: f64, lon_deg: f64, out: &std::path::Path, tag: &str) -> Result<String, String> {
        let run = crate::coupledworld::load_stored(&self.definition(), self.store_dir(), self.lattice_source(), run_cells(self.res()), COUPLED_STEPS)?
            .ok_or("no stored coupled run")?;
        let radius = self.radius_m();
        let per = cells_per_chunk_for(radius, ANET_CELL_M) as usize;
        let clat = ChunkLattice::new(per as u32).ok_or("no chunk lattice")?;
        let cell_m = clat.cell_m(radius);
        let params = ANetParams::spike(ANET_HALO_M, cell_m);
        let halo = params.halo_rings;
        let maps = run.maps;
        let base = Arc::new(WorldBase::new(self));
        let dir = Vec3::from_lat_lon(lat_deg.to_radians(), lon_deg.to_radians());
        let tile = TileId::from_direction(&TangentWarp, dir, planet_generators::chunk::CHUNK_GRID_LEVEL);
        let reg = region(&clat, tile, halo);
        let net = Network::build_with(&*maps, &*base, &clat, &reg, radius, &params.anchor);
        let core = reg.core;
        let c0 = reg.cells[0];
        let mut f = std::fs::File::create(out.join(format!("chunk_{tag}.bin"))).map_err(|e| e.to_string())?;
        f.write_all(&(per as u32).to_le_bytes()).unwrap();
        f.write_all(&cell_m.to_le_bytes()).unwrap();
        put_i32(&mut f, &[i32::from(c0.face), c0.i as i32, c0.j as i32]);
        put_f64(&mut f, &net.base_z[..core]);
        put_f64(&mut f, &net.anchor_z[..core]);
        put_f64(&mut f, &net.river_area_m2[..core]);
        put_i32(&mut f, &net.corridor[..core].iter().map(|&b| i32::from(b)).collect::<Vec<_>>());
        let next: Vec<i32> = (0..core).map(|c| if !net.corridor[c] || net.next[c] == NO_NEXT { -1 } else if (net.next[c] as usize) < core { net.next[c] as i32 } else { -2 }).collect();
        put_i32(&mut f, &next);
        let exits: Vec<i32> = (0..core)
            .filter(|&c| next[c] == -2)
            .flat_map(|c| {
                let n = reg.cells[net.next[c] as usize];
                [c as i32, i32::from(n.face), n.i as i32, n.j as i32]
            })
            .collect();
        put_i32(&mut f, &[(exits.len() / 4) as i32]);
        put_i32(&mut f, &exits);
        let mut entries: Vec<i32> = vec![];
        for c in core..reg.cells.len() {
            if net.corridor[c] && net.next[c] != NO_NEXT && (net.next[c] as usize) < core {
                let u = reg.cells[c];
                entries.extend([net.next[c] as i32, i32::from(u.face), u.i as i32, u.j as i32]);
            }
        }
        put_i32(&mut f, &[(entries.len() / 4) as i32]);
        put_i32(&mut f, &entries);
        // The ring of base heights just outside the core (the halo rows the symmetric gate rule reads): west (i0-1), east (i0+per), north (j0-1), south (j0+per), each `per` values along the edge.
        let ring = |di: i64, dj: i64, along_i: bool| -> Vec<f64> {
            (0..per as i64)
                .map(|t| {
                    let (i, j) = if along_i { (c0.i as i64 + t, c0.j as i64 + dj) } else { (c0.i as i64 + di, c0.j as i64 + t) };
                    let cell = planet_generators::chunkfield::Cell { face: c0.face, i: i as u32, j: j as u32 };
                    reg.index_of(cell).map_or(f64::NAN, |x| net.base_z[x])
                })
                .collect()
        };
        put_f64(&mut f, &ring(-1, 0, false));
        put_f64(&mut f, &ring(per as i64, 0, false));
        put_f64(&mut f, &ring(0, -1, true));
        put_f64(&mut f, &ring(0, per as i64, true));
        let ncorr = net.corridor[..core].iter().filter(|b| **b).count();
        let mut line = format!(
            "chunk {tag} lat {lat_deg} lon {lon_deg}: origin face {} i {} j {}, corridor cells in core {ncorr}, exits {}, entries {}",
            c0.face,
            c0.i,
            c0.j,
            exits.len() / 4,
            entries.len() / 4
        );
        for e in exits.chunks(4) {
            let d = clat_dir(&clat, e[1] as u8, e[2] as u32, e[3] as u32, radius);
            line += &format!("\n  exit core cell {} -> face {} i {} j {} at lat {:.5} lon {:.5}", e[0], e[1], e[2], e[3], d.0, d.1);
        }
        for e in entries.chunks(4) {
            let d = clat_dir(&clat, e[1] as u8, e[2] as u32, e[3] as u32, radius);
            line += &format!("\n  entry core cell {} <- face {} i {} j {} at lat {:.5} lon {:.5}", e[0], e[1], e[2], e[3], d.0, d.1);
        }
        Ok(line)
    }
}

fn clat_dir(lat: &ChunkLattice, face: u8, i: u32, j: u32, _radius: f64) -> (f64, f64) {
    let d = lat.direction(planet_generators::chunkfield::Cell { face, i, j });
    let (la, lo) = d.lat_lon();
    (la.to_degrees(), lo.to_degrees())
}

impl WorldTerrain {
    /// River cells of the coarse run with area in [1e10, 1e12] m2 by the run's height range over the cell and its eight neighbours (steepest first).
    pub fn route_candidates(self: &Arc<Self>) -> Result<String, String> {
        let run = crate::coupledworld::load_stored(&self.definition(), self.store_dir(), self.lattice_source(), run_cells(self.res()), COUPLED_STEPS)?
            .ok_or("no stored coupled run")?;
        let maps = run.maps;
        let lat = maps.lattice();
        let rec = maps.receivers();
        let mut v: Vec<(f64, usize)> = vec![];
        for c in 0..lat.len() {
            let a = f64::from(maps.network_area_m2(c));
            if maps.z(c) > 700.0 && rec[c] != planet_generators::coupled::NONE && (1e9..1e12).contains(&a) {
                let (mut lo, mut hi) = (f64::from(maps.z(c)), f64::from(maps.z(c)));
                for n in lat.neighbours(c) {
                    let z = f64::from(maps.z(n as usize));
                    lo = lo.min(z);
                    hi = hi.max(z);
                }
                v.push((hi - lo, c));
            }
        }
        v.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut out = String::new();
        for (r, c) in v.iter().take(12) {
            let (la, lo) = lat.direction(*c).lat_lon();
            out += &format!("relief {:.0} m z {:.0} area {:.2e} lat {:.4} lon {:.4}\n", r, maps.z(*c), maps.network_area_m2(*c), la.to_degrees(), lo.to_degrees());
        }
        Ok(out)
    }
}
