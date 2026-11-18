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

// ---------------------------------------------------------------- h4: Clough-Tocher patches on the split (spikes #138)

/// Central-difference gradients of a `w x h` block in block units (one-sided at the border).
pub fn gradients(z: &[f64], w: usize, h: usize) -> Vec<[f64; 2]> {
    (0..w * h)
        .map(|p| {
            let (i, j) = (p % w, p / w);
            let gx = if i == 0 {
                z[p + 1] - z[p]
            } else if i + 1 == w {
                z[p] - z[p - 1]
            } else {
                0.5 * (z[p + 1] - z[p - 1])
            };
            let gy = if j == 0 {
                z[p + w] - z[p]
            } else if j + 1 == h {
                z[p] - z[p - w]
            } else {
                0.5 * (z[p + w] - z[p - w])
            };
            [gx, gy]
        })
        .collect()
}

/// The triangle of the split that holds the point `(u, v)` of quad `q`: its corners as block indices and quad-local coordinates.
fn triangle(q: usize, w: usize, flag: u8, u: f64, v: f64) -> [(usize, [f64; 2]); 3] {
    let (a, b, c, d) = ((q, [0.0, 0.0]), (q + 1, [1.0, 0.0]), (q + w, [0.0, 1.0]), (q + w + 1, [1.0, 1.0]));
    if flag == MAIN {
        if u >= v {
            [a, b, d]
        } else {
            [a, d, c]
        }
    } else if u + v <= 1.0 {
        [a, b, c]
    } else {
        [b, d, c]
    }
}

/// One reduced Clough-Tocher macro triangle (cubic Bezier patches on the centroid split, the normal derivative linear along each outer edge) at the point `x`, from the
/// corners `p`, values `f` and gradients `g`; an edge `(i, i + 1)` with `mono[i]` keeps its control polygon monotone (the edge points clamped between the corner values).
/// Returns the value.
fn clough_tocher(p: [[f64; 2]; 3], f: [f64; 3], g: [[f64; 2]; 3], mono: [bool; 3], x: [f64; 2]) -> f64 {
    let dot = |a: [f64; 2], b: [f64; 2]| a[0] * b[0] + a[1] * b[1];
    let sub = |a: [f64; 2], b: [f64; 2]| [a[0] - b[0], a[1] - b[1]];
    let s = [(p[0][0] + p[1][0] + p[2][0]) / 3.0, (p[0][1] + p[1][1] + p[2][1]) / 3.0];
    // the edge points (i near, j far) of edge (i, j) = (k, k + 1)
    let edge = |k: usize| -> (f64, f64) {
        let (i, j) = (k, (k + 1) % 3);
        let mut cij = f[i] + dot(g[i], sub(p[j], p[i])) / 3.0;
        let mut cji = f[j] + dot(g[j], sub(p[i], p[j])) / 3.0;
        if mono[k] {
            let (lo, hi) = (f[i].min(f[j]), f[i].max(f[j]));
            cij = cij.clamp(lo, hi);
            cji = cji.clamp(lo, hi);
            if (f[i] >= f[j] && cij < cji) || (f[i] < f[j] && cij > cji) {
                let m = 0.5 * (cij + cji);
                cij = m;
                cji = m;
            }
        }
        (cij, cji)
    };
    let e = [edge(0), edge(1), edge(2)];
    let q: Vec<f64> = (0..3).map(|i| f[i] + dot(g[i], sub(s, p[i])) / 3.0).collect();
    // the interior point next to edge k, from the normal derivative being linear along it
    let inner = |k: usize| -> f64 {
        let (i, j) = (k, (k + 1) % 3);
        let t = sub(p[j], p[i]);
        let nrm = [-t[1], t[0]];
        let (ai, aj) = (sub(p[i], s), sub(p[j], s));
        let det = ai[0] * aj[1] - ai[1] * aj[0];
        let di = (nrm[0] * aj[1] - nrm[1] * aj[0]) / det;
        let dj = (ai[0] * nrm[1] - ai[1] * nrm[0]) / det;
        let ds = -(di + dj);
        let (cij, cji) = e[k];
        let c0 = di * f[i] + dj * cij + ds * q[i];
        let c2 = di * cji + dj * f[j] + ds * q[j];
        (0.5 * (c0 + c2) - di * cij - dj * cji) / ds
    };
    let inn = [inner(0), inner(1), inner(2)];
    let pt: Vec<f64> = (0..3).map(|i| (q[i] + inn[i] + inn[(i + 2) % 3]) / 3.0).collect();
    let centre = (pt[0] + pt[1] + pt[2]) / 3.0;
    // the sub-triangle (i, j, s) holding x and its barycentric coordinates
    let bary = |i: usize, j: usize| -> [f64; 3] {
        let (v0, v1, v2) = (sub(p[i], s), sub(p[j], s), sub(x, s));
        let det = v0[0] * v1[1] - v0[1] * v1[0];
        let li = (v2[0] * v1[1] - v2[1] * v1[0]) / det;
        let lj = (v0[0] * v2[1] - v0[1] * v2[0]) / det;
        [li, lj, 1.0 - li - lj]
    };
    let mut best = (0usize, [0.0; 3], f64::NEG_INFINITY);
    for k in 0..3 {
        let l = bary(k, (k + 1) % 3);
        let m = l[0].min(l[1]).min(l[2]);
        if m > best.2 {
            best = (k, l, m);
        }
    }
    let (k, l, _) = best;
    let (i, j) = (k, (k + 1) % 3);
    let (cij, cji) = e[k];
    let (a, b, c) = (l[0], l[1], l[2]);
    f[i] * a * a * a
        + f[j] * b * b * b
        + centre * c * c * c
        + 3.0 * (cij * a * a * b + cji * a * b * b + q[i] * a * a * c + q[j] * b * b * c + pt[i] * a * c * c + pt[j] * b * c * c)
        + 6.0 * inn[k] * a * b * c
}

