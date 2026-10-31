//! GEN-089..GEN-092: the terrain descriptors on synthetic rasters (change `terrain-scorecard`).

use planet_generators::descriptors::{self as d, form, Raster};

fn build(w: usize, h: usize, f: impl Fn(f64, f64) -> f64) -> Vec<f64> {
    // x and y in cell units from the raster centre.
    let (cx, cy) = ((w / 2) as f64, (h / 2) as f64);
    (0..w * h).map(|k| f((k % w) as f64 - cx, (k / w) as f64 - cy)).collect()
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

// spec: GEN-089
#[test]
fn slope_of_a_plane_rising_ten_degrees_east_is_ten_degrees_at_every_counted_cell() {
    let h = 30.0;
    let tan = libm::tan(10f64.to_radians());
    let z = build(40, 30, |x, _| tan * x * h);
    let s = d::slope_degrees(&Raster::new(&z, 40, 30, h), 3);
    let mut counted = 0;
    for (k, v) in s.iter().enumerate() {
        let (i, j) = (k % 40, k / 40);
        if i == 0 || j == 0 || i == 39 || j == 29 {
            assert!(v.is_nan(), "edge cell ({i}, {j}) is uncounted");
        } else {
            counted += 1;
            assert!((v - 10.0).abs() < 1e-9, "cell ({i}, {j}): {v} degrees");
        }
    }
    assert_eq!(counted, 38 * 28);
}

// spec: GEN-089
#[test]
fn a_no_data_cell_uncounts_the_slope_of_its_window() {
    let mut z = build(9, 9, |x, _| x);
    z[4 * 9 + 4] = f64::NAN;
    let s = d::slope_degrees(&Raster::new(&z, 9, 9, 1.0), 1);
    assert!(s[3 * 9 + 3].is_nan() && s[5 * 9 + 5].is_nan() && s[4 * 9 + 4].is_nan());
    assert!(!s[2 * 9 + 2].is_nan() && !s[6 * 9 + 6].is_nan());
}

// spec: GEN-089
#[test]
fn curvatures_of_a_paraboloid_and_a_trough_follow_the_closed_form_on_and_off_the_axis() {
    let (h, a) = (30.0, 1e-3);
    let (w, hh) = (41, 41);
    let paraboloid = build(w, hh, |x, y| a * ((x * h) * (x * h) + (y * h) * (y * h)));
    let trough = build(w, hh, |x, _| a * (x * h) * (x * h));
    let rel = |got: f64, want: f64| ((got - want) / want).abs();
    // (i0, j0) cells east and south of the centre; (10, 0) is on the axis.
    for &(i0, j0) in &[(10i64, 0i64), (10, 7), (-6, 12), (3, -14)] {
        let at = ((hh as i64 / 2 + j0) * w as i64 + w as i64 / 2 + i0) as usize;
        let (x0, y0) = (i0 as f64 * h, j0 as f64 * h);
        let rho = (x0 * x0 + y0 * y0).sqrt();
        let want_profile = 2.0 * a / libm::pow(1.0 + 4.0 * a * a * rho * rho, 1.5);
        let c = d::curvature(&Raster::new(&paraboloid, w, hh, h), 2);
        assert!(rel(c.profile[at], want_profile) < 1e-9, "({i0}, {j0}) paraboloid profile {} want {want_profile}", c.profile[at]);
        assert!(rel(c.contour[at], 1.0 / rho) < 1e-9, "({i0}, {j0}) paraboloid contour {} want {}", c.contour[at], 1.0 / rho);
        // The trough's gradient is along x: profile at x0 is 2a / (1 + 4 a^2 x0^2)^1.5, contour 0.
        let c = d::curvature(&Raster::new(&trough, w, hh, h), 2);
        let want = 2.0 * a / libm::pow(1.0 + 4.0 * a * a * x0 * x0, 1.5);
        assert!(rel(c.profile[at], want) < 1e-9, "({i0}, {j0}) trough profile {} want {want}", c.profile[at]);
        assert!(c.contour[at].abs() < 1e-12, "({i0}, {j0}) trough contour {}", c.contour[at]);
    }
    // On the trough's axis the gradient is under the floor: profile 0, contour uncounted.
    let c = d::curvature(&Raster::new(&trough, w, hh, h), 2);
    let axis = (hh / 2) * w + w / 2;
    assert_eq!(c.profile[axis], 0.0);
    assert!(c.contour[axis].is_nan());
    assert!(c.profile[0].is_nan());
}

// spec: GEN-089
#[test]
fn the_contour_curvature_of_a_near_flat_noisy_plane_is_uncounted_and_the_profile_bounded() {
    let mut s = 12345u64;
    let z: Vec<f64> = (0..60 * 60)
        .map(|_| {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((s >> 40) as f64 / 16777216.0 - 0.5) * 2e-4 // 1e-4 m of noise on 30 m cells: gradients near 1e-5
        })
        .collect();
    let c = d::curvature(&Raster::new(&z, 60, 60, 30.0), 3);
    assert!(c.contour.iter().all(|v| v.is_nan()), "every contour value is uncounted below a gradient of 1e-3");
    assert!((1..59).all(|j| c.profile[j * 60 + 1..j * 60 + 59].iter().all(|v| !v.is_nan() && v.abs() < 1e-3)), "the profile stays bounded");
}

// spec: GEN-090
#[test]
fn relief_of_a_block_holding_a_hundred_metre_step_is_a_hundred_metres() {
    let z = build(64, 64, |x, _| if x < -4.0 { 0.0 } else { 100.0 }); // the step sits at column 28, inside block 3 of 8
    let b = d::local_relief(&Raster::new(&z, 64, 64, 30.0), 8, 4);
    assert_eq!((b.bw, b.bh), (8, 8));
    for bj in 0..8 {
        for bi in 0..8 {
            let want = if bi == 3 { 100.0 } else { 0.0 };
            assert_eq!(b.values[bj * 8 + bi], want, "block ({bi}, {bj})");
        }
    }
    let mut z = z;
    z[10] = f64::NAN;
    let b = d::local_relief(&Raster::new(&z, 64, 64, 30.0), 8, 1);
    assert!(b.values[1].is_nan() && !b.values[0].is_nan());
    // A partial block at the edge is dropped.
    let b = d::local_relief(&Raster::new(&z, 64, 64, 30.0), 10, 1);
    assert_eq!((b.bw, b.bh), (6, 6));
}

// spec: GEN-090
#[test]
fn tpi_follows_crests_and_troughs_and_the_summed_area_equals_the_direct_mean() {
    let (w, h, r) = (1001, 41, 10usize);
    let period = 1000.0; // cells
    for offset in [0.0, 9000.0] {
        let z = build(w, h, |x, _| offset + 50.0 * libm::sin(2.0 * std::f64::consts::PI * x / period));
        let t = d::tpi(&Raster::new(&z, w, h, 10.0), r, 4);
        let (cx, cy) = (w / 2, h / 2);
        let crest = cy * w + cx + 250;
        let trough = cy * w + cx - 250;
        assert!(t[crest] > 0.0 && t[trough] < 0.0, "offset {offset}: crest {} trough {}", t[crest], t[trough]);
        // Edge rule: only cells at least r from the edge are counted.
        assert!(t[r * w + r - 1].is_nan() && !t[r * w + r].is_nan() && t[(h - r) * w + r].is_nan() && !t[(h - 1 - r) * w + r].is_nan());
        for &k in &[crest, trough, cy * w + 17, (r + 3) * w + 500, (h - 1 - r) * w + w - 1 - r] {
            let (i, j) = (k % w, k / w);
            let mut sum = 0.0;
            for jj in j - r..=j + r {
                for ii in i - r..=i + r {
                    sum += z[jj * w + ii];
                }
            }
            let direct = z[k] - sum / ((2 * r + 1) * (2 * r + 1)) as f64;
            assert!((t[k] - direct).abs() < 1e-9, "offset {offset}, cell ({i}, {j}): {} vs {direct}", t[k]);
        }
        // A window holding no-data is not counted; its neighbours outside the window are.
        let mut z = z;
        z[cy * w + 100] = f64::NAN;
        let t = d::tpi(&Raster::new(&z, w, h, 10.0), r, 1);
        assert!(t[cy * w + 100 + r].is_nan() && t[(cy + r) * w + 100 - r].is_nan());
        assert!(!t[cy * w + 100 + r + 1].is_nan() && !t[cy * w + 100 - r - 1].is_nan());
    }
}

fn forms_at_centre(w: usize, f: impl Fn(f64, f64) -> f64, lookup: usize) -> u8 {
    let z = build(w, w, f);
    d::geomorphons(&Raster::new(&z, w, w, 30.0), lookup, 1.0, 2)[(w / 2) * w + w / 2]
}

// spec: GEN-091
#[test]
fn geomorphons_name_the_apex_the_pit_the_valley_the_ridge_and_the_plane() {
    // Shapes whose elevation angles change along a line of sight: an exactly constant angle is a tie, which the rule resolves as higher.
    let w = 101;
    let hill = |x: f64, y: f64| 1000.0 - 0.002 * 900.0 * (x * x + y * y);
    assert_eq!(forms_at_centre(w, hill, 33), form::PEAK);
    assert_eq!(forms_at_centre(w, |x, y| -hill(x, y), 33), form::PIT);
    assert_eq!(forms_at_centre(w, |x, _| 0.002 * 900.0 * x * x, 33), form::VALLEY);
    assert_eq!(forms_at_centre(w, |x, _| -0.002 * 900.0 * x * x, 33), form::RIDGE);
    // A plane with a little curvature so that no line is an exact tie.
    let plane = |deg: f64| move |x: f64, _: f64| 30.0 * libm::tan(deg.to_radians()) * x - 1e-5 * 900.0 * x * x;
    assert_eq!(forms_at_centre(w, plane(10.0), 33), form::SLOPE);
    // Pins the ternary rule: at 1 degree a 0.7 degree plane is level in every direction (neither angle exceeds the threshold; the sum of the zenith and nadir
    // angles, 1.4 degrees, is not what counts).
    assert_eq!(forms_at_centre(w, plane(0.7), 33), form::FLAT);
}

// spec: GEN-091
#[test]
fn geomorphons_uncount_the_edge_band_and_cells_that_look_at_no_data() {
    let w = 101;
    let mut z = build(w, w, |x, _| 15.0 * x.abs());
    let c = d::geomorphons(&Raster::new(&z, w, w, 30.0), 33, 1.0, 1);
    assert_eq!(c[33 * w + 33 - 1], d::UNCOUNTED);
    assert_ne!(c[33 * w + 33], d::UNCOUNTED);
    assert_eq!(c[(w - 33) * w + 40], d::UNCOUNTED);
    z[50 * w + 60] = f64::NAN;
    let c = d::geomorphons(&Raster::new(&z, w, w, 30.0), 33, 1.0, 1);
    assert_eq!(c[50 * w + 50], d::UNCOUNTED, "the line east reaches the no-data cell");
    assert_ne!(c[40 * w + 40], d::UNCOUNTED);
}

// spec: GEN-091
#[test]
fn the_class_table_is_that_of_the_reference_for_all_45_pairs() {
    use form::{
        FLAT as FL, FOOTSLOPE as FS, HOLLOW as HL, PEAK as PK, PIT as PT, RIDGE as RI, SHOULDER as SH, SLOPE as SL, SPUR as SP,
        VALLEY as VL,
    };
    // rows: lower directions 0..=8; columns: higher directions 0..=(8 - lower), as in the reference's `forms[minus][plus]`.
    let table: [&[u8]; 9] = [
        &[FL, FL, FL, FS, FS, VL, VL, VL, PT],
        &[FL, FL, FS, FS, FS, VL, VL, VL],
        &[FL, SH, SL, SL, HL, HL, VL],
        &[SH, SH, SL, SL, SL, HL],
        &[SH, SH, SP, SL, SL],
        &[RI, RI, SP, SP],
        &[RI, RI, RI],
        &[RI, RI],
        &[PK],
    ];
    let mut pairs = 0;
    for (minus, row) in table.iter().enumerate() {
        for (plus, &want) in row.iter().enumerate() {
            assert_eq!(d::geomorphon_class(minus, plus), want, "{minus} lower, {plus} higher");
            pairs += 1;
        }
    }
    assert_eq!(pairs, 45);
}

/// A raster whose centre sees the given pattern: each of the 8 directions (E, NE, N, NW, W, SW, S, SE) is a line that rises (+1), falls (-1) or stays level (0). Rising
/// and falling lines change their angle along the line (no exact ties).
fn pattern_raster(pattern: [i8; 8]) -> (Vec<f64>, usize) {
    let (w, lookup) = (71usize, 33usize);
    let dirs: [(i64, i64); 8] = [(1, 0), (1, -1), (0, -1), (-1, -1), (-1, 0), (-1, 1), (0, 1), (1, 1)];
    let mut z = vec![0.0; w * w];
    for (&(dx, dy), &sign) in dirs.iter().zip(&pattern) {
        for k in 1..=lookup as i64 {
            let step = if dx != 0 && dy != 0 { 30.0 * 2f64.sqrt() } else { 30.0 };
            let tan = 0.1 + 0.002 * k as f64;
            z[((35 + dy * k) * w as i64 + 35 + dx * k) as usize] = sign as f64 * tan * k as f64 * step;
        }
    }
    (z, w)
}

// spec: GEN-091
#[test]
fn every_pattern_of_lower_and_higher_directions_reaches_its_class_through_the_classifier() {
    for minus in 0..=8usize {
        for plus in 0..=8 - minus {
            let mut pattern = [0i8; 8];
            pattern.iter_mut().take(plus).for_each(|v| *v = 1);
            pattern.iter_mut().skip(plus).take(minus).for_each(|v| *v = -1);
            let (z, w) = pattern_raster(pattern);
            let got = d::geomorphons(&Raster::new(&z, w, w, 30.0), 33, 1.0, 1)[35 * w + 35];
            assert_eq!(got, d::geomorphon_class(minus, plus), "{minus} lower, {plus} higher");
        }
    }
}

/// A tilted V valley along `j`: axis in the middle column, one metre of fall per row at 10 m cells, walls one metre per metre.
fn tilted_valley(w: usize, h: usize, pit_row: Option<usize>) -> Vec<f64> {
    let (cx, cell) = (w / 2, 10.0);
    let mut z: Vec<f64> = (0..w * h)
        .map(|k| {
            let (i, j) = (k % w, k / w);
            0.1 * cell * (h - 1 - j) as f64 + (i as f64 - cx as f64).abs() * cell
        })
        .collect();
    if let Some(p) = pit_row {
        z[p * w + cx] -= 1.5; // a metre and a half below the cell downstream
    }
    z
}

// spec: GEN-092
#[test]
fn a_pit_in_a_tilted_valley_is_filled_and_the_area_grows_one_row_per_step() {
    let (w, h, cell) = (21usize, 100usize, 10.0);
    let z = tilted_valley(w, h, Some(30));
    let r = Raster::new(&z, w, h, cell);
    let dr = d::drainage(&r);
    let axis = |j: usize| j * w + w / 2;
    assert!(dr.filled[axis(30)] > z[axis(30)] + 0.4, "the pit is filled to its outlet: {} from {}", dr.filled[axis(30)], z[axis(30)]);
    assert_eq!(dr.receiver[axis(30)] as usize, axis(31));
    for j in 0..h {
        let want = ((j + 1) * w) as f64 * cell * cell;
        assert_eq!(dr.area[axis(j)], want, "area on the axis at row {j}");
    }
    assert_eq!(dr.receiver[axis(h - 1)], d::NO_RECEIVER);
    // Density: channel cells have 1e5 m2 from row 47 on (48 rows x 21 cells x 100 m2 = 100,800); the outlet row has no receiver, so 52 steps of one cell.
    let want = (52.0 * cell / 1000.0) / ((w * h) as f64 * cell * cell / 1e6);
    let got = d::drainage_density(&r, &dr, d::CHANNEL_AREA_M2);
    assert!((got / want - 1.0).abs() < 1e-9, "density {got} want {want}");
}

// spec: GEN-092
#[test]
fn a_diagonal_step_counts_root_two_cells_and_no_data_is_an_outlet() {
    // A plane falling toward the south-east corner drains diagonally.
    let n = 12;
    let z = build(n, n, |x, y| -(x + y));
    let r = Raster::new(&z, n, n, 10.0);
    let dr = d::drainage(&r);
    assert_eq!(dr.receiver[3 * n + 3] as usize, 4 * n + 4);
    // (n - 1)^2 cells step diagonally; the last column and the last row (n - 1 cells each) step straight; the corner has no receiver.
    let length = ((n - 1) * (n - 1)) as f64 * 10.0 * 2f64.sqrt() + (2 * (n - 1)) as f64 * 10.0;
    let want = (length / 1000.0) / ((n * n) as f64 * 100.0 / 1e6);
    let got = d::drainage_density(&r, &dr, 0.0);
    assert!((got / want - 1.0).abs() < 1e-12, "density {got} want {want}");
    // A no-data corner drains its neighbours: the cell next to it is an outlet.
    let mut z = z;
    z[n * n - 1] = f64::NAN;
    let dr = d::drainage(&Raster::new(&z, n, n, 10.0));
    assert!(dr.area[n * n - 1].is_nan());
    assert_ne!(dr.receiver[(n - 2) * n + (n - 2)] as usize, n * n - 1, "no cell drains into no-data");
    assert!(dr.filled[(n - 2) * n + (n - 2)].is_finite());
}

fn rough(w: usize, h: usize, seed: u64, holes: bool) -> Vec<f64> {
    let mut s = seed;
    (0..w * h)
        .map(|k| {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let (i, j) = ((k % w) as f64, (k / w) as f64);
            if holes && (k % w) / 13 == 3 && (k / w) / 11 == 2 {
                return f64::NAN;
            }
            100.0 * libm::sin(i * 0.21) + 60.0 * libm::cos(j * 0.13) + ((s >> 40) as f64) / 16384.0
        })
        .collect()
}

// spec: GEN-089
// spec: GEN-090
// spec: GEN-091
// spec: GEN-092
#[test]
fn every_kernel_gives_the_same_bytes_at_one_and_eight_threads() {
    for &(w, h, holes) in &[(97usize, 83usize, true), (64, 64, false), (9, 5, false)] {
        let z = rough(w, h, 7, holes);
        let r = Raster::new(&z, w, h, 30.0);
        assert_eq!(bits(&d::slope_degrees(&r, 1)), bits(&d::slope_degrees(&r, 8)), "slope {w}x{h}");
        let (a, b) = (d::curvature(&r, 1), d::curvature(&r, 8));
        assert_eq!(bits(&a.profile), bits(&b.profile), "profile {w}x{h}");
        assert_eq!(bits(&a.contour), bits(&b.contour), "contour {w}x{h}");
        let (a, b) = (d::local_relief(&r, 4, 1), d::local_relief(&r, 4, 8));
        assert_eq!(bits(&a.values), bits(&b.values), "relief {w}x{h}");
        assert_eq!(bits(&d::tpi(&r, 3, 1)), bits(&d::tpi(&r, 3, 8)), "tpi {w}x{h}");
        assert_eq!(d::geomorphons(&r, 4, 1.0, 1), d::geomorphons(&r, 4, 1.0, 8), "geomorphons {w}x{h}");
        // The drainage has no thread parameter; it must not depend on the thread it runs on either.
        let a = d::drainage(&r);
        let b = std::thread::scope(|s| s.spawn(|| d::drainage(&r)).join().unwrap());
        assert_eq!(bits(&a.filled), bits(&b.filled), "filled {w}x{h}");
        assert_eq!(a.receiver, b.receiver, "receiver {w}x{h}");
        assert_eq!(bits(&a.area), bits(&b.area), "area {w}x{h}");
        assert_eq!(d::drainage_density(&r, &a, 1e4).to_bits(), d::drainage_density(&r, &b, 1e4).to_bits());
    }
}
