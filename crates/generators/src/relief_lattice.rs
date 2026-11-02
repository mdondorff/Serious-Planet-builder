//! The relief channels read at a direction and over a cap (GEN-099, GEN-102): the bilinear read of `k` and `m` (each term one read at the point) and the ranges of
//! the blocks a cap touches, from a pyramid of the block ranges, so that the bounds of nested caps nest (LOD-012).
//!
//! The reader owns the channels ([`ReliefChannels`], computed by the bake or loaded from the store) and an identity that enters the keys of a field that reads it.

use crate::relief_channels::{ReliefChannels, BLOCK_CELLS};
use planet_core::cube::{direction_to_face, face_to_direction, TangentWarp};
use planet_core::{Face, Vec3};

/// The smallest node, in cells, a range over a cap is taken from below the blocks (2 cells: 9 lattice points).
const MIN_NODE_CELLS: usize = 2;

/// The channels of a bake read at directions and over caps.
pub struct ReliefLattice {
    channels: ReliefChannels,
    /// The ranges `[k_min, k_max, m_min, m_max]` per level of the pyramid and face: level 0 the blocks, level `l` the `2^l x 2^l` blocks, row-major.
    levels: Vec<Vec<Vec<[f32; 4]>>>,
    id: u64,
}

fn merge(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [a[0].min(b[0]), a[1].max(b[1]), a[2].min(b[2]), a[3].max(b[3])]
}

impl ReliefLattice {
    /// The reader of `channels`; `id` names the channels' content for the keys of a field that reads it (the caller derives it from the stored channels' key or hash).
    pub fn new(channels: ReliefChannels, id: u64) -> ReliefLattice {
        let n = channels.cells();
        for f in 0..6u8 {
            let planes = channels.face_planes(Face(f));
            assert!(
                planes.k.iter().chain(planes.m).all(|v| v.is_finite()),
                "the relief channels of face {f} hold a value that is not finite (a failed read of the bake?): a block range would leave it out"
            );
        }
        assert!(n >= BLOCK_CELLS, "the channels have at least {BLOCK_CELLS} cells per face edge");
        let b = channels.cells() / BLOCK_CELLS;
        let mut levels: Vec<Vec<Vec<[f32; 4]>>> = Vec::new();
        levels.push((0..6u8).map(|f| (0..b * b).map(|i| channels.block_range(Face(f), i % b, i / b)).collect()).collect());
        let mut side = b;
        while side > 1 {
            let (prev, half) = (levels.last().expect("a level"), side / 2);
            let next: Vec<Vec<[f32; 4]>> = (0..6)
                .map(|f| {
                    (0..half * half)
                        .map(|i| {
                            let (x, y) = (2 * (i % half), 2 * (i / half));
                            let p = &prev[f];
                            merge(merge(p[y * side + x], p[y * side + x + 1]), merge(p[(y + 1) * side + x], p[(y + 1) * side + x + 1]))
                        })
                        .collect()
                })
                .collect();
            levels.push(next);
            side = half;
        }
        ReliefLattice { channels, levels, id }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn cells(&self) -> usize {
        self.channels.cells()
    }

    /// The channels this reader holds.
    pub fn channels(&self) -> &ReliefChannels {
        &self.channels
    }

    fn locate(&self, s: f64) -> (usize, f64) {
        let n = self.channels.cells();
        let x = ((s + 1.0) * 0.5 * n as f64).clamp(0.0, n as f64);
        let i = (x as usize).min(n - 1);
        (i, x - i as f64)
    }

    /// The tapered `k` and the ring mean `m` at the unit direction `d`: the bilinear interpolant (in f64) of the stored f32 points, continuous across face edges.
    pub fn at(&self, d: Vec3) -> (f64, f64) {
        let (face, s, t) = direction_to_face(&TangentWarp, d);
        let ((i, fx), (j, fy)) = (self.locate(s), self.locate(t));
        let p = |ii: usize, jj: usize| self.channels.at(face, ii, jj);
        let (p00, p10, p01, p11) = (p(i, j), p(i + 1, j), p(i, j + 1), p(i + 1, j + 1));
        let lerp = |a: f32, b: f32, c: f32, e: f32| {
            let (lo, hi) = (f64::from(a) + (f64::from(b) - f64::from(a)) * fx, f64::from(c) + (f64::from(e) - f64::from(c)) * fx);
            lo + (hi - lo) * fy
        };
        (lerp(p00.0, p10.0, p01.0, p11.0), lerp(p00.1, p10.1, p01.1, p11.1))
    }

    /// The range `[k_min, k_max, m_min, m_max]` over the whole planet.
    pub fn global_range(&self) -> [f64; 4] {
        let top = self.levels.last().expect("a level");
        let r = top.iter().flatten().fold([f32::INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::NEG_INFINITY], |a, b| merge(a, *b));
        r.map(f64::from)
    }

    /// The range `[k_min, k_max, m_min, m_max]` that holds `k` and `m` at every direction within `angular_radius` of `centre` and a bilinear read there (GEN-102):
    /// the union of the ranges of the blocks (and of the pyramid nodes wholly inside the cap) the cap touches, on every face it reaches. The set of blocks only
    /// grows with the radius, so the ranges of nested caps nest.
    pub fn range_over_cap(&self, centre: Vec3, angular_radius: f64) -> [f64; 4] {
        let mut acc = [f32::INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::NEG_INFINITY];
        let top = self.levels.len() - 1;
        for f in 0..6u8 {
            self.descend(Face(f), top, 0, 0, centre, angular_radius * (1.0 + 1e-9) + 1e-12, &mut acc);
        }
        acc.map(f64::from)
    }

    /// The direction of face coordinates `(s, t)`.
    fn dir(face: Face, s: f64, t: f64) -> Vec3 {
        face_to_direction(&TangentWarp, face, s.clamp(-1.0, 1.0), t.clamp(-1.0, 1.0))
    }

    /// The node `(x, y)` of level `level` of `face` against the cap: skipped when apart, taken whole when inside, else split down to the blocks.
    #[allow(clippy::too_many_arguments)]
    fn descend(&self, face: Face, level: usize, x: usize, y: usize, centre: Vec3, radius: f64, acc: &mut [f32; 4]) {
        let b = self.channels.cells() / BLOCK_CELLS;
        let side = (1usize << level) as f64;
        // The face coordinates of the node's square: a block is 2 / b wide, a node of level `l` is `2^l` blocks.
        let w = 2.0 / b as f64 * side;
        let (s0, t0) = (-1.0 + x as f64 * w, -1.0 + y as f64 * w);
        let c = Self::dir(face, s0 + 0.5 * w, t0 + 0.5 * w);
        // The corners bound the square on the sphere (its sides are great-circle arcs of the tangent-warped map, so the farthest point from the node's centre is a corner).
        let corner_radius = [(s0, t0), (s0 + w, t0), (s0, t0 + w), (s0 + w, t0 + w)]
            .iter()
            .map(|&(s, t)| c.angle_to(Self::dir(face, s, t)))
            .fold(0.0, f64::max)
            * (1.0 + 1e-9)
            + 1e-12;
        let apart = c.angle_to(centre);
        if apart > radius + corner_radius {
            return;
        }
        let nb = b >> level;
        if apart + corner_radius <= radius {
            *acc = merge(*acc, self.levels[level][face.0 as usize][y * nb + x]);
            return;
        }
        if level == 0 {
            // A block the cap only touches: below it the ranges come from the lattice points of the cells the cap touches, so that a small cap's bounds do not carry the whole block's range.
            self.descend_cells(face, x * BLOCK_CELLS, y * BLOCK_CELLS, BLOCK_CELLS, centre, radius, acc);
            return;
        }
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            self.descend(face, level - 1, 2 * x + dx, 2 * y + dy, centre, radius, acc);
        }
    }

