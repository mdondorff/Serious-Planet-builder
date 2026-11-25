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
// spike only (never merged): kept as run, so the runs keep their bits; main gets its own form through an OpenSpec change
#![allow(clippy::neg_cmp_op_on_partial_ord, clippy::manual_is_multiple_of, clippy::items_after_test_module, clippy::needless_range_loop)]

use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// spikes #147 (spike only): the breach path's cost per step. `false` (SPIKE-138): `max(0, z - z_floor)` per cell, whatever the step's length, so a diagonal step covers
/// sqrt 2 of the distance at the price of one cell and paths across even ground run diagonal. `true`: that cost times the step's length (1 or sqrt 2), plus
/// `LENGTH_COST_M` m per unit length, so a path on even ground is the shortest one in the octile metric.
pub static BREACH_BY_LENGTH: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// spikes #147 (spike only): the choice when both a breach and a fill are within their caps. `false` (SPIKE-138): the smaller volume. `true`: the fill (on a lowland the
/// smaller volume is a long narrow trench, which K5 then follows as a straight channel).
pub static PREFER_FILL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// lead #161 (spike only): a breach whose deepest cut exceeds this many micrometres is not taken: the depression is filled when the fill is allowed, else it stays
/// (an over-budget deep depression); 0: off. rg_chain sets it around band6's drain only.
pub static HUMP_LIMIT_MICRO: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// spikes #147 (spike only): a breach only along a path of at most this many cells (Lindsay's hybrid breach-fill); 0: no limit.
pub static BREACH_MAX_LEN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// spikes #147a (spike only): the breach path's cost per cell is the square of the cut (`max(0, z - z_floor)`^2, m^2), so a path of many shallow cuts beats one of a few
/// deep ones: the wide shallow saddle over the narrow deep notch. Wins over [`BREACH_BY_LENGTH`].
pub static BREACH_SQUARE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// lead #191 (spike only): an outlet cell keeps its per-cell floor in [`drain_selective_floors`]. `false`: an outlet is exempt from `cell_lo` (the search enters it and a
/// path ending there is taken), yet the path's cuts include it, so a border exit is cut past its budget (#190: R1h 11, M1 17 cells). `true`: an outlet the breach would cut
/// under its floor is not entered, and a path that would is not taken. rg_chain sets it around band6's drain only.
pub static OUTLET_FLOORS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// The cost per unit of path length of [`BREACH_BY_LENGTH`] (m).
pub const LENGTH_COST_M: f64 = 0.01;

/// crusader #198 (spike only, diagnostic like `PLANET_WIDTH_DUMP`): the stage whose accepted breaches are recorded, with the grid's place on the cube
/// (face, cells per face edge, the face cell of grid cell 0). `None`: nothing is recorded (the default). Recording reads the breach and changes nothing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BreachStage {
    pub stage: &'static str,
    pub face: u8,
    pub n: i64,
    pub origin: (i64, i64),
}

/// crusader #198: one accepted breach: its path (grid cells, from the depression's floor to the cell it drains to), its deepest cut and cut volume, and the
/// depression's cells, area (m2), spill (fill) level and floor (m).
#[derive(Clone, Debug, PartialEq)]
pub struct BreachRecord {
    pub at: BreachStage,
    pub w: usize,
    pub path: Vec<u32>,
    pub cut_max_m: f64,
    pub cut_volume_m3: f64,
    pub depression_cells: usize,
    pub depression_area_m2: f64,
    pub spill_m: f64,
    pub floor_m: f64,
}

static BREACH_AT: std::sync::Mutex<Option<BreachStage>> = std::sync::Mutex::new(None);
static BREACH_LOG: std::sync::Mutex<Vec<BreachRecord>> = std::sync::Mutex::new(Vec::new());

/// crusader #198: records the accepted breaches of the stage `at` from now on (`None`: stops recording).
pub fn record_breaches(at: Option<BreachStage>) {
    *BREACH_AT.lock().unwrap() = at;
}

/// crusader #198: the stage being recorded, if any.
pub fn breach_stage() -> Option<BreachStage> {
    *BREACH_AT.lock().unwrap()
}

/// crusader #198: records one breach when a stage is being recorded; `make` builds it only then.
pub fn record_breach(make: impl FnOnce(BreachStage) -> BreachRecord) {
    if let Some(at) = breach_stage() {
        let r = make(at);
        BREACH_LOG.lock().unwrap().push(r);
    }
}