/// h4: the Clough-Tocher read on the split at block coordinates `(fx, fy)`. `grad` from [`gradients`]; `step(p, q)` whether the edge between cells `p` and `q` is a D8 step
/// (its control polygon is kept monotone). Returns the value and the lowest and highest corner of the macro triangle (for the overshoot).
#[allow(clippy::too_many_arguments)]
pub fn read_ct(z: &[f64], w: usize, h: usize, split: &[u8], grad: &[[f64; 2]], step: &dyn Fn(usize, usize) -> bool, fx: f64, fy: f64) -> (f64, f64, f64) {
    let ix = (fx.floor().max(0.0) as usize).min(w - 2);
    let iy = (fy.floor().max(0.0) as usize).min(h - 2);
    let (u, v) = ((fx - ix as f64).clamp(0.0, 1.0), (fy - iy as f64).clamp(0.0, 1.0));
    let q = iy * w + ix;
    let t = triangle(q, w, split[q], u, v);
    let f = [z[t[0].0], z[t[1].0], z[t[2].0]];
    let g = [grad[t[0].0], grad[t[1].0], grad[t[2].0]];
    let mono = [step(t[0].0, t[1].0), step(t[1].0, t[2].0), step(t[2].0, t[0].0)];
    let val = clough_tocher([t[0].1, t[1].1, t[2].1], f, g, mono, [u, v]);
    (val, f[0].min(f[1]).min(f[2]), f[0].max(f[1]).max(f[2]))
}

/// h4c (reading 20, spikes #139): [`read_ct`] clamped to the range of its quad's four corners. Returns the value and that range.
#[allow(clippy::too_many_arguments)]
pub fn read_ct_clamped(z: &[f64], w: usize, h: usize, split: &[u8], grad: &[[f64; 2]], step: &dyn Fn(usize, usize) -> bool, fx: f64, fy: f64) -> (f64, f64, f64) {
    let ix = (fx.floor().max(0.0) as usize).min(w - 2);
    let iy = (fy.floor().max(0.0) as usize).min(h - 2);
    let q = iy * w + ix;
    let (a, b, c, d) = (z[q], z[q + 1], z[q + w], z[q + w + 1]);
    let (lo, hi) = (a.min(b).min(c).min(d), a.max(b).max(c).max(d));
    (read_ct(z, w, h, split, grad, step, fx, fy).0.clamp(lo, hi), lo, hi)
}

// ---------------------------------------------------------------- h3: the skeleton (spikes #138)

