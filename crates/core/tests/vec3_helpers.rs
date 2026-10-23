//! COORD-003: the direction helpers of `Vec3` and `core::math` give, bit for bit, what the inline copies they replace gave.

use planet_core::hash::SplitMix64;
use planet_core::{smoothstep, Sphere, Vec3, EARTH_RADIUS_M};

fn unit(rng: &mut SplitMix64) -> Vec3 {
    loop {
        let v = Vec3::new(rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0);
        if v.length() > 0.1 {
            return v.normalized();
        }
    }
}

// spec: COORD-003
#[test]
fn angle_to_is_the_atan2_of_the_cross_and_the_dot_product() {
    let mut rng = SplitMix64(7);
    for _ in 0..1000 {
        let (a, b) = (unit(&mut rng), unit(&mut rng));
        assert_eq!(a.angle_to(b).to_bits(), libm::atan2(a.cross(b).length(), a.dot(b)).to_bits());
        assert!((a.angle_to(b) - b.angle_to(a)).abs() < 1e-15);
    }
    let a = Vec3::new(1.0, 0.0, 0.0);
    assert_eq!(a.angle_to(a), 0.0);
    assert!((a.angle_to(Vec3::new(-1.0, 0.0, 0.0)) - std::f64::consts::PI).abs() < 1e-15);
    // A tiny angle keeps its relative precision (acos of the dot product would give 0 or 1.5e-8).
    let tiny = Vec3::new(1.0, 1e-12, 0.0).normalized();
    assert!((a.angle_to(tiny) / 1e-12 - 1.0).abs() < 1e-9);
}

// spec: COORD-003
#[test]
fn from_lat_lon_is_the_inline_formula_and_lat_lon_inverts_it_away_from_the_poles() {
    let mut rng = SplitMix64(11);
    for _ in 0..1000 {
        let (la, lo) = ((rng.next_f64() - 0.5) * 3.0, (rng.next_f64() - 0.5) * 6.2);
        let d = Vec3::from_lat_lon(la, lo);
        let old = Vec3::new(libm::cos(la) * libm::cos(lo), libm::cos(la) * libm::sin(lo), libm::sin(la));
        assert_eq!((d.x.to_bits(), d.y.to_bits(), d.z.to_bits()), (old.x.to_bits(), old.y.to_bits(), old.z.to_bits()));
        let (la2, lo2) = d.lat_lon();
        assert!((la2 - la).abs() < 1e-12 && (lo2 - lo).abs() < 1e-12, "({la}, {lo}) came back as ({la2}, {lo2})");
    }
}

// spec: COORD-003
#[test]
fn lat_lon_is_exact_at_the_poles_and_keeps_its_precision_next_to_them() {
    let half_pi = std::f64::consts::FRAC_PI_2;
    assert_eq!(Vec3::new(0.0, 0.0, 1.0).lat_lon(), (half_pi, 0.0));
    assert_eq!(Vec3::new(0.0, 0.0, -1.0).lat_lon(), (-half_pi, 0.0));
    // A direction 1e-9 rad from the pole: `asin` of z (= 1 - 5e-19, rounded to 1.0) says exactly 90 degrees, the `atan2` form says how far.
    let eps = 1e-9;
    let near = Vec3::new(libm::sin(eps), 0.0, libm::cos(eps));
    let (lat, lon) = near.lat_lon();
    assert!(((half_pi - lat) / eps - 1.0).abs() < 1e-6, "{}", half_pi - lat);
    assert_eq!(lon, 0.0);
    assert_eq!(libm::asin(near.z.clamp(-1.0, 1.0)), half_pi, "the asin form cannot tell");
    // The longitude of a point just past the antimeridian is near -pi, of one on it +pi.
    assert!((Vec3::new(-1.0, -1e-12, 0.0).lat_lon().1 + std::f64::consts::PI).abs() < 1e-9);
    assert_eq!(Vec3::new(-1.0, 0.0, 0.0).lat_lon().1, std::f64::consts::PI);
}

// spec: COORD-003
#[test]
fn tangent_basis_is_the_orthonormal_right_handed_pair_the_copies_built() {
    let mut rng = SplitMix64(13);
    let mut cases: Vec<Vec3> = (0..500).map(|_| unit(&mut rng)).collect();
    cases.extend([Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 0.0, -1.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)]);
    // Near the 0.9 switch of the helper axis, from both sides.
    cases.extend([0.8999, 0.9, 0.9001, -0.8999, -0.9001].map(|z: f64| Vec3::new((1.0 - z * z).sqrt(), 0.0, z)));
    for a in cases {
        let (e1, e2) = a.tangent_basis();
        let helper = if a.z.abs() < 0.9 { Vec3::new(0.0, 0.0, 1.0) } else { Vec3::new(1.0, 0.0, 0.0) };
        let old1 = helper.cross(a).normalized();
        let old2 = a.cross(old1);
        assert_eq!((e1, e2), (old1, old2));
        assert!(e1.dot(a).abs() < 1e-12 && e2.dot(a).abs() < 1e-12 && e1.dot(e2).abs() < 1e-12, "{a:?}");
        assert!((e1.length() - 1.0).abs() < 1e-12 && (e2.length() - 1.0).abs() < 1e-12);
        assert!(e1.cross(e2).dot(a) > 0.999_999, "{a:?}: right-handed with the normal");
    }
}

// spec: COORD-003
#[test]
fn smoothstep_and_the_earth_radius_are_the_values_the_copies_had() {
    let old = |e0: f64, e1: f64, x: f64| {
        let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    let mut rng = SplitMix64(17);
    for _ in 0..1000 {
        let (e0, w, x) = (rng.next_f64() * 100.0 - 50.0, rng.next_f64() * 40.0 + 0.1, rng.next_f64() * 200.0 - 100.0);
        assert_eq!(smoothstep(e0, e0 + w, x).to_bits(), old(e0, e0 + w, x).to_bits());
    }
    assert_eq!((smoothstep(0.0, 1.0, -1.0), smoothstep(0.0, 1.0, 0.5), smoothstep(0.0, 1.0, 2.0)), (0.0, 0.5, 1.0));
    assert_eq!(EARTH_RADIUS_M, 6_371_000.0);
    assert_eq!(Sphere::EARTH.radius_m, EARTH_RADIUS_M);
}
