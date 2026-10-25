//! SPIKE-40 throwaway (spike branch only, never merged): read-only river statistics of the coupled run and of A-net chunks.
use std::sync::Arc;

use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_generators::anet::{ANet, ANetParams};
use planet_generators::chunkfield::{region, ChunkLattice};
use planet_generators::coupled::NONE;

use crate::anetworld::{cells_per_chunk_for, WorldBase, ANET_CELL_M, ANET_HALO_M};
use crate::coupledworld::{run_cells, COUPLED_STEPS};
use crate::worldfield::WorldTerrain;

fn q(v: &mut [f64], p: f64) -> f64 {
    v.sort_by(f64::total_cmp);
    if v.is_empty() {
        f64::NAN
    } else {
        v[((v.len() as f64 - 1.0) * p).round() as usize]
    }
}
/// Slope and intercept of y on x by least squares.
fn fit(xs: &[f64], ys: &[f64]) -> (f64, f64) {
    let n = xs.len() as f64;
    let (mx, my) = (xs.iter().sum::<f64>() / n, ys.iter().sum::<f64>() / n);
    let sxy: f64 = xs.iter().zip(ys).map(|(x, y)| (x - mx) * (y - my)).sum();
    let sxx: f64 = xs.iter().map(|x| (x - mx) * (x - mx)).sum();
    (sxy / sxx, my - sxy / sxx * mx)
}
const OFFS: [(i64, i64); 8] = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)];