/// h3: the base read `base` (C1) lowered toward each D8 diagonal step line of the block near the point, never raised: within `full` L cells of a step line (the segment
/// between the two cell centres) to the line's linear value at the point's projection, falling off smoothly to no change at `outer` L cells. `diag(q)` lists the quad's
/// diagonal steps as (from, to) block indices; `l` is L cells a block cell.
#[allow(clippy::too_many_arguments)]
pub fn read_skeleton(z: &[f64], w: usize, h: usize, diag: &dyn Fn(usize) -> Option<(usize, usize)>, base: f64, fx: f64, fy: f64, l: f64, full: f64, outer: f64) -> f64 {
    let ix = (fx.floor().max(0.0) as i64).min(w as i64 - 2);
    let iy = (fy.floor().max(0.0) as i64).min(h as i64 - 2);
    let mut out = base;
    for qy in (iy - 1)..=(iy + 1) {
        for qx in (ix - 1)..=(ix + 1) {
            if qx < 0 || qy < 0 || qx > w as i64 - 2 || qy > h as i64 - 2 {
                continue;
            }
            let Some((a, b)) = diag(qy as usize * w + qx as usize) else { continue };
            let (ax, ay, bx, by) = ((a % w) as f64, (a / w) as f64, (b % w) as f64, (b / w) as f64);
            let (dx, dy) = (bx - ax, by - ay);
            let t = (((fx - ax) * dx + (fy - ay) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
            let dist = ((fx - ax - t * dx).powi(2) + (fy - ay - t * dy).powi(2)).sqrt() * l;
            if dist >= outer {
                continue;
            }
            let target = z[a] + t * (z[b] - z[a]);
            if target >= base {
                continue;
            }
            let s = ((dist - full) / (outer - full)).clamp(0.0, 1.0);
            let weight = 1.0 - s * s * (3.0 - 2.0 * s);
            out = out.min(base - weight * (base - target));
        }
    }
    out
}

// ---------------------------------------------------------------- h5 (reading 25, spikes #144b): h4c on the channel zone only

/// The channel zone of h5: the cells whose D8 area is at least `min_area` m2, and their 8 neighbours.
pub fn channel_zone(area: &[f64], w: usize, h: usize, min_area: f64) -> Vec<bool> {
    let mut zone = vec![false; w * h];
    for p in (0..w * h).filter(|&p| area[p] >= min_area) {
        let (x, y) = ((p % w) as i64, (p / w) as i64);
        for dy in -1..=1i64 {
            for dx in -1..=1i64 {
                let (a, b) = (x + dx, y + dy);
                if a >= 0 && b >= 0 && (a as usize) < w && (b as usize) < h {
                    zone[b as usize * w + a as usize] = true;
                }
            }
        }
    }
    zone
}

/// h5's weight of h4c at block coordinates `(fx, fy)`: `smoothstep(1 - d)`, `d` the distance in cells from the point to the nearest zone cell's square (1 inside the zone).
pub fn zone_weight(zone: &[bool], w: usize, h: usize, fx: f64, fy: f64) -> f64 {
    let (ix, iy) = (fx.round() as i64, fy.round() as i64);
    let mut d = f64::INFINITY;
    for j in iy - 2..=iy + 2 {
        for i in ix - 2..=ix + 2 {
            if i < 0 || j < 0 || i >= w as i64 || j >= h as i64 || !zone[j as usize * w + i as usize] {
                continue;
            }
            let (ex, ey) = (((fx - i as f64).abs() - 0.5).max(0.0), ((fy - j as f64).abs() - 0.5).max(0.0));
            d = d.min((ex * ex + ey * ey).sqrt());
        }
    }
    let t = (1.0 - d).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// ---------------------------------------------------------------- h5d (spikes #147a; the owner's decision-7 fallback): h4c also on the diagonal steps C1 dams

/// Per quad (indexed by its lower-left cell), the D8 diagonal step it holds when C1 dams it: `c1_centre(q)`, C1 at the quad's centre, lies above the step line's
/// value there (the mean of its two cells) by more than `tol` m. `diag(q)` lists the quad's diagonal step as (from, to).
pub fn dammed_steps(z: &[f64], w: usize, h: usize, diag: &dyn Fn(usize) -> Option<(usize, usize)>, c1_centre: &dyn Fn(usize) -> f64, tol: f64) -> Vec<Option<(usize, usize)>> {
    (0..w * h)
        .map(|q| {
            if q % w + 1 >= w || q / w + 1 >= h {
                return None;
            }
            let (a, b) = diag(q)?;
            (c1_centre(q) > 0.5 * (z[a] + z[b]) + tol).then_some((a, b))
        })
        .collect()
}

/// h5d's weight of h4c from the dammed steps at block coordinates `(fx, fy)`: 1 within `full` L cells of a dammed step line (the segment between its two cell centres),
/// smoothstep to 0 at `outer` L cells; `l` is L cells a block cell.
pub fn dam_weight(steps: &[Option<(usize, usize)>], w: usize, h: usize, fx: f64, fy: f64, l: f64, full: f64, outer: f64) -> f64 {
    let ix = (fx.floor().max(0.0) as i64).min(w as i64 - 2);
    let iy = (fy.floor().max(0.0) as i64).min(h as i64 - 2);
    let mut d = f64::INFINITY;
    for qy in iy - 1..=iy + 1 {
        for qx in ix - 1..=ix + 1 {
            if qx < 0 || qy < 0 || qx > w as i64 - 2 || qy > h as i64 - 2 {
                continue;
            }
            let Some((a, b)) = steps[qy as usize * w + qx as usize] else { continue };
            let (ax, ay, bx, by) = ((a % w) as f64, (a / w) as f64, (b % w) as f64, (b / w) as f64);
            let (dx, dy) = (bx - ax, by - ay);
            let t = (((fx - ax) * dx + (fy - ay) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
            d = d.min(((fx - ax - t * dx).powi(2) + (fy - ay - t * dy).powi(2)).sqrt() * l);
        }
    }
    let s = ((d - full) / (outer - full)).clamp(0.0, 1.0);
    1.0 - s * s * (3.0 - 2.0 * s)
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: GEN-189 (reading 19, KH h4): the Clough-Tocher read reproduces a plane and is continuous across the split and across quads.
    #[test]
    fn clough_tocher_plane_and_continuity() {
        let (w, h) = (4, 4);
        let plane: Vec<f64> = (0..w * h).map(|p| 2.0 * (p % w) as f64 + 3.0 * (p / w) as f64 + 1.0).collect();
        let g = gradients(&plane, w, h);
        let split = vec![MAIN, ANTI, MAIN, ANTI, MAIN, ANTI, MAIN, ANTI, MAIN, ANTI, MAIN, ANTI, MAIN, ANTI, MAIN, ANTI];
        for (fx, fy) in [(0.3, 0.7), (1.5, 1.2), (2.9, 0.1), (1.0, 2.5)] {
            let (v, _, _) = read_ct(&plane, w, h, &split, &g, &|_, _| false, fx, fy);
            assert!((v - (2.0 * fx + 3.0 * fy + 1.0)).abs() < 1e-9, "a plane at ({fx}, {fy}): {v}");
        }
        // a rough block: values match on both sides of the split diagonal and of a quad edge
        let z: Vec<f64> = (0..w * h).map(|p| ((p * 37 % 11) as f64) * 10.0).collect();
        let g = gradients(&z, w, h);
        let e = 1e-7;
        let side = |fx: f64, fy: f64| read_ct(&z, w, h, &split, &g, &|_, _| false, fx, fy).0;
        assert!((side(0.5 + e, 0.5 - e) - side(0.5 - e, 0.5 + e)).abs() < 1e-4, "across the split");
        assert!((side(1.0 - e, 0.4) - side(1.0 + e, 0.4)).abs() < 1e-4, "across a quad edge");
        // the corners are the heights
        assert!((side(1.0, 1.0) - z[5]).abs() < 1e-9);
    }

    // spec: GEN-189 (reading 20, KH h4c): the clamped read stays within its quad's corners, equals h4 where h4 does, and keeps a monotone step edge monotone.
    #[test]
    fn clamped_read_stays_in_the_quad() {
        let (w, h) = (4, 4);
        let z: Vec<f64> = (0..w * h).map(|p| ((p * 37 % 11) as f64) * 10.0 + if p == 5 { 300.0 } else { 0.0 }).collect();
        let g = gradients(&z, w, h);
        let split = vec![MAIN; w * h];
        let step = |p: usize, q: usize| (p == 5 && q == 10) || (p == 10 && q == 5);
        let mut clamped = 0;
        for k in 0..=60 {
            for l in 0..=60 {
                let (fx, fy) = (k as f64 * 0.05, l as f64 * 0.05);
                let (v, lo, hi) = read_ct_clamped(&z, w, h, &split, &g, &step, fx, fy);
                assert!(v >= lo && v <= hi, "inside the quad's range at ({fx}, {fy})");
                let raw = read_ct(&z, w, h, &split, &g, &step, fx, fy).0;
                if raw != v {
                    clamped += 1;
                } else {
                    assert_eq!(raw, v);
                }
            }
        }
        assert!(clamped > 0, "the rough block overshoots somewhere");
        // along the step 5 -> 10 (the main diagonal of quad 5): non-increasing
        let mut prev = f64::INFINITY;
        for k in 0..=32 {
            let t = k as f64 / 32.0;
            let v = read_ct_clamped(&z, w, h, &split, &g, &step, 1.0 + t, 1.0 + t).0;
            assert!(v <= prev + 1e-9, "monotone along the step at t = {t}");
            prev = v;
        }
    }

    // spec: GEN-189 (reading 19, KH h3): the skeleton never raises, lowers to the step line on it, and leaves cells far from any step unchanged.
    #[test]
    fn skeleton_lowers_along_the_step_only() {
        let z = [100.0, 200.0, 200.0, 0.0];
        let diag = |q: usize| (q == 0).then_some((0usize, 3usize));
        let on = read_skeleton(&z, 2, 2, &diag, 150.0, 0.5, 0.5, 8.0, 1.0, 2.0);
        assert_eq!(on, 50.0);
        let far = read_skeleton(&z, 2, 2, &diag, 150.0, 1.0, 0.0, 8.0, 1.0, 2.0);
        assert_eq!(far, 150.0);
        let low = read_skeleton(&z, 2, 2, &diag, 10.0, 0.5, 0.5, 8.0, 1.0, 2.0);
        assert_eq!(low, 10.0, "never raised");
    }

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