    /// The node of `size` x `size` cells at cell `(ci, cj)` of `face` against the cap, down to [`MIN_NODE_CELLS`]: its range is the min and max of its lattice points.
    #[allow(clippy::too_many_arguments)]
    fn descend_cells(&self, face: Face, ci: usize, cj: usize, size: usize, centre: Vec3, radius: f64, acc: &mut [f32; 4]) {
        let n = self.channels.cells() as f64;
        let w = 2.0 * size as f64 / n;
        let (s0, t0) = (-1.0 + 2.0 * ci as f64 / n, -1.0 + 2.0 * cj as f64 / n);
        let c = Self::dir(face, s0 + 0.5 * w, t0 + 0.5 * w);
        let corner_radius = [(s0, t0), (s0 + w, t0), (s0, t0 + w), (s0 + w, t0 + w)]
            .iter()
            .map(|&(s, t)| c.angle_to(Self::dir(face, s, t)))
            .fold(0.0, f64::max)
            * (1.0 + 1e-9)
            + 1e-12;
        let apart = c.angle_to(centre);
        if apart > radius + corner_radius {
            return;
        }
        if size <= MIN_NODE_CELLS {
            for j in cj..=cj + size {
                for i in ci..=ci + size {
                    let (k, m) = self.channels.at(face, i, j);
                    *acc = merge(*acc, [k, k, m, m]);
                }
            }
            return;
        }
        let half = size / 2;
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            self.descend_cells(face, ci + dx * half, cj + dy * half, half, centre, radius, acc);
        }
    }
}
