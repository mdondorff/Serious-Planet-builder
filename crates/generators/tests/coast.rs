//! GEN-136: the distance to the coast of the field on a lattice.

use planet_core::Vec3;
use planet_generators::coast::CoastLattice;

const R_KM: f64 = 6371.0;

fn fib(i: u32, n: u32) -> Vec3 {
    let (lon, z) = (f64::from(i) * 2.399_963, 1.0 - 2.0 * (f64::from(i) + 0.5) / f64::from(n));
    let r = (1.0 - z * z).sqrt();
    Vec3::new(r * libm::cos(lon), r * libm::sin(lon), z)
}

/// A field with one circular island: land within `radius_deg` of the centre.
fn island(centre: Vec3, radius_deg: f64) -> impl Fn(Vec3) -> f64 + Sync {
    let r = radius_deg.to_radians();
    move |d: Vec3| r - libm::acos(d.dot(centre).clamp(-1.0, 1.0))
}

fn lattice(centre: Vec3, radius_deg: f64, cells: usize, cap_km: f64, threads: usize) -> CoastLattice {
    let f = island(centre, radius_deg);
    CoastLattice::build(&f, &|d: Vec3| if d.z > 0.0 { 1.0 } else { 0.0 }, cells, cap_km, R_KM, threads)
}

// spec: GEN-136
#[test]
fn the_distance_to_a_circular_coast_is_the_arc_to_its_edge_within_a_fraction_of_a_cell() {
    let centre = Vec3::new(0.3, -0.5, 0.8).normalized();
    let (radius_deg, cells, cap_km) = (12.0, 128, 600.0);
    let l = lattice(centre, radius_deg, cells, cap_km, 3);
    let cell_km = 2.0 * core::f64::consts::PI * R_KM / 4.0 / cells as f64;
    let (mut worst, mut seen) = (0.0f64, 0usize);
    for i in 0..80_000 {
        let d = fib(i, 80_000);
        let angle_km = libm::acos(d.dot(centre).clamp(-1.0, 1.0)) * R_KM;
        let edge_km = radius_deg.to_radians() * R_KM;
        let exact = angle_km - edge_km; // positive in the sea, negative on land
        if exact.abs() < cap_km - 2.0 * cell_km {
            let got = l.read(d).0;
            worst = worst.max((got - exact).abs());
            seen += 1;
        }
    }
    assert!(seen > 1000, "{seen} points inside the cap");
    assert!(worst < 0.25 * cell_km, "the worst error {worst} km against a cell of {cell_km} km");
}

// spec: GEN-136
#[test]
fn the_distance_is_capped_signed_by_land_and_sea_and_the_whole_planet_of_sea_is_the_cap() {
    let centre = Vec3::new(0.0, 0.0, 1.0);
    let l = lattice(centre, 5.0, 64, 300.0, 1);
    assert!(l.read(Vec3::new(0.0, 0.0, -1.0)).0 >= 299.9, "far from the coast: the cap, {}", l.read(Vec3::new(0.0, 0.0, -1.0)).0);
    assert!(l.read(centre).0 < 0.0, "land is negative");
    assert!(l.read(Vec3::new(1.0, 0.0, 0.0)).0 > 0.0, "sea is positive");
    assert_eq!(l.cap_km(), 300.0);
    let none = CoastLattice::build(&|_| -1.0, &|_| 0.0, 16, 250.0, R_KM, 2);
    for i in 0..50 {
        assert_eq!(none.read(fib(i, 50)), (250.0, 0.0), "a world of sea has no coast: the cap and no activity");
    }
}

// spec: GEN-136
#[test]
fn the_activity_is_that_of_the_nearest_crossing_and_the_read_is_continuous_across_face_edges() {
    let centre = Vec3::new(0.7, 0.7, 0.2).normalized();
    let l = lattice(centre, 10.0, 64, 500.0, 2);
    let mut worst = 0.0f64;
    for i in 0..4000 {
        let a = fib(i, 4000);
        let b = Vec3::new(a.x + 1e-6, a.y, a.z).normalized();
        let ((da, aa), (db, ab)) = (l.read(a), l.read(b));
        worst = worst.max((da - db).abs()).max(((aa - ab).abs()) * 100.0);
    }
    assert!(worst < 0.01, "a 6 m step changes the read by {worst}");
    // The crossings of the northern edge of the island carry activity 1 (z over 0), the rest of it 0 or 1: the read stays in range.
    for i in 0..500 {
        let (_, a) = l.read(fib(i, 500));
        assert!((0.0..=1.0).contains(&a), "the activity is in [0, 1]: {a}");
    }
}

// spec: GEN-136
#[test]
fn the_lattice_is_the_same_bits_on_any_thread_count() {
    let centre = Vec3::new(-0.4, 0.2, 0.6).normalized();
    let (one, five) = (lattice(centre, 9.0, 64, 400.0, 1), lattice(centre, 9.0, 64, 400.0, 5));
    for i in 0..3000 {
        let d = fib(i, 3000);
        let (a, b) = (one.read(d), five.read(d));
        assert_eq!((a.0.to_bits(), a.1.to_bits()), (b.0.to_bits(), b.1.to_bits()), "the same bits at 1 and 5 threads");
    }
}

// spec: GEN-136
#[test]
fn the_cells_must_be_a_power_of_two() {
    let r = std::panic::catch_unwind(|| CoastLattice::build(&|_| -1.0, &|_| 0.0, 100, 250.0, R_KM, 1));
    assert!(r.is_err(), "100 cells are refused");
}

// spec: GEN-136
#[test]
fn the_read_is_continuous_along_a_walk_across_face_edges_and_corners() {
    let centre = Vec3::new(0.5, 0.5, 0.7).normalized();
    let l = lattice(centre, 14.0, 128, 700.0, 2);
    // Great circles through the face edges and a cube corner, in steps of 64 m.
    let circles = [
        (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)),
        (Vec3::new(1.0, 1.0, 1.0).normalized(), Vec3::new(-1.0, 1.0, 0.0).normalized()),
    ];
    let step = 1e-5;
    let mut worst = 0.0f64;
    for (a, b) in circles {
        let mut prev = l.read(a).0;
        for k in 1..=(core::f64::consts::TAU / step) as usize {
            let t = step * k as f64;
            let d = (a * libm::cos(t) + b * libm::sin(t)).normalized();
            let now = l.read(d).0;
            worst = worst.max((now - prev).abs());
            prev = now;
        }
    }
    // The distance is 1-Lipschitz (a km per km) plus the bilinear's own slope: a 64 m step moves it by far under 0.5 km.
    assert!(worst < 0.5, "a 64 m step changes the distance by {worst} km");
}
