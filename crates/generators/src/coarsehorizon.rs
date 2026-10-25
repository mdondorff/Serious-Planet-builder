//! The coarse horizon from the global run (M8-5, GEN-045): for the cells of a lattice laid over the cube sphere, per azimuth the elevation angle of
//! the highest terrain of the coupled run along the great circle in that direction, out to a far range (about 50 km), so the shadow of a peak
//! tens of kilometres away reaches a tile before its chunk's own horizon (GEN-044, range 5 km) is there. A pure function of the stored heights
//! and the parameters, in a fixed order with `libm` (CON-14); rows of cells are independent, so the result does not depend on the thread count.
//!
//! The observers sit on a lattice of `m` cells per face edge (a divisor of the run's `n`, so the stored map stays small: 25 MB at 512 for the
//! 16 azimuths); the heights are read at the run's own resolution through the direction of each sample, which is how a march crosses a cube
//! face edge: no face coordinate is carried along the march (the same way the coupled run finds its neighbours).

use crate::coupled::Lattice;
use crate::horizon::quantize;
use planet_core::Vec3;

/// The most azimuths a map may hold (the map is `6 m^2` cells of that many bytes).
pub const MAX_AZIMUTHS: u32 = 256;

/// Version of the march and the byte it stores; it is part of the key of the stored map (STRM-045), so a change to what the march gives must bump it.
pub const COARSE_HORIZON_VERSION: u32 = 1;

/// How the coarse horizon is marched.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoarseParams {
    /// Evenly spaced azimuths from north (0), clockwise through east (a quarter of the way round).
    pub azimuths: u32,
    /// How far the march goes along the ground, metres.
    pub range_m: f64,
    /// Ground distance between samples, metres (about a cell of the run's lattice).
    pub step_m: f64,
    /// Radius of the body, metres.
    pub radius_m: f64,
}

impl CoarseParams {
    /// 16 azimuths to 50 km with samples one cell of the run apart: a cell of an `n`-cell face edge is about `pi R / (2 n)` across (4.9 km at
    /// 2048 cells on an Earth-sized body), and a sample reads the nearest cell, so a shorter step would put a neighbour's height too near.
    pub fn for_lattice(n: usize, radius_m: f64) -> CoarseParams {
        CoarseParams { azimuths: 16, range_m: 50_000.0, step_m: core::f64::consts::FRAC_PI_2 * radius_m / n as f64, radius_m }
    }
}

/// The unit tangent at `d` pointing along azimuth `az` (radians from north, clockwise): north is the projection of the pole axis on the
/// tangent plane, east the pole axis crossed with `d`. At a pole (`d` parallel to the pole axis) the y axis stands in for it: east is then +x and north +y at the north pole. The azimuths are in the geographic frame (north toward +z), unlike GEN-044's, which are in a chunk raster's face frame.
fn tangent(d: Vec3, az: f64) -> Vec3 {
    let (east, north) = geographic_frame(d);
    north * libm::cos(az) + east * libm::sin(az)
}

/// The unit `(east, north)` tangents at the unit direction `d` in the geographic frame the coarse horizon's azimuths are in (see [`tangent`]).
pub fn geographic_frame(d: Vec3) -> (Vec3, Vec3) {
    let pole = Vec3::new(0.0, 0.0, 1.0);
    let mut east = pole.cross(d);
    if east.length() < 1e-9 {
        east = Vec3::new(0.0, 1.0, 0.0).cross(d);
    }
    let east = east.normalized();
    (east, d.cross(east).normalized())
}

/// The elevation angle, from the observer at height `h0` on a sphere of radius `r`, of the point at height `h` an arc `s` along the ground:
/// the angle of the chord above the observer's horizontal plane.
pub(crate) fn elevation(r: f64, h0: f64, h: f64, s: f64) -> f64 {
    let theta = s / r;
    libm::atan2((r + h) * libm::cos(theta) - (r + h0), (r + h) * libm::sin(theta))
}