impl WorldTerrain {
    pub fn hydro_coupled_report(self: &Arc<Self>) -> Result<String, String> {
        let run = crate::coupledworld::load_stored(&self.definition(), self.store_dir(), self.lattice_source(), run_cells(self.res()), COUPLED_STEPS)?
            .ok_or("no stored coupled run")?;
        let maps = run.maps;
        let lat = maps.lattice();
        let radius = lat.radius_m();
        let rec = maps.receivers();
        let nc = lat.len();
        let land = |c: usize| maps.z(c) > 0.0;
        let area = |c: usize| f64::from(maps.network_area_m2(c));
        let dist = |c: usize, r: usize| radius * lat.direction(c).dot(lat.direction(r)).clamp(-1.0, 1.0).acos();
        let mut out = format!("# coupled run: n {} cells {} cell edge ~{:.0} m\n", lat.n(), nc, radius * std::f64::consts::FRAC_PI_2 / lat.n() as f64);
        let land_area: f64 = (0..nc).filter(|&c| land(c)).map(|c| lat.area_m2(c)).sum();
        out += &format!("land area {:.4e} m2 ({:.1} % of the sphere)\n", land_area, 100.0 * land_area / (4.0 * std::f64::consts::PI * radius * radius));
        for thr in [1e8, 1e9, 1e10] {
            let mut riv: Vec<usize> = (0..nc).filter(|&c| land(c) && area(c) >= thr).collect();
            riv.sort_by(|&a, &b| area(a).total_cmp(&area(b)));
            let mut len = 0.0;
            let mut maxo = vec![0u8; nc];
            let mut cnt = vec![0u8; nc];
            let mut ord = vec![0u8; nc];
            let mut head = vec![false; nc];
            for &c in &riv {
                let o = if maxo[c] == 0 {
                    1
                } else if cnt[c] >= 2 {
                    maxo[c] + 1
                } else {
                    maxo[c]
                };
                ord[c] = o;
                head[c] = maxo[c] != o;
                let r = rec[c];
                if r != NONE {
                    len += dist(c, r as usize);
                    let r = r as usize;
                    if o > maxo[r] {
                        maxo[r] = o;
                        cnt[r] = 1;
                    } else if o == maxo[r] {
                        cnt[r] = cnt[r].saturating_add(1);
                    }
                }
            }
            let kmax = riv.iter().map(|&c| ord[c]).max().unwrap_or(0) as usize;
            let mut n_o = vec![0.0f64; kmax + 1];
            let mut l_o = vec![0.0f64; kmax + 1];
            for &c in &riv {
                let o = ord[c] as usize;
                if head[c] {
                    n_o[o] += 1.0;
                }
                if rec[c] != NONE {
                    l_o[o] += dist(c, rec[c] as usize);
                }
            }
            out += &format!(
                "\n## threshold {:.0e} m2: river cells {}, length {:.4e} m, drainage density {:.4} 1/km, max Strahler order {}\n# order streams mean_length_km\n",
                thr,
                riv.len(),
                len,
                len / 1000.0 / (land_area / 1e6),
                kmax
            );
            let (mut xs, mut yn, mut yl) = (vec![], vec![], vec![]);
            for o in 1..=kmax {
                out += &format!("{o} {} {:.1}\n", n_o[o], l_o[o] / n_o[o].max(1.0) / 1000.0);
                if o < kmax && n_o[o] >= 3.0 {
                    xs.push(o as f64);
                    yn.push(n_o[o].ln());
                    yl.push((l_o[o] / n_o[o]).ln());
                }
            }
            if xs.len() >= 2 {
                out += &format!(
                    "bifurcation ratio Rb {:.2}, length ratio Rl {:.2} (orders 1..{})\n",
                    (-fit(&xs, &yn).0).exp(),
                    fit(&xs, &yl).0.exp(),
                    kmax - 1
                );
            }
        }
        let mut best_donor = vec![NONE; nc];
        for c in 0..nc {
            let r = rec[c];
            if r != NONE && land(c) && area(c) >= 1e8 {
                let r = r as usize;
                if best_donor[r] == NONE || area(c) > area(best_donor[r] as usize) {
                    best_donor[r] = c as u32;
                }
            }
        }
        let mouths: Vec<usize> = (0..nc).filter(|&c| land(c) && area(c) >= 1e10 && (rec[c] == NONE || !land(rec[c] as usize))).collect();
        let (mut lx, mut ly, mut mx, mut my) = (vec![], vec![], vec![], vec![]);
        let mut sin = vec![];
        let mut sin_all = vec![];
        for &m in &mouths {
            let mut path = vec![m];
            while best_donor[*path.last().unwrap()] != NONE {
                path.push(best_donor[*path.last().unwrap()] as usize);
            }
            let mut cum = vec![0.0; path.len()];
            for k in (0..path.len() - 1).rev() {
                cum[k] = cum[k + 1] + dist(path[k + 1], path[k]);
            }
            mx.push(area(m).ln());
            my.push(cum[0].max(1.0).ln());
            if area(m) >= 1e11 {
                for k in 0..path.len() {
                    lx.push(area(path[k]).ln());
                    ly.push(cum[k].max(1.0).ln());
                }
            }
            if path.len() > 12 {
                sin_all.push(cum[0] / dist(path[0], *path.last().unwrap()));
                for k in (0..path.len() - 10).step_by(10) {
                    sin.push((cum[k] - cum[k + 10]) / dist(path[k], path[k + 10]));
                }
            }
        }
        out += &format!("\n## Hack: {} mouths with area >= 1e10 m2\n", mouths.len());
        if mx.len() > 2 {
            let (h, b) = fit(&mx, &my);
            out += &format!("mouths only: L = {:.3} * A_km2^{:.3} (L km); ", (b + h * 1e6f64.ln()).exp() / 1000.0, h);
            let mut rows: Vec<(f64, f64)> = mx.iter().zip(&my).map(|(a, l)| (a.exp(), l.exp())).collect();
            rows.sort_by(|a, b| b.0.total_cmp(&a.0));
            out += &format!(
                "largest basins (A km2, L km): {}\n",
                rows.iter().take(5).map(|(a, l)| format!("({:.2e}, {:.0})", a / 1e6, l / 1000.0)).collect::<Vec<_>>().join(" ")
            );
        }
        if lx.len() > 5 {
            let (h, b) = fit(&lx, &ly);
            out += &format!("all main-stem cells of basins >= 1e11 m2: L = {:.3} * A_km2^{:.3} (L km, {} points)\n", (b + h * 1e6f64.ln()).exp() / 1000.0, h, lx.len());
        }
        out += &format!(
            "main-stem sinuosity (D8 path / chord, 10-cell windows): p10 {:.2} p50 {:.2} p90 {:.2}; whole stem p50 {:.2}\n",
            q(&mut sin, 0.1),
            q(&mut sin, 0.5),
            q(&mut sin, 0.9),
            q(&mut sin_all, 0.5)
        );
        let mut bins: std::collections::BTreeMap<i32, Vec<f64>> = Default::default();
        for c in 0..nc {
            let r = rec[c];
            if r == NONE || !land(c) || area(c) < 1e9 {
                continue;
            }
            let d = dist(c, r as usize);
            let s = f64::from(maps.z(c) - maps.z(r as usize)) / d;
            if s > 0.0 {
                bins.entry((area(c).log10() * 4.0).floor() as i32).or_default().push(s);
            }
        }
        out += "\n## slope against area, river cells >= 1e9 m2 (coupled run heights)\n# log10_area median_slope p10 p90 cells\n";
        let (mut xs, mut ys) = (vec![], vec![]);
        for (k, v) in bins.iter_mut() {
            let la = f64::from(*k) / 4.0 + 0.125;
            let m = q(v, 0.5);
            out += &format!("{la:.3} {m:.5} {:.5} {:.5} {}\n", q(v, 0.1), q(v, 0.9), v.len());
            if v.len() >= 30 {
                xs.push(la * 10f64.ln());
                ys.push(m.ln());
            }
        }
        if xs.len() > 2 {
            out += &format!("concavity theta (S ~ A^-theta, bin medians): {:.3}\n", -fit(&xs, &ys).0);
        }
        Ok(out)
    }

