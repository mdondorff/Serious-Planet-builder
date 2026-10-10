//! COORD-005, COORD-006, COORD-007: face mapping, tile ids, cross-face neighbours.

use planet_core::cube::{direction_to_face, face_to_direction, FaceMapping, TangentWarp};
use planet_core::hash::SplitMix64;
use planet_core::{Face, Side, TileId, Vec3};

fn random_dir(rng: &mut SplitMix64) -> Vec3 {
    loop {
        let v = Vec3::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0));
        if v.length() > 0.1 {
            return v.normalized();
        }
    }
}

fn angle(a: Vec3, b: Vec3) -> f64 {
    libm::atan2(a.cross(b).length(), a.dot(b))
}

// spec: COORD-005
#[test]
fn face_axes_are_right_handed_and_orthonormal() {
    for f in Face::ALL {
        assert_eq!(f.u_axis().cross(f.v_axis()), f.normal(), "face {f:?}: U x V must equal the normal");
        assert_eq!(f.u_axis().dot(f.normal()), 0.0);
        assert_eq!(f.v_axis().dot(f.normal()), 0.0);
    }
}

// spec: COORD-005
#[test]
fn directions_round_trip_through_face_coordinates() {
    let map = TangentWarp;
    let mut rng = SplitMix64(1);
    let mut dirs: Vec<Vec3> = (0..50_000).map(|_| random_dir(&mut rng)).collect();
    // Exact axes, edges and corners are the awkward cases.
    for &(x, y, z) in &[(1.0, 0.0, 0.0), (0.0, -1.0, 0.0), (1.0, 1.0, 0.0), (1.0, 1.0, 1.0), (-1.0, 1.0, -1.0), (0.0, 1.0, 1.0)] {
        dirs.push(Vec3::new(x, y, z).normalized());
    }
    let mut worst = 0.0f64;
    for d in dirs {
        let (f, s, t) = direction_to_face(&map, d);
        assert!((-1.0..=1.0).contains(&s) && (-1.0..=1.0).contains(&t), "{d:?} gave s={s} t={t}");
        let back = face_to_direction(&map, f, s, t);
        worst = worst.max(angle(d, back));
    }
    assert!(worst <= 1e-12, "worst direction round-trip error {worst} rad");
}

// spec: COORD-005
#[test]
fn warp_is_odd_monotone_and_exact_at_the_edges() {
    let m = TangentWarp;
    assert_eq!(m.face_to_plane(1.0), 1.0);
    assert_eq!(m.face_to_plane(-1.0), -1.0);
    assert_eq!(m.face_to_plane(0.0), 0.0);
    let mut prev = -2.0;
    for i in 0..=1000 {
        let s = -1.0 + 2.0 * f64::from(i) / 1000.0;
        let u = m.face_to_plane(s);
        assert!(u > prev, "monotone at s={s}");
        assert_eq!(m.face_to_plane(-s), -u, "odd at s={s}");
        prev = u;
    }
}

// spec: COORD-006
#[test]
fn tile_ids_round_trip_and_reject_invalid_values() {
    let mut rng = SplitMix64(2);
    for _ in 0..20_000 {
        let level = (rng.next_u64() % 29) as u8;
        let face = Face((rng.next_u64() % 6) as u8);
        let n = 1u64 << level;
        let (x, y) = ((rng.next_u64() % n) as u32, (rng.next_u64() % n) as u32);
        let id = TileId::new(face, level, x, y).unwrap();
        assert_eq!((id.face(), id.level(), id.x(), id.y()), (face, level, x, y));
        assert_eq!(TileId::from_raw(id.raw()), Some(id));
        if let Some(children) = id.children() {
            for c in children {
                assert_eq!(c.parent(), Some(id));
            }
        } else {
            assert_eq!(level, 28);
        }
        assert_eq!(id.parent().is_none(), level == 0);
    }
    assert!(TileId::new(Face(6), 0, 0, 0).is_none());
    assert!(TileId::new(Face(0), 29, 0, 0).is_none());
    assert!(TileId::new(Face(0), 2, 4, 0).is_none(), "x out of range for level 2");
    assert!(TileId::from_raw(6 << 61).is_none(), "face 6");
    assert!(TileId::from_raw(29 << 56).is_none(), "level 29");
    assert!(TileId::from_raw(1).is_none(), "path bits above level 0");
    assert!(TileId::from_raw((1 << 56) | 0b100).is_none(), "path bits above level 1");
    // Documented layout: face 5, level 1, (x=1, y=0) -> path bits 0b10.
    assert_eq!(TileId::new(Face(5), 1, 1, 0).unwrap().raw(), (5u64 << 61) | (1 << 56) | 0b10);
}

// spec: COORD-006
#[test]
fn every_direction_belongs_to_exactly_one_tile_per_level() {
    let map = TangentWarp;
    let mut rng = SplitMix64(3);
    for level in 0..=3u8 {
        let n = 1u32 << level;
        let tiles: Vec<TileId> =
            Face::ALL.iter().flat_map(|&f| (0..n).flat_map(move |x| (0..n).map(move |y| TileId::new(f, level, x, y).unwrap()))).collect();
        assert_eq!(tiles.len(), 6 * (n * n) as usize);
        for _ in 0..2000 {
            let d = random_dir(&mut rng);
            let holders = tiles.iter().filter(|t| t.contains(&map, d)).count();
            assert_eq!(holders, 1, "direction {d:?} at level {level}");
        }
    }
}

