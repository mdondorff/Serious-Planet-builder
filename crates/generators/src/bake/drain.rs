//! SPIKE-138 (spikes #143; spike branch spike/drain-aplus only, never merged): a surface made to drain by a least-cost breach, or a fill where the fill moves less.
//!
//! On a `w x h` grid of 8 neighbours with outlet cells:
//! 1. `level` = the priority-flood fill of `z` from the outlets (no epsilon). A depression is an 8-connected component of cells with `level > z + TOL` on one level; its floor is
//!    its lowest cell (ties by the cell index).
//! 2. A depression holding an intended cell (`keep`) stays.
//! 3. Fill: every cell of the depression raised to its level; its volume `sum (level - z) a`.
//! 4. Breach: the least-cost path from the floor (Dijkstra, the cost of a cell `max(0, z - z_floor)`, ties by the cell index) to the first cell outside the depression whose fill
//!    level lies below the floor (or an outlet); the path lowered to `min(z, z_floor - DELTA k)` at its k-th cell; its volume `sum (z - z') a`.
//! 5. The smaller volume wins (ties: the breach). Fills apply first, then breaches; the whole repeats on the result until no depression is left or `max_rounds`.
//!
//! Sequential and deterministic: the result depends on the heights and the order of the cell indices only.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// spikes #147 (spike only): the breach path's cost per step. `false` (SPIKE-138): `max(0, z - z_floor)` per cell, whatever the step's length, so a diagonal step covers
/// sqrt 2 of the distance at the price of one cell and paths across even ground run diagonal. `true`: that cost times the step's length (1 or sqrt 2), plus
/// `LENGTH_COST_M` m per unit length, so a path on even ground is the shortest one in the octile metric.
pub static BREACH_BY_LENGTH: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// The cost per unit of path length of [`BREACH_BY_LENGTH`] (m).
pub const LENGTH_COST_M: f64 = 0.01;

/// A depression is a fill over this depth (m).
pub const TOL: f64 = 1e-3;
/// The descent per cell of a breach path (m).
pub const DELTA: f64 = 1e-4;

#[derive(Clone, Copy, Debug, Default)]
pub struct DrainStats {
    pub rounds: usize,
    /// Depressions met over all rounds; filled, breached, kept (intended).
    pub depressions: usize,
    pub filled: usize,
    pub breached: usize,
    pub kept: usize,
    /// Depressions left after the last round (not intended).
    pub left: usize,
    /// Depressions of the last round neither breached nor filled (both over their caps; [`drain_selective`]).
    pub deep: usize,
}

