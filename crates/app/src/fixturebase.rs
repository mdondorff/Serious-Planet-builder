//! The base of the G-RT fixture (GEN-177 follow-up, architect 2026-11-12): the in-memory `--world` base cannot carry a commit baked over the 16384-node lattice (its bake is capped at 1,024 cells a face edge, so `B + L`
//! would put the ground hundreds of metres off its trench), and there is no global base store for a frame-time fixture. This lattice answers the fixture region's rectangle with the exact values of the production
//! lattice (`BakeInputs::base` at the nodes, f32 as stored) sampled once, before the first frame, and everywhere else the coarse in-memory lattice, read bilinearly at the finer node. A fixture, not the world: the
//! rectangle's edge is a seam between exact and coarse ground, far from the flight over the region (the output says so).

use std::sync::Arc;

use planet_core::{par, Face, Vec3};
use planet_generators::global::{GlobalSample, LatticeSource, ReadError};

/// A lattice of `res` cells a face edge that holds exact elevations over one rectangle of one face and reads `coarse` (a lattice with a whole-number divisor of `res` cells) elsewhere.
pub struct FixtureLattice {
    res: u32,
    face: Face,
    i0: u32,
    j0: u32,
    w: usize,
    h: usize,
    z: Vec<f32>,
    coarse: Arc<dyn LatticeSource + Send>,
}

impl FixtureLattice {
    /// Samples `base(direction(i, j))` for the nodes `i0 .. i0 + w`, `j0 .. j0 + h` of `face` on `threads` (a pure function of the node: the same values on any thread count). `direction` maps a node to its
    /// direction on the sphere (the store lattice is vertex-based: vertex `k` lies at `2k/res - 1` on the face).
    pub fn sample(
        res: u32,
        face: Face,
        rect: (u32, u32, usize, usize),
        direction: &(dyn Fn(u32, u32) -> Vec3 + Sync),
        base: &(dyn Fn(Vec3) -> f64 + Sync),
        coarse: Arc<dyn LatticeSource + Send>,
        threads: usize,
    ) -> Result<FixtureLattice, String> {
        let (i0, j0, w, h) = rect;
        let coarse_res = coarse.res();
        if coarse_res == 0 || coarse_res > res || !res.is_multiple_of(coarse_res) {
            return Err(format!("the coarse lattice of {coarse_res} cells does not divide the {res} cells of the fixture lattice"));
        }
        if w == 0 || h == 0 || i0 as usize + w > res as usize + 1 || j0 as usize + h > res as usize + 1 {
            return Err(format!("the rectangle {rect:?} lies outside the {} nodes of a face edge", res as usize + 1));
        }
        let z = par::map(&threads, w * h, &|c| base(direction(i0 + (c % w) as u32, j0 + (c / w) as u32)) as f32);
        Ok(FixtureLattice { res, face, i0, j0, w, h, z, coarse })
    }

    /// The nodes the rectangle holds.
    pub fn nodes(&self) -> usize {
        self.z.len()
    }

    fn inside(&self, face: Face, i: u32, j: u32) -> bool {
        face == self.face && i >= self.i0 && j >= self.j0 && ((i - self.i0) as usize) < self.w && ((j - self.j0) as usize) < self.h
    }
}

impl LatticeSource for FixtureLattice {
    fn res(&self) -> u32 {
        self.res
    }