/// The coarse horizon bytes of every cell of an `m x m` lattice per face (faces in order, rows `j` outer, `i`, then azimuth): the stored form of
/// [`crate::horizon::quantize`]. `z` holds the heights of `run` (`run.len()` values; sea at or below 0 counts as 0, the sea's surface), `m` must
/// divide the run's lattice size. Refused with the cause named on a wrong size, a bad `m` or parameters that are not finite and positive.
pub fn coarse_horizon(run: &Lattice, z: &[f32], m: usize, p: &CoarseParams, threads: usize) -> Result<Vec<u8>, String> {
    let n = run.n();
    if z.len() != run.len() {
        return Err(format!("the run holds {} heights, not the {} of a lattice of {n} cells per face edge", z.len(), run.len()));
    }
    if m == 0 || m > n || !n.is_multiple_of(m) {
        return Err(format!("{m} observer cells per face edge: a divisor of the run's {n}"));
    }
    let finite_positive = |v: f64| v.is_finite() && v > 0.0;
    let ok = p.azimuths >= 1
        && p.azimuths <= MAX_AZIMUTHS
        && finite_positive(p.range_m)
        && finite_positive(p.step_m)
        && finite_positive(p.radius_m)
        && p.range_m / p.step_m <= 1.0e5
        && p.step_m <= p.range_m
        && p.range_m <= core::f64::consts::FRAC_PI_2 * p.radius_m;
    if !ok {
        return Err(format!(
            "bad coarse horizon parameters {p:?}: 1 to {MAX_AZIMUTHS} azimuths, finite and positive, a step of at most the range, at most 100,000 steps, a range of at most a quarter of the way round the body"
        ));
    }
    let obs = Lattice::new(m, p.radius_m);
    let a = p.azimuths as usize;
    // The last sample can fall just short of the range through rounding of the quotient: count it.
    let steps = (p.range_m / p.step_m + 1e-9).floor() as usize;
    let height = |dir: Vec3| f64::from(z[run.cell_at(dir)]).max(0.0);
    // An observer cell covers `k x k` cells of the run: its height is their mean (the sea's surface counts 0), not one arbitrary corner cell.
    let k = n / m;
    let observer_height = |cell: usize| -> f64 {
        let (face, rest) = (cell / (m * m), cell % (m * m));
        let (i, j) = (rest % m, rest / m);
        let mut sum = 0.0;
        for dj in 0..k {
            for di in 0..k {
                sum += f64::from(z[face * n * n + (j * k + dj) * n + i * k + di]).max(0.0);
            }
        }
        sum / (k * k) as f64
    };
    let cells = 6 * m * m;
    let mut out = vec![0u8; cells * a];
    let per = cells.div_ceil(threads.clamp(1, cells)).max(1);
    std::thread::scope(|s| {
        for (c, part) in out.chunks_mut(per * a).enumerate() {
            let (obs, height, observer_height) = (&obs, &height, &observer_height);
            planet_core::thread::spawn_named(s, "coarse-horizon", move || {
                for (r, bytes) in part.chunks_mut(a).enumerate() {
                    let d0 = obs.direction(c * per + r);
                    let h0 = observer_height(c * per + r);
                    for (k, byte) in bytes.iter_mut().enumerate() {
                        let az = core::f64::consts::TAU * k as f64 / f64::from(p.azimuths);
                        let t = tangent(d0, az);
                        let mut best = f64::NEG_INFINITY;
                        for step in 1..=steps {
                            let sarc = step as f64 * p.step_m;
                            let theta = sarc / p.radius_m;
                            let dir = d0 * libm::cos(theta) + t * libm::sin(theta);
                            best = best.max(elevation(p.radius_m, h0, height(dir), sarc));
                        }
                        *byte = if best.is_finite() { quantize(best) } else { 0 };
                    }
                }
            });
        }
    });
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::horizon::{dequantize, DEGREES_PER_STEP};

    /// A body of radius 100 km: the 64-cell lattice has cells of about 2.4 km.
    const R: f64 = 100_000.0;
    const N: usize = 64;

    fn params() -> CoarseParams {
        CoarseParams { azimuths: 16, range_m: 20_000.0, step_m: 2_000.0, radius_m: R }
    }

    /// Heights of a flat body with a plateau of `height` metres over the cells within `radius_m` of the cell `centre`.
    fn plateau(lat: &Lattice, centre: usize, radius_m: f64, height: f32) -> Vec<f32> {
        let c = lat.direction(centre);
        (0..lat.len()).map(|k| if R * libm::acos(lat.direction(k).dot(c).clamp(-1.0, 1.0)) <= radius_m { height } else { 0.0 }).collect()
    }

    /// The azimuth index nearest the direction from `from` toward `to`.
    fn azimuth_toward(from: Vec3, to: Vec3, a: u32) -> usize {
        let east = Vec3::new(0.0, 0.0, 1.0).cross(from).normalized();
        let north = from.cross(east).normalized();
        let az = libm::atan2(to.dot(east), to.dot(north));
        (az / core::f64::consts::TAU * f64::from(a)).round().rem_euclid(f64::from(a)) as usize % a as usize
    }

    // spec: GEN-045
    #[test]
    fn a_far_plateau_raises_the_horizon_of_cells_within_range_toward_it_and_not_beyond() {
        let run = Lattice::new(N, R);
        let p = params();
        // A plateau on the cube edge, so observers on the neighbouring face see it across the edge.
        let centre = N - 1 + (N / 2) * N; // face 0, last column, middle row
        let height = 3_000.0f32;
        let z = plateau(&run, centre, 4_000.0, height);
        let map = coarse_horizon(&run, &z, N, &p, 3).unwrap();
        let a = p.azimuths as usize;
        let c = run.direction(centre);
        let (mut near_seen, mut near_total, mut far_seen, mut across) = (0, 0, 0, 0);
        for cell in 0..run.len() {
            let d = run.direction(cell);
            let s = R * libm::acos(d.dot(c).clamp(-1.0, 1.0));
            let bytes = &map[cell * a..(cell + 1) * a];
            let toward = bytes[azimuth_toward(d, c, p.azimuths)];
            if (8_000.0..=16_000.0).contains(&s) {
                near_total += 1;
                // The plateau is wide enough that the sample line at the nearest azimuth meets it.
                near_seen += usize::from(toward > 0);
                if cell / (N * N) != 0 {
                    across += usize::from(toward > 0);
                }
                // Its angle is that of its top over the distance of the sample that meets it: between the nearest and the farthest edge.
                let lo = elevation(R, 0.0, f64::from(height), s + 4_000.0 + p.step_m);
                let hi = elevation(R, 0.0, f64::from(height), (s - 4_000.0 - p.step_m).max(1.0));
                let angle = dequantize(toward);
                assert!(
                    angle >= lo - 2.0 * DEGREES_PER_STEP.to_radians() && angle <= hi + 2.0 * DEGREES_PER_STEP.to_radians(),
                    "cell {cell} at {s:.0} m: {:.2} deg not in [{:.2}, {:.2}]",
                    angle.to_degrees(),
                    lo.to_degrees(),
                    hi.to_degrees()
                );
            }
            if s > p.range_m + 4_000.0 + p.step_m {
                far_seen += usize::from(bytes.iter().any(|&q| q > 0));
            }
        }
        assert!(near_total > 50 && near_seen * 10 >= near_total * 8, "{near_seen} of {near_total} cells in range see it");
        assert!(across > 5, "{across} cells on the other face see it across the edge");
        assert_eq!(far_seen, 0, "no cell beyond the range sees it");
    }

    // spec: GEN-045
    #[test]
    fn the_result_does_not_depend_on_the_thread_count_and_the_sea_counts_as_its_surface() {
        let run = Lattice::new(16, R);
        let z: Vec<f32> = (0..run.len())
            .map(|c| {
                let d = run.direction(c);
                (4000.0 * (d.x * d.y + 0.5 * d.z) - 800.0) as f32
            })
            .collect();
        let p = CoarseParams { range_m: 40_000.0, ..params() };
        let one = coarse_horizon(&run, &z, 8, &p, 1).unwrap();
        assert!(one.iter().any(|&q| q > 0));
        for threads in [2, 5, 1000] {
            assert_eq!(coarse_horizon(&run, &z, 8, &p, threads).unwrap(), one, "{threads} threads");
        }
        // A deep sea is the sea's surface (0), not a pit: replacing every negative height by another negative one changes nothing.
        let deeper: Vec<f32> = z.iter().map(|&h| if h < 0.0 { h - 500.0 } else { h }).collect();
        assert_eq!(coarse_horizon(&run, &deeper, 8, &p, 2).unwrap(), one);
        // A finer observer lattice has the same horizon where the observers coincide with the run's cells.
        let fine = coarse_horizon(&run, &z, 16, &p, 2).unwrap();
        assert_eq!(fine.len(), 6 * 16 * 16 * 16);
        assert_eq!(one.len(), 6 * 8 * 8 * 16);
    }

    // spec: GEN-045
    #[test]
    fn bad_input_is_refused_with_the_cause() {
        let run = Lattice::new(8, R);
        let z = vec![0.0f32; run.len()];
        let p = params();
        assert!(coarse_horizon(&run, &z[1..], 8, &p, 1).unwrap_err().contains("heights"));
        for m in [0usize, 3, 16] {
            assert!(coarse_horizon(&run, &z, m, &p, 1).unwrap_err().contains("divisor"), "m {m}");
        }
        for bad in [
            CoarseParams { azimuths: 0, ..p },
            CoarseParams { range_m: f64::NAN, ..p },
            CoarseParams { step_m: 0.0, ..p },
            CoarseParams { radius_m: -1.0, ..p },
            CoarseParams { range_m: 1.0e12, ..p },
        ] {
            assert!(coarse_horizon(&run, &z, 8, &bad, 1).unwrap_err().contains("bad coarse horizon"), "{bad:?}");
        }
        // A flat body has no horizon above 0.
        assert!(coarse_horizon(&run, &z, 8, &p, 1).unwrap().iter().all(|&q| q == 0));
        // Limits: azimuths, a step beyond the range, a range past a quarter of the way round, and the lattice's own step on a small body.
        for bad in [
            CoarseParams { azimuths: u32::MAX, ..p },
            CoarseParams { azimuths: MAX_AZIMUTHS + 1, ..p },
            CoarseParams { range_m: p.step_m * 0.5, ..p },
            CoarseParams { range_m: core::f64::consts::PI * R, ..p },
            CoarseParams { range_m: 50_000.0, ..CoarseParams::for_lattice(8, 7_958.0) },
        ] {
            assert!(coarse_horizon(&run, &z, 8, &bad, 1).unwrap_err().contains("bad coarse horizon"), "{bad:?}");
        }
        let ok = CoarseParams::for_lattice(64, 6_371_000.0);
        assert!(
            (ok.step_m - core::f64::consts::FRAC_PI_2 * 6_371_000.0 / 64.0).abs() < 1e-9 && ok.azimuths == 16 && ok.range_m == 50_000.0
        );
    }

    // spec: GEN-045
    #[test]
    fn the_chord_angle_is_exact_on_a_bare_sphere_and_flat_in_the_small() {
        // On a bare sphere the chord to a point an arc s away sits s / (2 R) below the horizontal.
        for s in [100.0, 5_000.0, 40_000.0] {
            assert!((elevation(R, 0.0, 0.0, s) + s / (2.0 * R)).abs() < 1e-9 * (s / R).max(1e-3), "s {s}");
        }
        // Small compared with the body: the plain angle of the rise over the distance.
        let (h, s) = (50.0, 200.0);
        assert!((elevation(6_371_000.0, 0.0, h, s) - libm::atan(h / s)).abs() < 1e-4);
        assert!(elevation(R, 100.0, 100.0, 3_000.0) < 0.0, "the same height is below the horizon of an observer on it");
    }

    /// The bytes of the single observer `cell` of an `m`-cell lattice, marched by hand with the module's primitives.
    fn one_observer(run: &Lattice, z: &[f32], m: usize, cell: usize, p: &CoarseParams) -> Vec<u8> {
        let (n, obs) = (run.n(), Lattice::new(m, p.radius_m));
        let k = n / m;
        let (face, rest) = (cell / (m * m), cell % (m * m));
        let (i, j) = (rest % m, rest / m);
        let mut sum = 0.0;
        for dj in 0..k {
            for di in 0..k {
                sum += f64::from(z[face * n * n + (j * k + dj) * n + i * k + di]).max(0.0);
            }
        }
        let h0 = sum / (k * k) as f64;
        let d0 = obs.direction(cell);
        (0..p.azimuths)
            .map(|a| {
                let t = tangent(d0, core::f64::consts::TAU * f64::from(a) / f64::from(p.azimuths));
                let mut best = f64::NEG_INFINITY;
                let mut s = p.step_m;
                while s <= p.range_m + 1e-6 {
                    let th = s / p.radius_m;
                    let dir = d0 * libm::cos(th) + t * libm::sin(th);
                    best = best.max(elevation(p.radius_m, h0, f64::from(z[run.cell_at(dir)]).max(0.0), s));
                    s += p.step_m;
                }
                quantize(best)
            })
            .collect()
    }

    // spec: GEN-045
    #[test]
    fn a_coarser_observer_lattice_takes_the_mean_height_of_the_cells_it_covers_and_keeps_the_order_of_cells() {
        let run = Lattice::new(16, R);
        let z: Vec<f32> = (0..run.len())
            .map(|c| {
                let d = run.direction(c);
                (3000.0 * (d.x * d.y + 0.4 * d.z * d.x) + 600.0 * ((c % 3) as f64)) as f32
            })
            .collect();
        let p = CoarseParams { range_m: 30_000.0, step_m: 6_000.0, ..params() };
        for m in [4usize, 8, 16] {
            let map = coarse_horizon(&run, &z, m, &p, 3).unwrap();
            let a = p.azimuths as usize;
            for cell in (0..6 * m * m).step_by(7) {
                assert_eq!(&map[cell * a..(cell + 1) * a], one_observer(&run, &z, m, cell, &p).as_slice(), "m {m}, cell {cell}");
            }
        }
    }

    // spec: GEN-045
    #[test]
    fn azimuth_zero_is_north_toward_plus_z_and_a_quarter_turn_is_east_and_a_polar_observer_has_a_frame() {
        let run = Lattice::new(N, R);
        let p = params();
        let a = p.azimuths as usize;
        let plateau_at = |dir: Vec3| plateau(&run, run.cell_at(dir), 4_000.0, 3_000.0);
        let theta = 12_000.0 / R;
        // An observer at +x on the equator: a plateau toward +z lights azimuth 0, one toward +y (east: z cross x) azimuth 4.
        let obs = run.cell_at(Vec3::new(1.0, 0.0, 0.0));
        let north = coarse_horizon(&run, &plateau_at(Vec3::new(libm::cos(theta), 0.0, libm::sin(theta))), N, &p, 2).unwrap();
        let east = coarse_horizon(&run, &plateau_at(Vec3::new(libm::cos(theta), libm::sin(theta), 0.0)), N, &p, 2).unwrap();
        let b = |m: &Vec<u8>, k: usize| m[obs * a + k];
        assert!(
            b(&north, 0) > 0 && b(&north, 4) == 0 && b(&north, 8) == 0 && b(&north, 12) == 0,
            "north: {:?}",
            &north[obs * a..(obs + 1) * a]
        );
        assert!(b(&east, 4) > 0 && b(&east, 0) == 0 && b(&east, 8) == 0 && b(&east, 12) == 0, "east: {:?}", &east[obs * a..(obs + 1) * a]);
        // At the north pole (an odd lattice has a cell centred on it) east is +x and north +y.
        let run = Lattice::new(15, R);
        let pole = run.cell_at(Vec3::new(0.0, 0.0, 1.0));
        let toward_x = Vec3::new(libm::sin(theta), 0.0, libm::cos(theta));
        let z = plateau(&run, run.cell_at(toward_x), 4_000.0, 3_000.0);
        let map = coarse_horizon(&run, &z, 15, &p, 1).unwrap();
        assert!(map[pole * a + 4] > 0, "toward +x is east at the pole: {:?}", &map[pole * a..(pole + 1) * a]);
        assert_eq!(map[pole * a + 12], 0, "and nothing to the west");
    }
}