// spec: COORD-007
#[test]
fn neighbours_are_symmetric_across_all_face_edges_and_corners() {
    let map = TangentWarp;
    for level in 0..=4u8 {
        let n = 1u32 << level;
        for f in Face::ALL {
            for x in 0..n {
                for y in 0..n {
                    let t = TileId::new(f, level, x, y).unwrap();
                    for side in Side::ALL {
                        let nb = t.neighbor(&map, side);
                        assert_ne!(nb, t, "{t:?} {side:?}");
                        assert_eq!(nb.level(), level);
                        let back: Vec<Side> = Side::ALL.into_iter().filter(|&s| nb.neighbor(&map, s) == t).collect();
                        assert_eq!(back.len(), 1, "{t:?} -{side:?}-> {nb:?}: expected exactly one way back, got {back:?}");
                    }
                }
            }
        }
    }
}

// spec: COORD-007
#[test]
fn shared_border_samples_are_bit_identical_from_both_sides() {
    let map = TangentWarp;
    let res = 8u32;
    let mut checked = 0;
    for level in 0..=3u8 {
        let n = 1u32 << level;
        for f in Face::ALL {
            for x in 0..n {
                for y in 0..n {
                    let t = TileId::new(f, level, x, y).unwrap();
                    for side in Side::ALL {
                        let nb = t.neighbor(&map, side);
                        // Every border sample of t along `side` must equal, bit for bit, some border sample of the neighbour.
                        for k in 0..=res {
                            let mine = match side {
                                Side::East => t.sample_direction(&map, res, res, k),
                                Side::West => t.sample_direction(&map, res, 0, k),
                                Side::North => t.sample_direction(&map, res, k, res),
                                Side::South => t.sample_direction(&map, res, k, 0),
                            };
                            let mut theirs = Vec::new();
                            for j in 0..=res {
                                theirs.extend([
                                    nb.sample_direction(&map, res, 0, j),
                                    nb.sample_direction(&map, res, res, j),
                                    nb.sample_direction(&map, res, j, 0),
                                    nb.sample_direction(&map, res, j, res),
                                ]);
                            }
                            assert!(theirs.contains(&mine), "{t:?} {side:?} sample {k}: {mine:?} has no bit-identical partner in {nb:?}");
                            checked += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(checked > 10_000);
}

// spec: COORD-007
#[test]
fn the_eight_corners_are_shared_by_exactly_three_faces_with_one_direction() {
    let map = TangentWarp;
    for sx in [-1.0, 1.0] {
        for sy in [-1.0, 1.0] {
            for sz in [-1.0, 1.0] {
                let corner = Vec3::new(sx, sy, sz).normalized();
                let mut found = Vec::new();
                for f in Face::ALL {
                    for s in [-1.0, 1.0] {
                        for t in [-1.0, 1.0] {
                            if face_to_direction(&map, f, s, t) == corner {
                                found.push(f);
                            }
                        }
                    }
                }
                assert_eq!(found.len(), 3, "corner {corner:?} found on faces {found:?}");
            }
        }
    }
}

fn triangle_area(a: Vec3, b: Vec3, c: Vec3) -> f64 {
    // Solid angle of a spherical triangle (Van Oosterom and Strackee).
    2.0 * libm::atan2(a.dot(b.cross(c)).abs(), 1.0 + a.dot(b) + b.dot(c) + c.dot(a))
}

// spec: COORD-005
#[test]
fn tangent_warp_tile_areas_stay_within_the_published_distortion() {
    // Measured for ADR 0003 and the M2 gate: the ratio between the largest and smallest tile of one level.
    let m = TangentWarp;
    for level in [2u8, 4, 6] {
        let n = 1u32 << level;
        let (mut lo, mut hi, mut sum) = (f64::MAX, 0.0f64, 0.0);
        for x in 0..n {
            for y in 0..n {
                let t = TileId::new(Face(0), level, x, y).unwrap();
                let (s0, s1, t0, t1) = t.bounds();
                let p = |s, t| face_to_direction(&m, Face(0), s, t);
                let (a, b, c, d) = (p(s0, t0), p(s1, t0), p(s1, t1), p(s0, t1));
                let area = triangle_area(a, b, c) + triangle_area(a, c, d);
                lo = lo.min(area);
                hi = hi.max(area);
                sum += area;
            }
        }
        // One face covers 4 pi / 6 steradians.
        assert!((sum - 4.0 * core::f64::consts::PI / 6.0).abs() < 1e-9, "level {level}: tiles must tile the face, area {sum}");
        let ratio = hi / lo;
        eprintln!("level {level}: tile area ratio max/min = {ratio:.4}");
        assert!(
            ratio > 1.15 && ratio < 1.5,
            "tile area ratio {ratio} at level {level} (published: about 1.41 for the adjusted gnomonic warp)"
        );
    }
}
