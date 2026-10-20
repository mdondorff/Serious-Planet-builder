//! TEST-015: mesh invariants on the steep stress field (generators::stress::Steep), which the smooth reference generator
//! never exercises: about 8 km of relief and fine-scale slopes.

use planet_core::cube::TangentWarp;
use planet_core::hash::SplitMix64;
use planet_core::{Face, Side, TileId, Vec3};
use planet_generators::height::{cache_key, cache_key_field, HeightField, Reference};
use planet_generators::mesh::{mesh_key, mesh_key_field, tile_mesh, tile_mesh_field, TileMesh};
use planet_generators::stress::{Steep, STEEP_AMPLITUDE_M, STEEP_VERSION};

const R: f64 = 6_371_000.0;
const CELLS: u32 = 16;
const STEEP: Steep = Steep(1);

fn id(f: u8, l: u8, x: u32, y: u32) -> TileId {
    TileId::new(Face(f), l, x, y).unwrap()
}

fn world(m: &TileMesh, i: u32, j: u32) -> Vec3 {
    let v = m.vertices[(j * (m.cells + 1) + i) as usize];
    m.origin.0 + Vec3::new(f64::from(v.pos[0]), f64::from(v.pos[1]), f64::from(v.pos[2]))
}

fn coarse_world(m: &TileMesh, i: u32, j: u32) -> Vec3 {
    let v = m.vertices[(j * (m.cells + 1) + i) as usize];
    m.origin.0 + Vec3::new(f64::from(v.coarse[0]), f64::from(v.coarse[1]), f64::from(v.coarse[2]))
}

fn nrm(m: &TileMesh, i: u32, j: u32) -> Vec3 {
    let v = m.vertices[(j * (m.cells + 1) + i) as usize];
    Vec3::new(f64::from(v.normal[0]), f64::from(v.normal[1]), f64::from(v.normal[2]))
}

fn border(side: Side, n: u32) -> Vec<(u32, u32)> {
    (0..=n)
        .map(|k| match side {
            Side::East => (n, k),
            Side::West => (0, k),
            Side::North => (k, n),
            Side::South => (k, 0),
        })
        .collect()
}

// spec: TEST-015
#[test]
fn the_stress_field_is_high_and_steep() {
    let mut rng = SplitMix64(7);
    let (mut hi, mut lo, mut slope) = (f64::MIN, f64::MAX, 0.0f64);
    for _ in 0..4000 {
        let d = Vec3::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)).normalized();
        let h = STEEP.height(d);
        hi = hi.max(h);
        lo = lo.min(h);
        // Slope over 20 m along an arbitrary tangent.
        let t = d.cross(Vec3::new(0.0, 0.0, 1.0)).normalized();
        let d2 = (d + t * (20.0 / R)).normalized();
        slope = slope.max((STEEP.height(d2) - h).abs() / 20.0);
        assert!(h.abs() <= STEEP_AMPLITUDE_M, "height {h} m outside the documented bound");
    }
    eprintln!("steep field: heights {lo:.0}..{hi:.0} m, steepest 20 m slope {:.0} degrees", libm::atan(slope).to_degrees());
    assert!(hi - lo > 6000.0, "the stress field must have several km of relief, got {:.0} m", hi - lo);
    let min_slope = libm::tan(27f64.to_radians());
    assert!(slope > min_slope, "the stress field must have slopes above 27 degrees, got {:.1}", libm::atan(slope).to_degrees());
}

// spec: TEST-015
#[test]
fn reference_field_meshes_and_keys_are_unchanged_and_steep_keys_never_alias() {
    let t = id(3, 9, 100, 200);
    assert_eq!(tile_mesh_field(&Reference(1), &TangentWarp, t, CELLS, R), tile_mesh(1, &TangentWarp, t, CELLS, R));
    assert_eq!(mesh_key_field(&Reference(1), &TangentWarp, t, CELLS, R), mesh_key(&TangentWarp, 1, t, CELLS, R));
    assert_eq!(cache_key_field(&Reference(1), &TangentWarp, t, CELLS), cache_key(&TangentWarp, 1, t, CELLS));
    let steep = tile_mesh_field(&STEEP, &TangentWarp, t, CELLS, R);
    assert_ne!(steep.key, mesh_key(&TangentWarp, 1, t, CELLS, R), "a stress mesh must never be served as a reference mesh");
    assert_ne!(cache_key_field(&STEEP, &TangentWarp, t, CELLS), cache_key(&TangentWarp, 1, t, CELLS));
    assert_ne!(steep.vertices, tile_mesh(1, &TangentWarp, t, CELLS, R).vertices);
}

/// Committed known answer of one stress mesh, tied to the stress field's version (ADR 0006 version guard).
const KNOWN_STEEP_MESH: u64 = 10611333974132247449;

// spec: TEST-015
#[test]
fn the_stress_field_has_a_version_guarded_known_answer() {
    assert_eq!(
        (STEEP_VERSION, planet_generators::mesh::MESH_VERSION),
        (1, 9),
        "bump KNOWN_STEEP_MESH together with STEEP_VERSION or MESH_VERSION"
    );
    let h = tile_mesh_field(&STEEP, &TangentWarp, id(4, 12, 1000, 2000), CELLS, R).content_hash();
    assert_eq!(
        h, KNOWN_STEEP_MESH,
        "stress mesh output changed (got {h}): bump STEEP_VERSION (or MESH_VERSION) and update the known answer"
    );
}

