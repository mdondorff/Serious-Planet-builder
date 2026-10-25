//! The coupled run's starting heights from a bake of any finer resolution (M8-1 part c, GEN-056): the run's lattice (`n` cells per face edge)
//! is independent of the bake's (`res` cells, `res` a multiple of `n`), so a 2048-cell run starts from a 16384-cell bake.
//!
//! The reduction is a box filter. A bake cell holds the mean of its four corner vertices; a run cell holds the mean of the `r x r` bake cells
//! (`r = res / n`) it covers, which weighs the vertices 1 at the corners, 2 along the edges and 4 inside, over `4 r^2`. Both lattices are
//! uniform in the face coordinates `(s, t)`, so the cells nest exactly. The sum is in `f64` in one fixed order (rows, then columns) and is
//! rounded to `f32` once, so the result is bit-exact for any thread count (CON-14).

use crate::global::{LatticeSource, ReadError};
use planet_core::cube::Face;
use planet_core::hash::mix64;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// Version of the reduction; it is part of the identity of the run's inputs (STRM-043), so a change that alters the heights must bump it.
pub const REDUCTION_VERSION: u32 = 1;

/// The mean elevation of every cell of the run's lattice of `n` cells per face edge, from `src`: cell `f * n^2 + j * n + i`, `i` along the
/// face's `s` axis (the order of `coupled::Lattice`). `threads` only sets the speed. A read error names the vertex; `res` that is not a
/// multiple of `n` is refused.
pub fn reduce_bake(src: &dyn LatticeSource, n: u32, threads: usize) -> Result<Vec<f32>, ReadError> {
    let res = src.res();
    if n == 0 || res < n || !res.is_multiple_of(n) || !(res / n).is_power_of_two() {
        return Err(ReadError::Shape(format!(
            "a coupled run of {n} cells per face edge cannot start from a bake of {res} cells: the run must divide the bake"
        )));
    }
    let (r, n) = ((res / n) as usize, n as usize);
    // Bands of rows that fall in one block row of the source, so each block is fetched about once.
    let rows_per_band = if src.block_cells() > 0 { (src.block_cells() as usize / r).max(1) } else { 1 };
    let bands_per_face = n.div_ceil(rows_per_band);
    let bands = 6 * bands_per_face;
    let next = AtomicUsize::new(0);
    let done: Mutex<Vec<Band>> = Mutex::new(Vec::with_capacity(bands));
    std::thread::scope(|s| {
        for _ in 0..threads.clamp(1, bands) {
            s.spawn(|| loop {
                let b = next.fetch_add(1, Ordering::Relaxed);
                if b >= bands {
                    break;
                }
                let face = Face((b / bands_per_face) as u8);
                let j0 = (b % bands_per_face) * rows_per_band;
                let out = band(src, face, j0, (j0 + rows_per_band).min(n), n, r);
                done.lock().expect("reduction results").push((b, out));
            });
        }
    });
    let mut parts = done.into_inner().expect("reduction results");
    parts.sort_by_key(|p| p.0);
    let mut z = Vec::with_capacity(6 * n * n);
    for (_, part) in parts {
        z.extend(part?);
    }
    Ok(z)
}

/// The rows of one band, or why they could not be read, with the band's index.
type Band = (usize, Result<Vec<f32>, ReadError>);

/// Rows `j0..j1` of one face, each row `n` cells.
fn band(src: &dyn LatticeSource, face: Face, j0: usize, j1: usize, n: usize, r: usize) -> Result<Vec<f32>, ReadError> {
    let weight = |k: usize| if k == 0 || k == r { 1.0 } else { 2.0 };
    let norm = 4.0 * (r * r) as f64;
    let mut out = Vec::with_capacity((j1 - j0) * n);
    let mut row_sum = vec![0.0f64; n];
    let mut acc = vec![0.0f64; n];
    for j in j0..j1 {
        acc.iter_mut().for_each(|a| *a = 0.0);
        for vj in 0..=r {
            // One vertex row of the bake, read once for the whole width; each cell sums its own `r + 1` vertices with the end weights.
            let mut v = Vec::with_capacity(n * r + 1);
            for vi in 0..=n * r {
                let z = src.at(face, vi as u32, (j * r + vj) as u32).map_err(|e| ReadError::Source(format!("face {}: {e}", face.0)))?;
                v.push(f64::from(z.elevation_m));
            }
            for i in 0..n {
                let mut s = 0.0;
                for k in 0..=r {
                    s += weight(k) * v[i * r + k];
                }
                row_sum[i] = s;
            }
            let wj = weight(vj);
            for i in 0..n {
                acc[i] += wj * row_sum[i];
            }
        }
        out.extend(acc.iter().map(|a| (a / norm) as f32));
    }
    Ok(out)
}