#[derive(PartialEq)]
struct Key(f64, u32);
impl Eq for Key {}
impl PartialOrd for Key {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Key {
    // a min-heap on (value, index)
    fn cmp(&self, o: &Self) -> Ordering {
        o.0.total_cmp(&self.0).then(o.1.cmp(&self.1))
    }
}

const D8: [(i64, i64); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];

fn neighbours(w: usize, h: usize, p: usize) -> impl Iterator<Item = usize> {
    let (x, y) = ((p % w) as i64, (p / w) as i64);
    D8.iter().filter_map(move |&(dx, dy)| {
        let (a, b) = (x + dx, y + dy);
        (a >= 0 && b >= 0 && (a as usize) < w && (b as usize) < h).then(|| b as usize * w + a as usize)
    })
}

/// The priority-flood fill level of `z` from the outlets (no epsilon); a cell no outlet reaches keeps +inf.
pub fn fill_level(z: &[f64], w: usize, h: usize, outlet: &[bool]) -> Vec<f64> {
    let n = w * h;
    let mut level = vec![f64::INFINITY; n];
    let mut heap = BinaryHeap::new();
    for p in (0..n).filter(|&p| outlet[p]) {
        level[p] = z[p];
        heap.push(Key(z[p], p as u32));
    }
    while let Some(Key(l, c)) = heap.pop() {
        let c = c as usize;
        if l > level[c] {
            continue;
        }
        for q in neighbours(w, h, c) {
            let lq = z[q].max(l);
            if lq < level[q] {
                level[q] = lq;
                heap.push(Key(lq, q as u32));
            }
        }
    }
    level
}

/// The depressions of `z`: (label per cell, `u32::MAX` outside; per depression its cells and floor cell). Cells of one depression share one fill level.
pub fn depressions(z: &[f64], level: &[f64], w: usize, h: usize) -> (Vec<u32>, Vec<(Vec<u32>, usize)>) {
    let n = w * h;
    let mut label = vec![u32::MAX; n];
    let mut comps = Vec::new();
    let mut stack = Vec::new();
    for s in 0..n {
        if label[s] != u32::MAX || !(level[s] > z[s] + TOL) || !level[s].is_finite() {
            continue;
        }
        let id = comps.len() as u32;
        label[s] = id;
        stack.push(s);
        let mut cells = Vec::new();
        let mut floor = s;
        while let Some(c) = stack.pop() {
            cells.push(c as u32);
            if z[c] < z[floor] || (z[c] == z[floor] && c < floor) {
                floor = c;
            }
            for q in neighbours(w, h, c) {
                if label[q] == u32::MAX && level[q] > z[q] + TOL && level[q] == level[s] {
                    label[q] = id;
                    stack.push(q);
                }
            }
        }
        cells.sort_unstable();
        comps.push((cells, floor));
    }
    (label, comps)
}

/// Drains `z` (see the module). `keep(p)`: an intended cell; `area(p)`: the cell's area (m2); `search`: the most cells a breach search may settle beyond 4 times the depression's
/// size (beyond: filled). Returns the drained surface and the counts.
#[allow(clippy::too_many_arguments)]
pub fn drain(
    z0: &[f64],
    w: usize,
    h: usize,
    outlet: &[bool],
    keep: &dyn Fn(usize) -> bool,
    area: &dyn Fn(usize) -> f64,
    max_rounds: usize,
    search: usize,
) -> (Vec<f64>, DrainStats) {
    drain_capped(z0, w, h, outlet, keep, area, max_rounds, search, f64::INFINITY)
}

/// [`drain`] where a breach whose deepest cut exceeds `max_cut` m is not taken (the depression is filled).
#[allow(clippy::too_many_arguments)]
pub fn drain_capped(
    z0: &[f64],
    w: usize,
    h: usize,
    outlet: &[bool],
    keep: &dyn Fn(usize) -> bool,
    area: &dyn Fn(usize) -> f64,
    max_rounds: usize,
    search: usize,
    max_cut: f64,
) -> (Vec<f64>, DrainStats) {
    drain_selective(z0, w, h, outlet, keep, area, max_rounds, search, &|_| max_cut, &|_| f64::INFINITY)
}

/// SPIKE-138 (spikes #145): [`drain`] taking a breach only when its deepest cut is within `cut_cap(floor)` and a fill only when its depth (level - floor) is within
/// `fill_cap(floor)`; both allowed: the smaller volume; neither: the depression stays (`DrainStats::deep`, for K5).
#[allow(clippy::too_many_arguments)]
pub fn drain_selective(
    z0: &[f64],
    w: usize,
    h: usize,
    outlet: &[bool],
    keep: &dyn Fn(usize) -> bool,
    area: &dyn Fn(usize) -> f64,
    max_rounds: usize,
    search: usize,
    cut_cap: &dyn Fn(usize) -> f64,
    fill_cap: &dyn Fn(usize) -> f64,
) -> (Vec<f64>, DrainStats) {
    let (z, st, _) = drain_selective_report(z0, w, h, outlet, keep, area, max_rounds, search, cut_cap, fill_cap);
    (z, st)
}

/// A depression [`drain_selective`] left: its floor cell, its cells, its depth (fill level - floor) and its least breach cut (+inf when no path was found).
#[derive(Clone, Copy, Debug)]
pub struct Deep {
    pub floor: usize,
    pub cells: usize,
    pub depth: f64,
    pub cut: f64,
}

/// [`drain_selective`] that also returns the depressions of its last round neither breached nor filled (spikes #147: the census per stage).
#[allow(clippy::too_many_arguments)]
pub fn drain_selective_report(
    z0: &[f64],
    w: usize,
    h: usize,
    outlet: &[bool],
    keep: &dyn Fn(usize) -> bool,
    area: &dyn Fn(usize) -> f64,
    max_rounds: usize,
    search: usize,
    cut_cap: &dyn Fn(usize) -> f64,
    fill_cap: &dyn Fn(usize) -> f64,
) -> (Vec<f64>, DrainStats, Vec<Deep>) {
    let n = w * h;
    let mut z = z0.to_vec();
    let mut st = DrainStats::default();
    let mut deeps: Vec<Deep> = Vec::new();
    let mut dist = vec![f64::INFINITY; n];
    let mut parent = vec![u32::MAX; n];
    let mut seen: Vec<usize> = Vec::new();
    for round in 0..max_rounds {
        let level = fill_level(&z, w, h, outlet);
        let (label, comps) = depressions(&z, &level, w, h);
        let active: Vec<usize> = (0..comps.len()).filter(|&k| !comps[k].0.iter().any(|&c| keep(c as usize))).collect();
        st.rounds = round + 1;
        if active.is_empty() {
            st.kept = comps.len();
            st.left = 0;
            return (z, st, Vec::new());
        }
        st.depressions += active.len();
        let mut fills: Vec<usize> = Vec::new();
        let mut cuts: Vec<(usize, f64)> = Vec::new();
        let mut deep = 0usize;
        deeps.clear();
        for &k in &active {
            let (cells, floor) = &comps[k];
            let zf = z[*floor];
            let lv = level[cells[0] as usize];
            let fill_v: f64 = cells.iter().map(|&c| (lv - z[c as usize]) * area(c as usize)).sum();
            // the least-cost breach path from the floor
            let mut heap = BinaryHeap::new();
            dist[*floor] = 0.0;
            parent[*floor] = u32::MAX;
            seen.push(*floor);
            heap.push(Key(0.0, *floor as u32));
            let limit = 4 * cells.len() + search;
            let mut settled = 0usize;
            let mut end = None;
            while let Some(Key(d, c)) = heap.pop() {
                let c = c as usize;
                if d > dist[c] {
                    continue;
                }
                if c != *floor && label[c] != k as u32 && (outlet[c] || level[c] < zf - TOL) {
                    end = Some(c);
                    break;
                }
                settled += 1;
                if settled > limit {
                    break;
                }
                let by_length = BREACH_BY_LENGTH.load(std::sync::atomic::Ordering::Relaxed);
                for q in neighbours(w, h, c) {
                    let nd = if by_length {
                        let len = if q % w != c % w && q / w != c / w { std::f64::consts::SQRT_2 } else { 1.0 };
                        d + len * ((z[q] - zf).max(0.0) + LENGTH_COST_M)
                    } else {
                        d + (z[q] - zf).max(0.0)
                    };
                    if nd < dist[q] {
                        if dist[q].is_infinite() {
                            seen.push(q);
                        }
                        dist[q] = nd;
                        parent[q] = c as u32;
                        heap.push(Key(nd, q as u32));
                    }
                }
            }
            let mut path = Vec::new();
            if let Some(e) = end {
                let mut c = e;
                path.push(c);
                while parent[c] != u32::MAX {
                    c = parent[c] as usize;
                    path.push(c);
                }
                path.reverse();
            }
            for &s in &seen {
                dist[s] = f64::INFINITY;
                parent[s] = u32::MAX;
            }
            seen.clear();
            let fill_ok = lv - zf <= fill_cap(*floor);
            let (breach_ok, breach_v, cut) = if path.is_empty() {
                (false, f64::INFINITY, f64::INFINITY)
            } else {
                let v: f64 = path.iter().enumerate().map(|(i, &c)| (z[c] - z[c].min(zf - DELTA * i as f64)).max(0.0) * area(c)).sum();
                let cut: f64 = path.iter().enumerate().map(|(i, &c)| z[c] - z[c].min(zf - DELTA * i as f64)).fold(0.0, f64::max);
                (cut <= cut_cap(*floor), v, cut)
            };
            if breach_ok && (!fill_ok || breach_v <= fill_v) {
                for (i, &c) in path.iter().enumerate() {
                    cuts.push((c, zf - DELTA * i as f64));
                }
                st.breached += 1;
            } else if fill_ok {
                fills.push(k);
            } else {
                deep += 1;
                deeps.push(Deep { floor: *floor, cells: cells.len(), depth: lv - zf, cut });
            }
        }
        st.deep = deep;
        if fills.is_empty() && cuts.is_empty() {
            break;
        }
        st.filled += fills.len();
        for &k in &fills {
            let lv = level[comps[k].0[0] as usize];
            for &c in &comps[k].0 {
                z[c as usize] = z[c as usize].max(lv);
            }
        }
        for &(c, v) in &cuts {
            z[c] = z[c].min(v);
        }
        let _ = n;
    }
    let level = fill_level(&z, w, h, outlet);
    let (_, comps) = depressions(&z, &level, w, h);
    let intended = comps.iter().filter(|(cells, _)| cells.iter().any(|&c| keep(c as usize))).count();
    st.kept = intended;
    st.left = comps.len() - intended;
    (z, st, deeps)
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: SPIKE-138 (spike only)
    #[test]
    fn a_narrow_rim_is_breached_a_wide_shallow_pit_filled() {
        // a 9 x 3 strip, outlet at x = 0: a deep pit behind a one-cell rim
        let (w, h) = (9usize, 3usize);
        let mut z = vec![10.0f64; w * h];
        for y in 0..h {
            z[y * w] = 0.0;
            for x in 1..w {
                z[y * w + x] = 1.0 + 0.1 * x as f64;
            }
            z[y * w + 4] = 5.0; // the rim
            for x in 5..w {
                z[y * w + x] = 0.5; // the pit, deep below the rim
            }
        }
        let outlet: Vec<bool> = (0..w * h).map(|p| p % w == 0).collect();
        let (d, st) = drain(&z, w, h, &outlet, &|_| false, &|_| 1.0, 10, 1000);
        assert_eq!(st.left, 0);
        assert!(st.breached >= 1, "{st:?}");
        // the pit floor is not raised; the rim is cut on one row only
        assert!((5..w).all(|x| d[w + x] <= 0.5 + 1e-9));
        let level = fill_level(&d, w, h, &outlet);
        assert!((0..w * h).all(|p| level[p] <= d[p] + TOL));
    }

    // spec: SPIKE-138 (spike only)
    #[test]
    fn a_deep_pit_behind_a_high_rim_stays_under_both_caps() {
        let (w, h) = (9usize, 3usize);
        let mut z = vec![0.0f64; w * h];
        for y in 0..h {
            for x in 1..w {
                z[y * w + x] = 1.0 + 0.1 * x as f64;
            }
            z[y * w + 4] = 200.0;
            for x in 5..w {
                z[y * w + x] = 0.5;
            }
        }
        let outlet: Vec<bool> = (0..w * h).map(|p| p % w == 0).collect();
        let (d, st) = drain_selective(&z, w, h, &outlet, &|_| false, &|_| 1.0, 10, 1000, &|_| 50.0, &|_| 50.0);
        assert_eq!(d, z);
        assert_eq!(st.deep, 1);
        // a 20 m rim: both within the caps, the smaller volume (the breach) wins
        for y in 0..h {
            z[y * w + 4] = 20.0;
        }
        let (_, st) = drain_selective(&z, w, h, &outlet, &|_| false, &|_| 1.0, 10, 1000, &|_| 50.0, &|_| 50.0);
        assert_eq!((st.deep, st.left), (0, 0));
        assert!(st.breached >= 1);
    }

    // spec: SPIKE-138 (spike only)
    #[test]
    fn an_intended_depression_stays() {
        let (w, h) = (5usize, 5usize);
        let mut z = vec![10.0f64; w * h];
        for p in 0..w * h {
            if p % w == 0 || p % w == w - 1 || p / w == 0 || p / w == h - 1 {
                z[p] = 0.0;
            }
        }
        for y in 1..4 {
            for x in 1..4 {
                z[y * w + x] = 5.0;
            }
        }
        z[2 * w + 2] = 1.0;
        let outlet: Vec<bool> = (0..w * h).map(|p| z[p] == 0.0).collect();
        let (d, st) = drain(&z, w, h, &outlet, &|p| p == 2 * w + 2, &|_| 1.0, 10, 1000);
        assert_eq!(d, z);
        assert_eq!((st.kept, st.left), (1, 0));
    }
}

/// The outlets of an L surface (reading 12): the window's edge cells, and the cells at or below `sea` 8-connected (through such cells) to a `seed` cell or an edge cell.
pub fn sea_outlets(z: &[f64], w: usize, h: usize, seed: &dyn Fn(usize) -> bool, sea: f64) -> Vec<bool> {
    let n = w * h;
    let edge = |p: usize| p % w == 0 || p / w == 0 || p % w == w - 1 || p / w == h - 1;
    let mut o: Vec<bool> = (0..n).map(edge).collect();
    let mut stack: Vec<usize> = (0..n).filter(|&p| z[p] <= sea && (edge(p) || seed(p))).collect();
    for &p in &stack {
        o[p] = true;
    }
    while let Some(c) = stack.pop() {
        for q in neighbours(w, h, c) {
            if !o[q] && z[q] <= sea {
                o[q] = true;
                stack.push(q);
            }
        }
    }
    o
}

/// The band made to drain on its coarse surface (A+): `z = kh + r` drained with [`drain`] (outlets [`sea_outlets`]); the band `r'' = drained - kh`.
#[allow(clippy::too_many_arguments)]
pub fn drain_band(
    kh: &[f64],
    r: &[f64],
    w: usize,
    h: usize,
    seed: &dyn Fn(usize) -> bool,
    keep: &dyn Fn(usize) -> bool,
    area: &dyn Fn(usize) -> f64,
    max_rounds: usize,
) -> (Vec<f64>, DrainStats) {
    drain_band_capped(kh, r, w, h, seed, keep, area, max_rounds, f64::INFINITY)
}

/// [`drain_band`] with [`drain_selective`]'s rule: a breach only within `cap` m of cut, a fill only within `cap` m of depth (spikes #145: the basins K5 left stay).
#[allow(clippy::too_many_arguments)]
pub fn drain_band_capped(
    kh: &[f64],
    r: &[f64],
    w: usize,
    h: usize,
    seed: &dyn Fn(usize) -> bool,
    keep: &dyn Fn(usize) -> bool,
    area: &dyn Fn(usize) -> f64,
    max_rounds: usize,
    cap: f64,
) -> (Vec<f64>, DrainStats) {
    let z: Vec<f64> = (0..w * h).map(|p| kh[p] + r[p]).collect();
    let outlet = sea_outlets(&z, w, h, seed, 0.0);
    let (d, st) = if cap.is_finite() {
        drain_selective(&z, w, h, &outlet, keep, area, max_rounds, 4096, &|_| cap, &|_| cap)
    } else {
        drain(&z, w, h, &outlet, keep, area, max_rounds, 4096)
    };
    ((0..w * h).map(|p| d[p] - kh[p]).collect(), st)
}

/// The depressions of `z` (outlets `outlet`) whose floor `inside` admits: (any over TOL, of them at least 0.5 m deep and 16 cells, of them over 5 m deep, their fill volume m3).
pub fn pit_census(z: &[f64], w: usize, h: usize, outlet: &[bool], inside: &dyn Fn(usize) -> bool, area: &dyn Fn(usize) -> f64) -> (usize, usize, usize, f64) {
    let level = fill_level(z, w, h, outlet);
    let (_, comps) = depressions(z, &level, w, h);
    let (mut any, mut g, mut d5, mut vol) = (0usize, 0usize, 0usize, 0.0f64);
    for (cells, floor) in comps.iter().filter(|(_, f)| inside(*f)) {
        let depth = level[*floor] - z[*floor];
        any += 1;
        g += usize::from(depth >= 0.5 && cells.len() >= 16);
        d5 += usize::from(depth > 5.0);
        vol += cells.iter().map(|&c| (level[c as usize] - z[c as usize]) * area(c as usize)).sum::<f64>();
    }
    (any, g, d5, vol)
}

/// The fill level of the depressions of `z` (outlets `outlet`), -inf outside them (spikes #145: K5's breach floor inside the basins the selective drain left).
pub fn level_floor(z: &[f64], w: usize, h: usize, outlet: &[bool]) -> Vec<f64> {
    let level = fill_level(z, w, h, outlet);
    (0..w * h).map(|p| if level[p] > z[p] + TOL && level[p].is_finite() { level[p] } else { f64::NEG_INFINITY }).collect()
}
