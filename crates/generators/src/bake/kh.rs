//! KH, the hand-over of `z_s` at L (spike code on `spike/kh-stage`, never merged as is; SPIKE-136's h2, G0 contracts §12 reading 19).
//! Each H1 quad (the four cell centres `a = (i, j)`, `b = (i + 1, j)`, `c = (i, j + 1)`, `d = (i + 1, j + 1)` of a block of H1 heights) is read as two triangles, linear on each:
//! - split along the quad's D8 diagonal step (`a`-`d` or `b`-`c`), so every D8 step of `z_s` is an edge of the read; when both diagonals step, the one with the larger D8 area;
//! - otherwise along the diagonal whose lower end is higher (the higher pair stays connected; never valley to valley across a divide).
//!
//! No H1 height is changed. The split is an H1 field (from H1's routing, landmass-global like H1); the read of an L cell uses its quad's four heights and one flag only, so any
//! partition of the L cells gives the same bits.

use super::flood::priority_flood_with;
use super::route::NONE;
use super::Grid;

/// The quad is split along `a`-`d`.
pub const MAIN: u8 = 0;
/// The quad is split along `b`-`c`.
pub const ANTI: u8 = 1;

/// How the quads were split.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SplitStats {
    /// Quads split along a D8 diagonal step.
    pub by_step: usize,
    /// Of them, quads with a step on both diagonals.
    pub both: usize,
    /// Quads split along the higher pair.
    pub by_pair: usize,
}

/// The split of each quad of a `w x h` block of heights `z` (row-major). `recv(p)` is the block index of cell `p`'s D8 receiver (`None`: an outlet or outside the block);
/// `area(p)` its D8 drainage area. The last row and column carry no quad (their flag is `MAIN`).
pub fn split(z: &[f64], w: usize, h: usize, recv: &dyn Fn(usize) -> Option<usize>, area: &dyn Fn(usize) -> f64) -> (Vec<u8>, SplitStats) {
    assert_eq!(z.len(), w * h, "one height per cell of the block");
    let mut st = SplitStats::default();
    let flags = (0..w * h)
        .map(|q| {
            let (i, j) = (q % w, q / w);
            if i + 1 >= w || j + 1 >= h {
                return MAIN;
            }
            let (a, b, c, d) = (q, q + 1, q + w, q + w + 1);
            let step = |p: usize, t: usize| recv(p) == Some(t);
            let main = if step(a, d) {
                area(a)
            } else if step(d, a) {
                area(d)
            } else {
                0.0
            };
            let anti = if step(b, c) {
                area(b)
            } else if step(c, b) {
                area(c)
            } else {
                0.0
            };
            if main > 0.0 || anti > 0.0 {
                st.by_step += 1;
                st.both += usize::from(main > 0.0 && anti > 0.0);
                if anti > main {
                    ANTI
                } else {
                    MAIN
                }
            } else {
                st.by_pair += 1;
                if z[b].min(z[c]) > z[a].min(z[d]) {
                    ANTI
                } else {
                    MAIN
                }
            }
        })
        .collect();
    (flags, st)
}

/// The read at block coordinates `(fx, fy)` (cell centres at integers), clamped to the block's quads.
pub fn read(z: &[f64], w: usize, h: usize, split: &[u8], fx: f64, fy: f64) -> f64 {
    let ix = (fx.floor().max(0.0) as usize).min(w - 2);
    let iy = (fy.floor().max(0.0) as usize).min(h - 2);
    let (u, v) = ((fx - ix as f64).clamp(0.0, 1.0), (fy - iy as f64).clamp(0.0, 1.0));
    let q = iy * w + ix;
    let (a, b, c, d) = (z[q], z[q + 1], z[q + w], z[q + w + 1]);
    if split[q] == MAIN {
        if u >= v {
            a + u * (b - a) + v * (d - b)
        } else {
            a + v * (c - a) + u * (d - c)
        }
    } else if u + v <= 1.0 {
        a + u * (b - a) + v * (c - a)
    } else {
        d + (1.0 - u) * (c - d) + (1.0 - v) * (b - d)
    }
}

/// KH's routing on a grid of heights (SPIKE-135's ladder rule): the canonical priority flood with a 1e-3 m grade from `outlet`, D8 steepest descent on it (ties to the lower cell id),
/// and the D8 drainage area of true cell areas. Returns the receivers (`NONE` for an outlet) and the areas.
pub fn routing(grid: &Grid, z: &[f64], outlet: &[bool]) -> (Vec<u32>, Vec<f64>) {
    let n = grid.len();
    let filled = priority_flood_with(grid, z, outlet, 1.0e-3, true);
    let recv: Vec<u32> = (0..n)
        .map(|p| {
            if outlet[p] {
                return NONE;
            }
            let (mut best, mut bq) = (0.0f64, usize::MAX);
            for d in 0..8 {
                if let Some(q) = grid.neighbour(p, d) {
                    let s = (filled[q] - filled[p]) / grid.distance_at(p, d);
                    if s < best || (s == best && bq != usize::MAX && q < bq) {
                        (best, bq) = (s, q);
                    }
                }
            }
            if bq == usize::MAX {
                NONE
            } else {
                bq as u32
            }
        })
        .collect();
    // the D8 area: cells in descending filled height (each cell above its receiver), ties by id
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| filled[b].total_cmp(&filled[a]).then(a.cmp(&b)));
    let mut area: Vec<f64> = (0..n).map(|p| grid.cell_area(p)).collect();
    for &p in &order {
        if recv[p] != NONE {
            let r = recv[p] as usize;
            area[r] += area[p];
        }
    }
    (recv, area)
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: GEN-189 (reading 19, KH): a read at a node is the node's height; a quad split along a step keeps the step's diagonal linear.
    #[test]
    fn nodes_and_diagonals() {
        // 2 x 2 block: a high pair b, c and a low diagonal a -> d (a D8 step)
        let z = [10.0, 50.0, 50.0, 0.0];
        let recv = |p: usize| if p == 0 { Some(3) } else { None };
        let (s, st) = split(&z, 2, 2, &recv, &|_| 1.0);
        assert_eq!(s[0], MAIN);
        assert_eq!(st, SplitStats { by_step: 1, both: 0, by_pair: 0 });
        for (q, (x, y)) in [(0, (0.0, 0.0)), (1, (1.0, 0.0)), (2, (0.0, 1.0)), (3, (1.0, 1.0))] {
            assert_eq!(read(&z, 2, 2, &s, x, y), z[q]);
        }
        // the middle of the step's diagonal: the mean of a and d, not of all four
        assert_eq!(read(&z, 2, 2, &s, 0.5, 0.5), 5.0);
        // without the step, the higher pair b, c stays connected
        let (s2, st2) = split(&z, 2, 2, &|_| None, &|_| 1.0);
        assert_eq!(s2[0], ANTI);
        assert_eq!(st2.by_pair, 1);
        assert_eq!(read(&z, 2, 2, &s2, 0.5, 0.5), 50.0);
    }
}