    fn at(&self, face: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
        if i > self.res || j > self.res {
            return Err(ReadError::Source(format!("node ({}, {i}, {j}) is outside the {} cells of the lattice", face.0, self.res)));
        }
        // The coarse lattice at the finer node: bilinear in the elevation between its four vertices, the other channels of the nearest.
        let ratio = self.res / self.coarse.res();
        let (ci, cj) = (i / ratio, j / ratio);
        let (fi, fj) = (f64::from(i % ratio) / f64::from(ratio), f64::from(j % ratio) / f64::from(ratio));
        let coarse_res = self.coarse.res();
        let (ci1, cj1) = ((ci + 1).min(coarse_res), (cj + 1).min(coarse_res));
        let mut s = self.coarse.at(face, ci, cj)?;
        if self.inside(face, i, j) {
            s.elevation_m = self.z[(j - self.j0) as usize * self.w + (i - self.i0) as usize];
            return Ok(s);
        }
        if fi != 0.0 || fj != 0.0 {
            let (a, b, c, d) = (
                f64::from(s.elevation_m),
                f64::from(self.coarse.at(face, ci1, cj)?.elevation_m),
                f64::from(self.coarse.at(face, ci, cj1)?.elevation_m),
                f64::from(self.coarse.at(face, ci1, cj1)?.elevation_m),
            );
            s.elevation_m = (a * (1.0 - fi) * (1.0 - fj) + b * fi * (1.0 - fj) + c * (1.0 - fi) * fj + d * fi * fj) as f32;
        }
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A coarse lattice of 8 cells whose elevation is `10 i + 100 j` on face 2 and fails off it.
    struct Coarse;

    impl LatticeSource for Coarse {
        fn res(&self) -> u32 {
            8
        }
        fn at(&self, face: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
            if face != Face(2) || i > 8 || j > 8 {
                return Err(ReadError::Source(format!("coarse node ({}, {i}, {j})", face.0)));
            }
            Ok(GlobalSample { continent: 1.0, elevation_m: (10 * i + 100 * j) as f32, temperature_c: 5.0, precipitation_mm: 500.0 })
        }
    }

    fn direction(i: u32, j: u32) -> Vec3 {
        Vec3::new(f64::from(i), f64::from(j), 1.0)
    }

    fn base(d: Vec3) -> f64 {
        1000.0 + d.x * 3.0 + d.y * 7.0
    }

    fn lattice(threads: usize) -> FixtureLattice {
        FixtureLattice::sample(64, Face(2), (16, 24, 10, 6), &direction, &base, Arc::new(Coarse), threads).unwrap()
    }

    // spec: GEN-177
    #[test]
    fn the_rectangle_holds_the_exact_base_values_and_nothing_else_does() {
        let l = lattice(2);
        assert_eq!((l.res(), l.nodes()), (64, 60));
        for (i, j) in [(16, 24), (25, 29), (20, 27)] {
            let e = l.at(Face(2), i, j).unwrap().elevation_m;
            assert_eq!(e.to_bits(), (base(direction(i, j)) as f32).to_bits(), "node ({i}, {j}) is the base's");
        }
        // One node outside is the coarse read, not the base's.
        let out = l.at(Face(2), 26, 24).unwrap().elevation_m;
        assert_ne!(out.to_bits(), (base(direction(26, 24)) as f32).to_bits());
    }

    // spec: GEN-177
    #[test]
    fn outside_the_rectangle_the_coarse_lattice_is_read_bilinearly_at_the_finer_node() {
        let l = lattice(1);
        // The coarse elevation is linear, so the bilinear read at node (i, j) of 64 cells is the plane `10 i / 8 + 100 j / 8` exactly.
        for (i, j) in [(0, 0), (8, 8), (40, 56), (64, 64), (3, 5), (61, 2)] {
            let want = (10.0 * f64::from(i) / 8.0 + 100.0 * f64::from(j) / 8.0) as f32;
            let got = l.at(Face(2), i, j).unwrap().elevation_m;
            assert!((got - want).abs() < 1e-3, "node ({i}, {j}): {got} against {want}");
        }
        // The other channels are the coarse lattice's; a node past the lattice or on a face the coarse lattice lacks is an error.
        assert_eq!(l.at(Face(2), 3, 5).unwrap().precipitation_mm, 500.0);
        assert!(l.at(Face(2), 65, 0).is_err() && l.at(Face(3), 0, 0).is_err());
    }

    // spec: GEN-177
    #[test]
    fn the_sampling_is_the_same_on_one_and_eight_threads_and_bad_rectangles_are_refused() {
        let (a, b) = (lattice(1), lattice(8));
        assert!(a.z.iter().zip(&b.z).all(|(x, y)| x.to_bits() == y.to_bits()));
        let bad = |rect| FixtureLattice::sample(64, Face(2), rect, &direction, &base, Arc::new(Coarse), 1).is_err();
        assert!(bad((60, 0, 10, 4)), "past the edge");
        assert!(bad((0, 0, 0, 4)), "empty");
        assert!(FixtureLattice::sample(60, Face(2), (0, 0, 2, 2), &direction, &base, Arc::new(Coarse), 1).is_err(), "8 does not divide 60");
    }
}

#[cfg(test)]
mod rectangle_tests {
    use crate::worldfield::fixture_rectangle;
    use planet_cache::l_writer::CommitSpec;

    fn spec() -> CommitSpec {
        // The G-RT fixture's: 98,304 local cells over 16,384 nodes (6 cells a node), tiles of 768 and a halo of 64.
        CommitSpec {
            res: 16_384,
            cells: 98_304,
            tile: 768,
            halo: 64,
            radius_m: 6_371_000.0,
            seed: 14,
            sea_level_m: 0.0,
            canonical_flood: true,
            width_a: 7.2,
            width_b: 0.5,
            base_id: 1,
            inputs_id: 2,
        }
    }

    // spec: GEN-177
    #[test]
    fn the_rectangle_holds_every_tile_with_its_halo_and_a_tile_of_margin_and_the_c1_stencil() {
        // The G-RT fixture's tiles on face 1: ti 83..=93, tj 7..=17 (as its commit lists them).
        let tiles: Vec<(u8, u32, u32)> = (7..=17).flat_map(|tj| (83..=93).map(move |ti| (1u8, ti, tj))).collect();
        let (face, (i0, j0, w, h)) = fixture_rectangle(&spec(), tiles.iter().copied()).unwrap();
        assert_eq!(face.0, 1);
        // The local cell range of the tiles with halo: [83 * 768 - 64, 94 * 768 + 64); a node every 6 cells; a tile of margin (128 nodes) and 3 + 4 nodes of stencil.
        let (lo, hi) = ((83 * 768 - 64) / 6, (94 * 768 + 64) / 6);
        assert!(i64::from(i0) <= lo as i64 - 128 - 2, "the margin is a tile and the stencil is 3 nodes: {i0} against {lo}");
        assert!((i0 as usize + w) as i64 >= hi as i64 + 128 + 3, "the far side: {} against {hi}", i0 as usize + w);
        assert!(j0 > 0 && h > 0 && (j0 as usize + h) <= 16_385);
        // A face-edge commit is clamped to the face and a two-face commit is refused.
        let (_, (i0, _, w, _)) = fixture_rectangle(&spec(), [(1u8, 0u32, 0u32), (1, 1, 1)].into_iter()).unwrap();
        assert_eq!(i0, 0);
        assert!(i0 as usize + w <= 16_385);
        assert!(fixture_rectangle(&spec(), [(1u8, 5u32, 5u32), (2, 5, 5)].into_iter()).is_err());
        assert!(fixture_rectangle(&spec(), std::iter::empty()).is_err());
    }
}