/// The identity of the run's inputs when they are declared and not stored: the reduction, the planet radius, and the constant uplift (m per
/// year) and erodibility. The heights themselves are a function of the bake (whose stage key is in the key of the stored run) and of the
/// reduction, so a stored run is found without reading the bake. Per-cell driver maps (ADR 0076) will carry their own identity.
pub fn declared_inputs_hash(radius_m: f64, uplift: f32, erodibility: f32) -> u64 {
    let mut h = mix64(0x626b_5f72_6564_7563 ^ u64::from(REDUCTION_VERSION));
    for bits in [radius_m.to_bits(), u64::from(uplift.to_bits()), u64::from(erodibility.to_bits())] {
        h = mix64(h ^ bits);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coupled::{run, CoupledInput, CoupledParams, Lattice};
    use crate::global::GlobalSample;

    /// A bake whose elevation is a function of the vertex, on exactly representable values so every sum is exact in any order.
    struct Synthetic {
        res: u32,
        block: u32,
        fail_at: Option<(u32, u32)>,
    }

    fn height(face: u8, i: u32, j: u32) -> f32 {
        // Multiples of 1/8 below 2^12: any sum of a few million of them is exact in f64.
        (((u32::from(face) * 7919 + i * 31 + j * 17 + (i * j) % 13) % 4096) as f32) * 0.125 - 100.0
    }

    impl LatticeSource for Synthetic {
        fn res(&self) -> u32 {
            self.res
        }
        fn at(&self, face: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
            if self.fail_at == Some((i, j)) {
                return Err(ReadError::Source(format!("vertex ({i}, {j}) is missing")));
            }
            Ok(GlobalSample { continent: 0.0, elevation_m: height(face.0, i, j), temperature_c: 0.0, precipitation_mm: 0.0 })
        }
        fn block_cells(&self) -> u32 {
            self.block
        }
    }

    /// The definition, written out for one cell: the mean over its bake cells of the mean of each cell's four corners.
    fn by_definition(res: u32, n: u32, face: usize, i: usize, j: usize) -> f32 {
        let r = (res / n) as usize;
        let mut total = 0.0f64;
        for q in 0..r {
            for p in 0..r {
                let (x, y) = ((i * r + p) as u32, (j * r + q) as u32);
                let corners = [(x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1)];
                total += corners.iter().map(|&(a, b)| f64::from(height(face as u8, a, b))).sum::<f64>() / 4.0;
            }
        }
        (total / (r * r) as f64) as f32
    }

    fn pre_reduced(res: u32, n: u32) -> Vec<f32> {
        let n_us = n as usize;
        let mut z = Vec::new();
        for face in 0..6 {
            for j in 0..n_us {
                for i in 0..n_us {
                    z.push(by_definition(res, n, face, i, j));
                }
            }
        }
        z
    }

    // spec: GEN-056
    #[test]
    fn a_run_cell_is_the_mean_of_the_bake_cells_it_covers_for_every_ratio() {
        for (res, n) in [(16u32, 16u32), (16, 8), (32, 8), (64, 8), (64, 2), (8, 1)] {
            let got = reduce_bake(&Synthetic { res, block: 0, fail_at: None }, n, 2).unwrap();
            assert_eq!(got, pre_reduced(res, n), "bake {res} to run {n}");
        }
    }

    // spec: GEN-056
    #[test]
    fn a_plane_reduces_to_the_plane_at_the_cell_centre() {
        struct Plane(u32);
        impl LatticeSource for Plane {
            fn res(&self) -> u32 {
                self.0
            }
            fn at(&self, _: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
                Ok(GlobalSample {
                    continent: 0.0,
                    elevation_m: 0.5 * i as f32 + 0.25 * j as f32 + 3.0,
                    temperature_c: 0.0,
                    precipitation_mm: 0.0,
                })
            }
        }
        let (res, n) = (32u32, 4u32);
        let z = reduce_bake(&Plane(res), n, 3).unwrap();
        let r = (res / n) as f32;
        for (c, v) in z.iter().enumerate() {
            let (i, j) = ((c % 4) as f32, ((c / 4) % 4) as f32);
            // The centre of run cell (i, j) is at bake vertex coordinates ((i + 0.5) r, (j + 0.5) r).
            let want = 0.5 * (i + 0.5) * r + 0.25 * (j + 0.5) * r + 3.0;
            assert!((v - want).abs() < 1e-4, "cell {c}: {v} against {want}");
        }
    }

    // spec: GEN-056
    #[test]
    fn the_reduction_is_bit_equal_for_every_thread_count_and_block_structure() {
        let want = reduce_bake(&Synthetic { res: 64, block: 0, fail_at: None }, 8, 1).unwrap();
        for (threads, block) in [(2usize, 0u32), (5, 16), (8, 8), (64, 32), (3, 64)] {
            assert_eq!(
                reduce_bake(&Synthetic { res: 64, block, fail_at: None }, 8, threads).unwrap(),
                want,
                "{threads} threads, blocks of {block}"
            );
        }
    }

    /// Heights with a full mantissa, so that a sum depends on its order.
    struct Noisy(u32);

    impl LatticeSource for Noisy {
        fn res(&self) -> u32 {
            self.0
        }
        fn at(&self, face: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
            let h = mix64(u64::from(face.0) << 40 ^ u64::from(i) << 20 ^ u64::from(j));
            let elevation_m = (h >> 40) as f32 / 16_777_216.0 * 9000.0 - 3000.0;
            Ok(GlobalSample { continent: 0.0, elevation_m, temperature_c: 0.0, precipitation_mm: 0.0 })
        }
        fn block_cells(&self) -> u32 {
            16
        }
    }

    // spec: GEN-056
    #[test]
    fn heights_whose_sum_depends_on_its_order_still_reduce_bit_equal_for_every_thread_count() {
        let want = reduce_bake(&Noisy(128), 16, 1).unwrap();
        assert!(want.iter().any(|v| v.fract() != 0.0 && (v * 8.0).fract() != 0.0), "the heights are not on a coarse grid");
        for threads in [2usize, 3, 8, 96] {
            let got = reduce_bake(&Noisy(128), 16, threads).unwrap();
            assert!(got.iter().zip(&want).all(|(a, b)| a.to_bits() == b.to_bits()), "{threads} threads");
        }
    }

    // spec: GEN-056
    #[test]
    fn the_reduction_of_a_fixed_bake_is_pinned_to_its_version() {
        // A change that alters the heights must bump REDUCTION_VERSION (it re-keys every stored run, STRM-043): update both together.
        let z = reduce_bake(&Noisy(64), 8, 2).unwrap();
        let h = z.iter().fold(0u64, |h, v| mix64(h ^ u64::from(v.to_bits())));
        assert_eq!((REDUCTION_VERSION, h), (1, PINNED), "the reduction changed: bump REDUCTION_VERSION and pin the new hash");
    }

    const PINNED: u64 = 9473277785721432640;

    // spec: GEN-056
    #[test]
    fn a_run_from_a_finer_bake_equals_the_run_from_the_pre_reduced_heights() {
        let (n, steps) = (8u32, 3usize);
        let lat = Lattice::new(n as usize, planet_core::EARTH_RADIUS_M);
        let make = |z: Vec<f32>| {
            let len = z.len();
            CoupledInput { z, uplift: vec![1.0e-3; len], erodibility: vec![2.0e-5; len] }
        };
        let params = CoupledParams::default();
        for res in [32u32, 64] {
            let from_bake = run(&lat, &make(reduce_bake(&Synthetic { res, block: 16, fail_at: None }, n, 4).unwrap()), &params, steps);
            let from_heights = run(&lat, &make(pre_reduced(res, n)), &params, steps);
            assert_eq!(from_bake.z, from_heights.z, "bake {res}: heights");
            assert_eq!(from_bake.area_m2, from_heights.area_m2, "bake {res}: drainage area");
            assert_eq!(from_bake.receiver, from_heights.receiver, "bake {res}: receivers");
        }
    }

    // spec: GEN-056
    #[test]
    fn a_run_that_does_not_divide_the_bake_is_refused_and_a_missing_vertex_is_named() {
        for (res, n) in [(16u32, 3u32), (16, 32), (16, 0)] {
            let e = reduce_bake(&Synthetic { res, block: 0, fail_at: None }, n, 1).unwrap_err();
            assert!(matches!(e, ReadError::Shape(_)), "bake {res}, run {n}");
        }
        let e = reduce_bake(&Synthetic { res: 16, block: 4, fail_at: Some((5, 9)) }, 4, 3).unwrap_err();
        assert!(e.to_string().contains("face"), "{e}");
        assert!(e.to_string().contains("(5, 9)"), "{e}");
    }

    // spec: GEN-056
    #[test]
    fn the_declared_identity_of_the_inputs_changes_with_each_part() {
        let base = declared_inputs_hash(6.4e6, 1.0e-3, 2.0e-5);
        assert_eq!(base, declared_inputs_hash(6.4e6, 1.0e-3, 2.0e-5));
        for other in [
            declared_inputs_hash(6.5e6, 1.0e-3, 2.0e-5),
            declared_inputs_hash(6.4e6, 2.0e-3, 2.0e-5),
            declared_inputs_hash(6.4e6, 1.0e-3, 3.0e-5),
        ] {
            assert_ne!(base, other);
        }
    }
}