    pub fn hydro_chunk_report(self: &Arc<Self>, lat_deg: f64, lon_deg: f64) -> Result<String, String> {
        let run = crate::coupledworld::load_stored(&self.definition(), self.store_dir(), self.lattice_source(), run_cells(self.res()), COUPLED_STEPS)?
            .ok_or("no stored coupled run")?;
        let radius = self.radius_m();
        let per = cells_per_chunk_for(radius, ANET_CELL_M) as usize;
        let clat = ChunkLattice::new(per as u32).ok_or("no chunk lattice")?;
        let cell_m = clat.cell_m(radius);
        let params = ANetParams::spike(ANET_HALO_M, cell_m);
        let halo = params.halo_rings;
        let stage = ANet { run: run.maps, base: Arc::new(WorldBase::new(self)), radius_m: radius, params };
        let dir = Vec3::from_lat_lon(lat_deg.to_radians(), lon_deg.to_radians());
        let tile = TileId::from_direction(&TangentWarp, dir, planet_generators::chunk::CHUNK_GRID_LEVEL);
        let reg = region(&clat, tile, halo);
        let s = stage.solve(&clat, &reg);
        let core = reg.core;
        let ncell = s.z.len();
        let mut out = format!("# chunk at lat {lat_deg} lon {lon_deg}: tile {tile:?}, {per}x{per} core cells of {cell_m:.2} m, halo {halo} rings\n");
        let core_area = core as f64 * cell_m * cell_m;
        let valid = |r: u32| (r as usize) < ncell;
        let step = |c: usize, r: usize| {
            let k = reg.neighbours[c].iter().position(|&n| n as usize == r).unwrap_or(0);
            if k < 4 {
                cell_m
            } else {
                cell_m * std::f64::consts::SQRT_2
            }
        };
        for thr in [1e6, 1e7, 1e8, 1e9] {
            let mut len = 0.0;
            let mut n = 0;
            for c in 0..core {
                if s.area_m2[c] >= thr && valid(s.receiver[c]) {
                    len += step(c, s.receiver[c] as usize);
                    n += 1;
                }
            }
            out += &format!("channel cells with area >= {thr:.0e} m2 in core: {n}, length {:.0} m, density {:.3} 1/km\n", len, len / 1000.0 / (core_area / 1e6));
        }
        out += "\n## cut depth (z_base - z, m) by area (core cells)\n# log10_area cells p10 p50 p90 law_GEN057\n";
        let mut bins: std::collections::BTreeMap<i32, Vec<f64>> = Default::default();
        for c in 0..core {
            if s.area_m2[c] >= 1e5 {
                bins.entry((s.area_m2[c].log10() * 2.0).floor() as i32).or_default().push(s.z_base[c] - s.z[c]);
            }
        }
        for (k, v) in bins.iter_mut() {
            let la = f64::from(*k) / 2.0 + 0.25;
            out += &format!("{la:.2} {} {:.1} {:.1} {:.1} {:.1}\n", v.len(), q(v, 0.1), q(v, 0.5), q(v, 0.9), (3.0 * (10f64.powf(la) / 1e8).sqrt()).min(100.0));
        }
        let mut sb: std::collections::BTreeMap<i32, Vec<f64>> = Default::default();
        for c in 0..core {
            if s.area_m2[c] < 1e6 || !valid(s.receiver[c]) {
                continue;
            }
            let r = s.receiver[c] as usize;
            let sl = (s.z[c] - s.z[r]) / step(c, r);
            if sl > 0.0 {
                sb.entry((s.area_m2[c].log10() * 2.0).floor() as i32).or_default().push(sl);
            }
        }
        out += "\n## channel slope (solved z) by area\n# log10_area cells median p10 p90\n";
        let (mut xs, mut ys) = (vec![], vec![]);
        for (k, v) in sb.iter_mut() {
            let la = f64::from(*k) / 2.0 + 0.25;
            let m = q(v, 0.5);
            out += &format!("{la:.2} {} {:.4} {:.4} {:.4}\n", v.len(), m, q(v, 0.1), q(v, 0.9));
            if v.len() >= 50 {
                xs.push(la * 10f64.ln());
                ys.push(m.ln());
            }
        }
        if xs.len() > 2 {
            out += &format!("concavity theta (bin medians): {:.3}\n", -fit(&xs, &ys).0);
        }
        out += "\n## valley transects: river cells every 5th, +-600 m; rim = lower of the two sides' maxima; width at half the depth, corrected to the perpendicular\n# log10_area transects rim_depth_p50 p10 p90 width_half_p50 p10 p90\n";
        let reach = (600.0 / cell_m) as i64;
        let mut tb: std::collections::BTreeMap<i32, Vec<(f64, f64)>> = Default::default();
        let mut idx = 0usize;
        for c in 0..core {
            if s.area_m2[c] < 1e7 || !valid(s.receiver[c]) || (s.receiver[c] as usize) >= core {
                continue;
            }
            idx += 1;
            if idx % 5 != 0 {
                continue;
            }
            let (i, j) = ((c % per) as i64, (c / per) as i64);
            let r = s.receiver[c] as usize;
            let k = reg.neighbours[c].iter().position(|&n| n as usize == r).unwrap_or(0);
            let (di, dj) = OFFS[k];
            let norm = ((di * di + dj * dj) as f64).sqrt();
            let (fi, fj) = (di as f64 / norm, dj as f64 / norm);
            let (ei, ej) = if fj.abs() > fi.abs() { (1i64, 0i64) } else { (0, 1) };
            let dotv = fi * ei as f64 + fj * ej as f64;
            let perp = (1.0 - dotv * dotv).sqrt();
            if i - reach * ei < 0 || j - reach * ej < 0 || i + reach * ei >= per as i64 || j + reach * ej >= per as i64 {
                continue;
            }
            let z = |t: i64| s.z[((j + t * ej) as usize) * per + (i + t * ei) as usize];
            let bed = (-3..=3).map(z).fold(f64::INFINITY, f64::min);
            let lmax = (-reach..0).map(z).fold(f64::NEG_INFINITY, f64::max);
            let rmax = (1..=reach).map(z).fold(f64::NEG_INFINITY, f64::max);
            let depth = lmax.min(rmax) - bed;
            if depth <= 0.0 {
                continue;
            }
            let half = bed + depth / 2.0;
            let (mut l, mut rr) = (0, 0);
            while l < reach && z(-l - 1) <= half {
                l += 1;
            }
            while rr < reach && z(rr + 1) <= half {
                rr += 1;
            }
            tb.entry((s.area_m2[c].log10() * 2.0).floor() as i32).or_default().push((depth, (l + rr + 1) as f64 * cell_m * perp));
        }
        for (k, v) in tb.iter() {
            let la = f64::from(*k) / 2.0 + 0.25;
            let mut d: Vec<f64> = v.iter().map(|x| x.0).collect();
            let mut w: Vec<f64> = v.iter().map(|x| x.1).collect();
            out += &format!("{la:.2} {} {:.1} {:.1} {:.1} {:.0} {:.0} {:.0}\n", v.len(), q(&mut d, 0.5), q(&mut d, 0.1), q(&mut d, 0.9), q(&mut w, 0.5), q(&mut w, 0.1), q(&mut w, 0.9));
        }
        let mut hist = String::from("
## core cells by area (m2): ");
        for t in [1e3, 1e4, 3e4, 1e5, 3e5, 1e6, 1e8, 1e10] {
            hist += &format!(">={t:.0e}: {}  ", (0..core).filter(|&c| s.area_m2[c] >= t).count());
        }
        out += &hist;
        out += "
";
        // Upstream walk of the largest channel: donors inside the core, the largest-area one first.
        let mut donor = vec![u32::MAX; core];
        for c in 0..core {
            let r = s.receiver[c];
            if valid(r) && (r as usize) < core {
                let r = r as usize;
                if donor[r] == u32::MAX || s.area_m2[c] > s.area_m2[donor[r] as usize] {
                    donor[r] = c as u32;
                }
            }
        }
        let top = (0..core).max_by(|&a, &b| s.area_m2[a].total_cmp(&s.area_m2[b])).unwrap();
        let mut path = vec![top];
        while donor[*path.last().unwrap()] != u32::MAX && path.len() < 100_000 {
            path.push(donor[*path.last().unwrap()] as usize);
        }
        path.reverse(); // head to mouth
        let pos = |c: usize| ((c % per) as f64 * cell_m, (c / per) as f64 * cell_m);
        let mut cum = vec![0.0; path.len()];
        for k in 1..path.len() {
            cum[k] = cum[k - 1] + step(path[k], path[k - 1]);
        }
        for win in [125usize, 500, 1250] {
            let mut v = vec![];
            for k in (0..path.len().saturating_sub(win)).step_by((win / 2).max(1)) {
                let (a, b) = (pos(path[k]), pos(path[k + win]));
                v.push((cum[k + win] - cum[k]) / (a.0 - b.0).hypot(a.1 - b.1));
            }
            out += &format!(
                "sinuosity of the largest channel (area {:.2e} to {:.2e} m2, {} cells, {:.0} m), windows of {:.0} m: p10 {:.2} p50 {:.2} p90 {:.2} ({} windows)
",
                s.area_m2[path[0]],
                s.area_m2[top],
                path.len(),
                cum[path.len() - 1],
                win as f64 * cell_m,
                q(&mut v, 0.1),
                q(&mut v, 0.5),
                q(&mut v, 0.9),
                v.len()
            );
        }
        out += "
## bed profile of the largest channel (head to mouth), every 50 cells: dist_m area_m2 z_bed z_base cut slope_over_400m
";
        for k in (0..path.len()).step_by(50) {
            let k2 = (k + 50).min(path.len() - 1);
            let sl = if k2 > k { (s.z[path[k]] - s.z[path[k2]]) / (cum[k2] - cum[k]) } else { f64::NAN };
            out += &format!("{:.0} {:.2e} {:.1} {:.1} {:.1} {:.4}
", cum[k], s.area_m2[path[k]], s.z[path[k]], s.z_base[path[k]], s.z_base[path[k]] - s.z[path[k]], sl);
        }
        let (mut sl_all, mut up) = (vec![], 0usize);
        for k in 0..path.len().saturating_sub(50) {
            let sl = (s.z[path[k]] - s.z[path[k + 50]]) / (cum[k + 50] - cum[k]);
            if sl < 0.0 { up += 1 }
            sl_all.push(sl);
        }
        out += &format!("slope over 400 m windows along the channel: p10 {:.4} p50 {:.4} p90 {:.4}, windows rising downstream {} of {}
", q(&mut sl_all, 0.1), q(&mut sl_all, 0.5), q(&mut sl_all, 0.9), up, sl_all.len());
        Ok(out)
    }
}