/// crusader #198: the breaches recorded so far, removed from the log.
pub fn take_breaches() -> Vec<BreachRecord> {
    std::mem::take(&mut *BREACH_LOG.lock().unwrap())
}

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
    drain_selective_floors(z0, w, h, outlet, keep, area, max_rounds, search, cut_cap, fill_cap, &|_| f64::NEG_INFINITY)
}

/// [`drain_selective_report`] with a floor per cell (lead #178 items 1 and 3): a breach path never takes a cell under `cell_lo(c)` (the cell's budget left, reading 17's sea + 0.5 m);
/// the search does not enter a cell the breach would cut under it, and a found path that would is not taken (the depression is filled within its cap, else left).
/// `|_| -inf`: [`drain_selective_report`].
#[allow(clippy::too_many_arguments)]
pub fn drain_selective_floors(
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
    cell_lo: &dyn Fn(usize) -> f64,
) -> (Vec<f64>, DrainStats, Vec<Deep>) {
    let n = w * h;
    let mut z = z0.to_vec();
    let mut st = DrainStats::default();
    let mut deeps: Vec<Deep> = Vec::new();
    let mut dist = vec![f64::INFINITY; n];
    let mut parent = vec![u32::MAX; n];
    let mut seen: Vec<usize> = Vec::new();
    // lead #191: read once per call, so the search and floors_ok agree by construction
    let outlet_floors = OUTLET_FLOORS.load(std::sync::atomic::Ordering::Relaxed);
    let outlet_free = |q: usize| outlet[q] && !outlet_floors;
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
                let square = BREACH_SQUARE.load(std::sync::atomic::Ordering::Relaxed);
                for q in neighbours(w, h, c) {
                    // lead #178: a cell the breach would cut under its floor is not entered. An outlet ends the path but is cut with it (#190); it is exempt unless
                    // OUTLET_FLOORS (lead #191)
                    if !outlet_free(q) && z[q] > zf && zf < cell_lo(q) {
                        continue;
                    }
                    let nd = if square {
                        let cut = (z[q] - zf).max(0.0);
                        d + cut * cut
                    } else if by_length {
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
                let max_len = BREACH_MAX_LEN.load(std::sync::atomic::Ordering::Relaxed);
                // lead #178: the path's descent of DELTA a cell stays above every cell's floor (the end outlet's too with OUTLET_FLOORS, lead #191)
                let floors_ok = path
                    .iter()
                    .enumerate()
                    .all(|(i, &c)| outlet_free(c) || z[c] <= zf - DELTA * i as f64 || zf - DELTA * i as f64 >= cell_lo(c));
                (floors_ok && cut <= cut_cap(*floor) && (max_len == 0 || path.len() <= max_len), v, cut)
            };
            let prefer_fill = PREFER_FILL.load(std::sync::atomic::Ordering::Relaxed);
            let hump = HUMP_LIMIT_MICRO.load(std::sync::atomic::Ordering::Relaxed);
            let breach_ok = breach_ok && (hump == 0 || cut <= hump as f64 * 1.0e-6);
            if breach_ok && (!fill_ok || (!prefer_fill && breach_v <= fill_v)) {
                for (i, &c) in path.iter().enumerate() {
                    cuts.push((c, zf - DELTA * i as f64));
                }
                st.breached += 1;
                record_breach(|at| BreachRecord {
                    at,
                    w,
                    path: path.iter().map(|&c| c as u32).collect(),
                    cut_max_m: cut,
                    cut_volume_m3: breach_v,
                    depression_cells: cells.len(),
                    depression_area_m2: cells.iter().map(|&c| area(c as usize)).sum(),
                    spill_m: lv,
                    floor_m: zf,
                });
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

    // crusader #198 (spike only, diagnostic): PLANET_BREACH_DUMP's record changes no cell
    #[test]
    fn recording_the_breaches_changes_no_cell_and_records_each_accepted_one() {
        // a 13 x 5 strip (a width no other test uses: the log is process-global), outlet at x = 0, three pits behind one-cell rims of different heights
        let (w, h) = (13usize, 5usize);
        let mut z = vec![10.0f64; w * h];
        for y in 0..h {
            for x in 0..w {
                z[y * w + x] = if x == 0 { 0.0 } else { 1.0 + 0.05 * x as f64 };
            }
            for (rim, top) in [(3usize, 4.0), (7, 2.5), (10, 6.0)] {
                z[y * w + rim] = top;
            }
            z[y * w + 5] = 0.6;
            z[y * w + 9] = 0.8;
            z[y * w + 12] = 0.7;
        }
        let outlet: Vec<bool> = (0..w * h).map(|p| p % w == 0).collect();
        let caps = |_: usize| 50.0;
        let run = || drain_selective(&z, w, h, &outlet, &|_| false, &|_| 4.0, 10, 1000, &caps, &caps);
        let (off, st_off) = run();
        let at = BreachStage { stage: "test-198", face: 2, n: 64, origin: (5, 7) };
        record_breaches(Some(at));
        let (on, st_on) = run();
        record_breaches(None);
        let mine: Vec<BreachRecord> = take_breaches().into_iter().filter(|r| r.w == w && r.at == at).collect();
        assert!(off.iter().zip(&on).all(|(a, b)| a.to_bits() == b.to_bits()), "the record changes no cell");
        assert_eq!((st_off.breached, st_off.filled), (st_on.breached, st_on.filled));
        assert!(st_on.breached >= 1, "{st_on:?}");
        assert_eq!(mine.len(), st_on.breached, "one record per accepted breach");
        for r in &mine {
            let (first, last) = (r.path[0] as usize, *r.path.last().unwrap() as usize);
            assert!(z[first] <= r.spill_m && (r.floor_m - z[first]).abs() < 1e-12, "the path starts at the depression's floor");
            assert!(outlet[last] || z[last] < r.floor_m, "it ends where the water leaves");
            assert!(r.cut_max_m >= 0.0 && r.cut_max_m <= 50.0 && r.cut_volume_m3 >= 0.0);
            assert!(r.depression_cells >= 1 && r.depression_area_m2 == 4.0 * r.depression_cells as f64 && r.spill_m >= r.floor_m);
        }
        // nothing is recorded with no stage set
        let _ = run();
        assert!(take_breaches().iter().all(|r| r.w != w || r.at != at));
    }

    // SPIKE-138 (spike only; not a spec requirement)
    #[test]
    fn a_breach_holds_each_cells_floor_and_the_pit_is_filled_instead() {
        // lead #178: the strip below with the rim's floor at 3 m (its budget left): the breach would cut it to 0.5 m, so it is not taken and the pit is filled; without the floor it is
        let (w, h) = (9usize, 3usize);
        let mut z = vec![10.0f64; w * h];
        for y in 0..h {
            z[y * w] = 0.0;
            for x in 1..w {
                z[y * w + x] = 1.0 + 0.1 * x as f64;
            }
            z[y * w + 4] = 5.0;
            for x in 5..w {
                z[y * w + x] = 0.5;
            }
        }
        let outlet: Vec<bool> = (0..w * h).map(|p| p % w == 0).collect();
        let inf = |_: usize| f64::INFINITY;
        let lo = |p: usize| if p % w == 4 { 3.0 } else { f64::NEG_INFINITY };
        let (free, sf, _) = drain_selective_floors(&z, w, h, &outlet, &|_| false, &|_| 1.0, 10, 1000, &inf, &inf, &|_| f64::NEG_INFINITY);
        let (held, sh, _) = drain_selective_floors(&z, w, h, &outlet, &|_| false, &|_| 1.0, 10, 1000, &inf, &inf, &lo);
        assert!(sf.breached >= 1 && (0..h).any(|y| free[y * w + 4] < 3.0), "{sf:?}");
        assert!((0..h).all(|y| held[y * w + 4] >= 3.0), "no rim cell under its floor");
        assert!(sh.breached == 0 && sh.filled >= 1 && sh.left == 0, "{sh:?}");
        let (rep, _, _) = drain_selective_report(&z, w, h, &outlet, &|_| false, &|_| 1.0, 10, 1000, &inf, &inf);
        assert_eq!(rep, free, "no floor: the report's surface");
    }

    /// Tests that set [`OUTLET_FLOORS`] hold this lock for their whole run; [`OutletFloorsOn`] clears the flag on drop (also on a panic).
    static OUTLET_FLOORS_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    fn outlet_floors_lock() -> std::sync::MutexGuard<'static, ()> {
        OUTLET_FLOORS_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }
    struct OutletFloorsOn;
    impl OutletFloorsOn {
        fn set() -> Self {
            OUTLET_FLOORS.store(true, std::sync::atomic::Ordering::Relaxed);
            OutletFloorsOn
        }
    }
    impl Drop for OutletFloorsOn {
        fn drop(&mut self) {
            OUTLET_FLOORS.store(false, std::sync::atomic::Ordering::Relaxed);
        }
    }

    // lead #191 (spike only; not a spec requirement): the outlet at a breach's end is cut under its floor with OUTLET_FLOORS off; floors_ok refuses that path with it on
    #[test]
    fn floors_ok_refuses_a_path_that_cuts_its_end_outlet_under_its_floor() {
        let _lock = outlet_floors_lock();
        // a 10 x 1 strip: the outlet at x = 0 at the pit's floor height, so the search enters it whatever its floor; the path's descent of DELTA a cell cuts it by 2 DELTA
        let (w, h) = (10usize, 1usize);
        let z = vec![2.0, 9.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0];
        let outlet: Vec<bool> = (0..w).map(|p| p == 0).collect();
        let inf = |_: usize| f64::INFINITY;
        let lo = |p: usize| if p == 0 { 2.0 - 0.5 * DELTA } else { f64::NEG_INFINITY };
        let (free, sf, _) = drain_selective_floors(&z, w, h, &outlet, &|_| false, &|_| 1.0, 10, 1000, &inf, &inf, &lo);
        let (held, sh, _) = {
            let _on = OutletFloorsOn::set();
            drain_selective_floors(&z, w, h, &outlet, &|_| false, &|_| 1.0, 10, 1000, &inf, &inf, &lo)
        };
        assert!(sf.breached >= 1 && free[0] < 2.0 - 0.5 * DELTA, "off: the end outlet is cut under its floor ({free:?}, {sf:?})");
        assert!(held[0] >= 2.0 - 0.5 * DELTA, "on: the end outlet stays at or above its floor ({held:?})");
        assert!(sh.breached == 0 && sh.filled >= 1, "on: the pit is filled instead ({sh:?})");
    }

    // lead #191 (spike only; not a spec requirement): the breach's end outlet keeps its floor with OUTLET_FLOORS
    #[test]
    fn the_end_outlet_keeps_its_floor_with_outlet_floors() {
        let _lock = outlet_floors_lock();
        // a 10 x 1 strip: the outlet at x = 0 stands high (a border exit on land), a one-cell rim at x = 1, a wide pit behind it (the breach, 13 m3, is cheaper than the
        // fill, 56 m3); only the outlet carries a floor
        let (w, h) = (10usize, 1usize);
        let z = vec![8.0, 9.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0];
        let outlet: Vec<bool> = (0..w).map(|p| p == 0).collect();
        let inf = |_: usize| f64::INFINITY;
        let lo = |p: usize| if p == 0 { 7.0 } else { f64::NEG_INFINITY };
        let (free, sf, _) = drain_selective_floors(&z, w, h, &outlet, &|_| false, &|_| 1.0, 10, 1000, &inf, &inf, &lo);
        let (held, sh, _) = {
            let _on = OutletFloorsOn::set();
            drain_selective_floors(&z, w, h, &outlet, &|_| false, &|_| 1.0, 10, 1000, &inf, &inf, &lo)
        };
        assert!(sf.breached >= 1 && free[0] < 7.0, "off: the end outlet is cut under its floor ({free:?}, {sf:?})");
        assert!(held[0] >= 7.0, "on: the end outlet stays at or above its floor ({held:?})");
        assert!(sh.breached == 0 && sh.filled >= 1, "on: the pit is filled instead ({sh:?})");
        for p in 1..w {
            assert!(held[p] >= z[p], "on: no other cell is cut ({held:?})");
        }
    }

    // SPIKE-138 (spike only; not a spec requirement)
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

    // SPIKE-138 (spike only; not a spec requirement)
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

    // SPIKE-138 (spike only; not a spec requirement)
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
pub fn pit_census(
    z: &[f64],
    w: usize,
    h: usize,
    outlet: &[bool],
    inside: &dyn Fn(usize) -> bool,
    area: &dyn Fn(usize) -> f64,
) -> (usize, usize, usize, f64) {
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

// ---- crusader #208 (spike only): the selective drain on any `Grid` (a face window, the cube grid, a region's domain on it) and its join hooks for the wave runner.
// The functions above are untouched, so the chain keeps its bits. These repeat their rule with the neighbours of a `Grid`: on a plane `Grid` of `w x h` they are the same cells in the same
// order (D8's order as `Grid::neighbour`'s directions), so the bits are the same (`the_drain_on_a_plane_grid_is_the_drain`). On the cube grid the index is the cube id, so the ties by the index are
// ties by a partition-free id.

/// D8's order (`D8`) as `Grid::neighbour`'s directions (`bake::NEIGHBOURS`).
const D8_DIR: [usize; 8] = [4, 2, 5, 0, 1, 6, 3, 7];

fn grid_neighbours(g: &super::Grid, p: usize) -> impl Iterator<Item = usize> + '_ {
    D8_DIR.iter().filter_map(move |&d| g.neighbour(p, d))
}

/// [`fill_level`] on a `Grid`.
pub fn fill_level_on(g: &super::Grid, z: &[f64], outlet: &[bool]) -> Vec<f64> {
    let n = g.len();
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
        for q in grid_neighbours(g, c) {
            let lq = z[q].max(l);
            if lq < level[q] {
                level[q] = lq;
                heap.push(Key(lq, q as u32));
            }
        }
    }
    level
}

/// [`depressions`] on a `Grid`.
pub fn depressions_on(g: &super::Grid, z: &[f64], level: &[f64]) -> (Vec<u32>, Vec<(Vec<u32>, usize)>) {
    let n = g.len();
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
            for q in grid_neighbours(g, c) {
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

/// The arguments of [`drain_selective_floors`] besides the grid, the surface and the outlets.
pub struct SelParams<'a> {
    pub keep: &'a dyn Fn(usize) -> bool,
    pub area: &'a dyn Fn(usize) -> f64,
    pub max_rounds: usize,
    pub search: usize,
    pub cut_cap: &'a dyn Fn(usize) -> f64,
    pub fill_cap: &'a dyn Fn(usize) -> f64,
    pub cell_lo: &'a dyn Fn(usize) -> f64,
}

/// One round's shared input: the fill level of the surface, its depressions and the active ones (none holds a kept cell). A partitioned run takes it from the planet-wide fill
/// (GEN-121, GEN-191); here it is computed on the whole grid.
pub struct SelRound {
    pub level: Vec<f64>,
    pub label: Vec<u32>,
    pub comps: Vec<(Vec<u32>, usize)>,
    pub active: Vec<usize>,
}

pub fn sel_round(g: &super::Grid, z: &[f64], outlet: &[bool], p: &SelParams) -> SelRound {
    let level = fill_level_on(g, z, outlet);
    let (label, comps) = depressions_on(g, z, &level);
    let active = (0..comps.len()).filter(|&k| !comps[k].0.iter().any(|&c| (p.keep)(c as usize))).collect();
    SelRound { level, label, comps, active }
}

/// A region's search domain: the cells a search may enter, and its border (the domain's cells with a neighbour outside it).
pub struct SearchDomain<'a> {
    pub inside: &'a [bool],
    pub border: &'a [bool],
}

/// One depression's decision in a round.
#[derive(Clone, Debug, PartialEq)]
pub enum SelDecision {
    /// The path from the floor (its k-th cell lowered to `floor - DELTA k`), its deepest cut and its volume.
    Breach {
        path: Vec<u32>,
        cut: f64,
        volume: f64,
    },
    Fill,
    /// Neither (both over their caps); its least cut (+inf when no path was found).
    Deep {
        cut: f64,
    },
    /// The search reached the domain's border before an exit: decided by the join, on the whole grid.
    Candidate,
}

/// [`drain_selective_floors`]'s decision for active depression `k`, with the search restricted to `dom` when given. Exact in the domain: a search that pops a border cell, or reaches
/// an exit while a border cell's tentative cost is not above the exit's, is a [`SelDecision::Candidate`]; otherwise every cell it popped has its unrestricted cost and parent, so
/// the decision is the unrestricted one (the settle limit included: the pops are the same).
pub fn sel_decide(
    g: &super::Grid,
    z: &[f64],
    outlet: &[bool],
    r: &SelRound,
    k: usize,
    p: &SelParams,
    dom: Option<&SearchDomain>,
) -> SelDecision {
    let outlet_floors = OUTLET_FLOORS.load(std::sync::atomic::Ordering::Relaxed);
    let outlet_free = |q: usize| outlet[q] && !outlet_floors;
    let (cells, floor) = &r.comps[k];
    let floor = *floor;
    let zf = z[floor];
    let lv = r.level[cells[0] as usize];
    let fill_v: f64 = cells.iter().map(|&c| (lv - z[c as usize]) * (p.area)(c as usize)).sum();
    let mut dist: std::collections::HashMap<usize, (f64, u32)> = std::collections::HashMap::new();
    let mut heap = BinaryHeap::new();
    dist.insert(floor, (0.0, u32::MAX));
    heap.push(Key(0.0, floor as u32));
    let limit = 4 * cells.len() + p.search;
    let (mut settled, mut end, mut border_min) = (0usize, None, f64::INFINITY);
    let by_length = BREACH_BY_LENGTH.load(std::sync::atomic::Ordering::Relaxed);
    let square = BREACH_SQUARE.load(std::sync::atomic::Ordering::Relaxed);
    let (w, _) = g.layout();
    while let Some(Key(d, c)) = heap.pop() {
        let c = c as usize;
        if d > dist[&c].0 {
            continue;
        }
        if dom.is_some_and(|m| m.border[c]) {
            return SelDecision::Candidate;
        }
        if c != floor && r.label[c] != k as u32 && (outlet[c] || r.level[c] < zf - TOL) {
            if border_min <= d {
                return SelDecision::Candidate;
            }
            end = Some(c);
            break;
        }
        settled += 1;
        if settled > limit {
            break;
        }
        for q in grid_neighbours(g, c) {
            if dom.is_some_and(|m| !m.inside[q]) {
                continue;
            }
            if !outlet_free(q) && z[q] > zf && zf < (p.cell_lo)(q) {
                continue;
            }
            let nd = if square {
                let cut = (z[q] - zf).max(0.0);
                d + cut * cut
            } else if by_length {
                let len = if q % w != c % w && q / w != c / w { std::f64::consts::SQRT_2 } else { 1.0 };
                d + len * ((z[q] - zf).max(0.0) + LENGTH_COST_M)
            } else {
                d + (z[q] - zf).max(0.0)
            };
            if dist.get(&q).is_none_or(|&(b, _)| nd < b) {
                dist.insert(q, (nd, c as u32));
                heap.push(Key(nd, q as u32));
                if dom.is_some_and(|m| m.border[q]) {
                    border_min = border_min.min(nd);
                }
            }
        }
    }
    let mut path = Vec::new();
    if let Some(e) = end {
        let mut c = e;
        path.push(c);
        while dist[&c].1 != u32::MAX {
            c = dist[&c].1 as usize;
            path.push(c);
        }
        path.reverse();
    }
    let fill_ok = lv - zf <= (p.fill_cap)(floor);
    let (breach_ok, breach_v, cut) = if path.is_empty() {
        (false, f64::INFINITY, f64::INFINITY)
    } else {
        let v: f64 = path.iter().enumerate().map(|(i, &c)| (z[c] - z[c].min(zf - DELTA * i as f64)).max(0.0) * (p.area)(c)).sum();
        let cut: f64 = path.iter().enumerate().map(|(i, &c)| z[c] - z[c].min(zf - DELTA * i as f64)).fold(0.0, f64::max);
        let max_len = BREACH_MAX_LEN.load(std::sync::atomic::Ordering::Relaxed);
        let floors_ok = path
            .iter()
            .enumerate()
            .all(|(i, &c)| outlet_free(c) || z[c] <= zf - DELTA * i as f64 || zf - DELTA * i as f64 >= (p.cell_lo)(c));
        (floors_ok && cut <= (p.cut_cap)(floor) && (max_len == 0 || path.len() <= max_len), v, cut)
    };
    let prefer_fill = PREFER_FILL.load(std::sync::atomic::Ordering::Relaxed);
    let hump = HUMP_LIMIT_MICRO.load(std::sync::atomic::Ordering::Relaxed);
    let breach_ok = breach_ok && (hump == 0 || cut <= hump as f64 * 1.0e-6);
    if breach_ok && (!fill_ok || (!prefer_fill && breach_v <= fill_v)) {
        SelDecision::Breach { path: path.iter().map(|&c| c as u32).collect(), cut, volume: breach_v }
    } else if fill_ok {
        SelDecision::Fill
    } else {
        SelDecision::Deep { cut }
    }
}

/// Applies a round's decisions to the cells `owned` selects: every fill (to its level, by max), then every breach (to `floor - DELTA k`, by min), as [`drain_selective_floors`].
pub fn sel_apply(z0: &[f64], z: &mut [f64], r: &SelRound, decisions: &[(usize, SelDecision)], owned: &dyn Fn(usize) -> bool) {
    for (k, dec) in decisions {
        if *dec == SelDecision::Fill {
            let lv = r.level[r.comps[*k].0[0] as usize];
            for &c in r.comps[*k].0.iter().filter(|&&c| owned(c as usize)) {
                z[c as usize] = z[c as usize].max(lv);
            }
        }
    }
    for (k, dec) in decisions {
        if let SelDecision::Breach { path, .. } = dec {
            let zf = z0[r.comps[*k].1];
            for (i, &c) in path.iter().enumerate().filter(|(_, &c)| owned(c as usize)) {
                z[c as usize] = z[c as usize].min(zf - DELTA * i as f64);
            }
        }
    }
}

/// [`drain_selective_floors`] on a `Grid`, composed of the hooks above (the unpartitioned run: every decision on the whole grid). No breach record.
pub fn drain_selective_floors_on(g: &super::Grid, z0: &[f64], outlet: &[bool], p: &SelParams) -> (Vec<f64>, DrainStats) {
    let mut z = z0.to_vec();
    let mut st = DrainStats::default();
    for round in 0..p.max_rounds {
        let r = sel_round(g, &z, outlet, p);
        st.rounds = round + 1;
        if r.active.is_empty() {
            st.kept = r.comps.len();
            return (z, st);
        }
        st.depressions += r.active.len();
        let decisions: Vec<(usize, SelDecision)> = r.active.iter().map(|&k| (k, sel_decide(g, &z, outlet, &r, k, p, None))).collect();
        st.deep = decisions.iter().filter(|(_, d)| matches!(d, SelDecision::Deep { .. })).count();
        let changed = decisions.iter().any(|(_, d)| matches!(d, SelDecision::Fill | SelDecision::Breach { .. }));
        if !changed {
            break;
        }
        st.filled += decisions.iter().filter(|(_, d)| *d == SelDecision::Fill).count();
        st.breached += decisions.iter().filter(|(_, d)| matches!(d, SelDecision::Breach { .. })).count();
        let zr = z.clone();
        sel_apply(&zr, &mut z, &r, &decisions, &|_| true);
    }
    let level = fill_level_on(g, &z, outlet);
    let (_, comps) = depressions_on(g, &z, &level);
    let intended = comps.iter().filter(|(cells, _)| cells.iter().any(|&c| (p.keep)(c as usize))).count();
    st.kept = intended;
    st.left = comps.len() - intended;
    (z, st)
}

#[cfg(test)]
mod on_grid_tests {
    use super::*;

    // spike only (crusader #208): the generic drain on a plane grid is today's drain, bit for bit, on random relief with floors, kept cells and caps
    #[test]
    fn the_drain_on_a_plane_grid_is_the_drain() {
        let (w, h) = (61usize, 47usize);
        let mut z = vec![0.0f64; w * h];
        let mut s = 0x1234_5678_9abc_def0u64;
        for v in z.iter_mut() {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            *v = (s % 4000) as f64 / 40.0;
        }
        let outlet: Vec<bool> = (0..w * h).map(|p| p % w == 0 || z[p] < 2.0).collect();
        let keep = |p: usize| p == 17 * w + 30;
        let cap = |p: usize| 20.0 + (p % 7) as f64;
        let lo = |p: usize| if p % 11 == 0 { 30.0 } else { f64::NEG_INFINITY };
        let area = |p: usize| 1.0 + (p % 3) as f64 * 0.25;
        let (a, sa, _) = drain_selective_floors(&z, w, h, &outlet, &keep, &area, 8, 4096, &cap, &cap, &lo);
        let g = super::super::Grid::new(w, h, 1.0);
        let p = SelParams { keep: &keep, area: &area, max_rounds: 8, search: 4096, cut_cap: &cap, fill_cap: &cap, cell_lo: &lo };
        let (b, sb) = drain_selective_floors_on(&g, &z, &outlet, &p);
        assert!(sa.breached > 0 && sa.filled > 0, "the relief has breaches and fills: {sa:?}");
        assert!(a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits()), "the same bits");
        assert_eq!(
            (sa.rounds, sa.depressions, sa.filled, sa.breached, sa.deep),
            (sb.rounds, sb.depressions, sb.filled, sb.breached, sb.deep)
        );
    }
}