// spec: TEST-015
#[test]
fn steep_deep_tiles_stay_tile_local_within_a_millimetre() {
    for t in [id(2, 26, 123_456, 654_321), id(0, 20, 400_000, 700_000), id(5, 16, 9_000, 30_000)] {
        let m = tile_mesh_field(&STEEP, &TangentWarp, t, CELLS, R);
        let mut worst = 0.0f64;
        for j in 0..=CELLS {
            for i in 0..=CELLS {
                let dir = t.sample_direction(&TangentWarp, CELLS, i, j);
                worst = worst.max((dir * (R + STEEP.height(dir))).distance(world(&m, i, j)));
            }
        }
        assert!(worst < 1e-3, "{t:?}: worst vertex error {worst} m on steep terrain (CON-13: 1 mm)");
    }
}

// spec: TEST-015
#[test]
fn steep_borders_match_positions_and_normals_across_every_cube_edge() {
    let m = TangentWarp;
    let (mut pairs, mut worst_pos, mut worst_angle) = (0, 0.0f64, 0.0f64);
    for f in 0..6u8 {
        for x in 0..4u32 {
            for y in 0..4u32 {
                let t = id(f, 2, x, y);
                let a = tile_mesh_field(&STEEP, &m, t, CELLS, R);
                for side in Side::ALL {
                    let b = tile_mesh_field(&STEEP, &m, t.neighbor(&m, side), CELLS, R);
                    for (i, j) in border(side, CELLS) {
                        let p = world(&a, i, j);
                        let (d, n) = border(Side::East, CELLS)
                            .into_iter()
                            .chain(border(Side::West, CELLS))
                            .chain(border(Side::North, CELLS))
                            .chain(border(Side::South, CELLS))
                            .map(|(bi, bj)| (world(&b, bi, bj).distance(p), nrm(&b, bi, bj)))
                            .fold((f64::MAX, Vec3::ZERO), |acc, x| if x.0 < acc.0 { x } else { acc });
                        worst_pos = worst_pos.max(d);
                        let x = nrm(&a, i, j);
                        worst_angle = worst_angle.max(libm::atan2(x.cross(n).length(), x.dot(n)).to_degrees());
                    }
                    pairs += 1;
                }
            }
        }
    }
    assert_eq!(pairs, 6 * 16 * 4);
    let edge = R * core::f64::consts::FRAC_PI_2 / 4.0;
    assert!(worst_pos <= edge * 1e-6 + 1e-3, "steep border vertices differ by up to {worst_pos} m between neighbours");
    assert!(worst_angle < 1e-6, "steep border normals differ by up to {worst_angle} degrees between neighbours");
}

// spec: TEST-015
#[test]
fn a_fully_morphed_steep_border_lies_on_the_coarser_neighbours_border() {
    let m = TangentWarp;
    let mut checked = 0;
    for (f, l, x, y) in
        [(0u8, 1u8, 0u32, 0u32), (0, 2, 3, 0), (2, 3, 7, 7), (4, 1, 1, 1), (3, 2, 0, 3), (1, 12, 4095, 2048), (5, 18, 0, 77)]
    {
        let t = id(f, l, x, y);
        let tm = tile_mesh_field(&STEEP, &m, t, CELLS, R);
        for side in Side::ALL {
            let nb = t.neighbor(&m, side);
            let Some(p) = nb.parent() else { continue };
            if Some(p) == t.parent() {
                continue;
            }
            let pm = tile_mesh_field(&STEEP, &m, p, CELLS, R);
            // Segments between adjacent lattice points of the parent's four borders: the border polyline itself.
            let segments: Vec<(Vec3, Vec3)> = Side::ALL
                .into_iter()
                .flat_map(|s| {
                    let pts: Vec<Vec3> = border(s, CELLS).into_iter().map(|(i, j)| world(&pm, i, j)).collect();
                    pts.windows(2).map(|w| (w[0], w[1])).collect::<Vec<_>>()
                })
                .collect();
            let spacing = R * core::f64::consts::FRAC_PI_2 / f64::from(1u32 << p.level()) / f64::from(CELLS);
            let on_polyline = |q: Vec3| {
                segments
                    .iter()
                    .map(|(a, b)| {
                        let ab = *b - *a;
                        let s = ((q - *a).dot(ab) / ab.dot(ab)).clamp(0.0, 1.0);
                        (q - (*a + ab * s)).length()
                    })
                    .fold(f64::MAX, f64::min)
            };
            let worst = border(side, CELLS).into_iter().map(|(i, j)| on_polyline(coarse_world(&tm, i, j))).fold(0.0f64, f64::max);
            let tol = spacing * 1e-6 + 2e-3;
            assert!(
                worst < tol,
                "{t:?} {side:?}: morphed steep border vertex is {worst} m off the coarser neighbour's border (tolerance {tol} m)"
            );
            checked += 1;
        }
    }
    assert!(checked >= 6, "only {checked} borders checked");
}
