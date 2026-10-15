//! The compact tile builder (GEN-019, ADR 0033 point 4): layout, bytes, 16-bit octahedral normals, the 16-bit height step,
//! the CON-13 relief guard and determinism. GPU-free.

use planet_core::cube::TangentWarp;
use planet_core::hash::SplitMix64;
use planet_core::{Face, TileId, Vec3};
use planet_generators::compact::{
    compact_tile_field, oct_decode, oct_encode, slot_bytes, slot_texel_bytes, tile_data_field, vertex_reason, CompactTile, RenderTile,
    VertexReason, COMPACT_MIN_LEVEL, COMPACT_RECORD_BYTES, FALLBACK_RELIEF_M, TEXEL_BYTES,
};
use planet_generators::global::{bake, GridSource};
use planet_generators::height::{HeightField, Reference};
use planet_generators::height_fields::{build_field, HeightFieldKind};
use planet_generators::mesh::{mesh_key_field, tile_mesh_field, vertex_count};
use planet_generators::stress::Steep;
use planet_world_def::{BodyId, GenContext, WorldDefinition};

const R: f64 = 6_371_000.0;

fn id(f: u8, l: u8, x: u32, y: u32) -> TileId {
    TileId::new(Face(f), l, x, y).unwrap()
}

/// Angle between a decoded normal and the exact one, from the cross product (acos of a dot product near 1 loses half the digits).
fn angle_deg(a: [f32; 3], b: Vec3) -> f64 {
    let a = Vec3::new(f64::from(a[0]), f64::from(a[1]), f64::from(a[2]));
    libm::atan2(a.cross(b).length(), a.dot(b)).to_degrees()
}

// spec: GEN-019
#[test]
fn a_tile_is_8_byte_texels_and_a_128_byte_record() {
    assert_eq!(TEXEL_BYTES, 8);
    assert_eq!(COMPACT_RECORD_BYTES, 128);
    // ADR 0033: 8,712 B of texels and 8,840 B per tile at cells 32; 33.9 KB at cells 64.
    assert_eq!((slot_texel_bytes(32), slot_bytes(32)), (8_712, 8_840));
    assert_eq!(slot_bytes(64), 65 * 65 * 8 + 128);
    let t = compact_tile_field(&Reference(1), &TangentWarp, id(2, 12, 100, 200), 32, R);
    assert_eq!(t.texels.len(), 33 * 33);
    assert_eq!(t.texel_data().len() as u64, slot_texel_bytes(32));
    assert_eq!(t.resident_bytes(), slot_bytes(32));
    assert_eq!(t.record_bytes().len(), 128);
    assert!(t.texels.iter().all(|x| x[3] == 0), "the spare 16 bits are reserved and zero");
    // The record carries the control points and parameters verbatim.
    let r = t.record_bytes();
    let word = |k: usize| f32::from_le_bytes(r[k * 4..k * 4 + 4].try_into().unwrap());
    assert_eq!([word(0), word(1), word(2)], t.ctrl[0]);
    assert_eq!([word(24), word(25), word(26)], t.ctrl[8]);
    assert_eq!([word(27), word(28), word(29), word(30)], [t.hr_min, t.hr_step, t.h_origin, t.skirt_depth]);
    assert_eq!(u32::from_le_bytes(r[124..128].try_into().unwrap()), u32::from(t.flip));
}

// spec: GEN-019
#[test]
fn a_tile_has_one_key_in_either_format() {
    let t = id(1, 9, 33, 400);
    let c = compact_tile_field(&Reference(3), &TangentWarp, t, 16, R);
    let m = tile_mesh_field(&Reference(3), &TangentWarp, t, 16, R);
    assert_eq!(c.key, m.key);
    assert_eq!(c.key, mesh_key_field(&Reference(3), &TangentWarp, t, 16, R));
    assert_eq!(c.origin, m.origin, "the origin rule is shared, so camera-relative origins in the plan are valid for both");
}

// spec: GEN-019, CON-14
#[test]
fn sixteen_bit_octahedral_normals_stay_far_inside_the_normal_bound() {
    // The ADR's figure was a calculation (about 0.002 degrees); this is the measurement over random directions, the six axes, the
    // octahedron's folds and edges, and a dense sweep around the equator of the fold (z = 0) where the encoding is coarsest.
    let mut rng = SplitMix64(11);
    let mut dirs: Vec<Vec3> =
        (0..200_000).map(|_| Vec3::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)).normalized()).collect();
    for axis in [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)] {
        dirs.push(axis);
        dirs.push(axis * -1.0);
    }
    for k in 0..4000 {
        let a = f64::from(k) / 4000.0 * core::f64::consts::TAU;
        dirs.push(Vec3::new(libm::cos(a), libm::sin(a), 0.0));
        dirs.push(Vec3::new(libm::cos(a), libm::sin(a), 1.0e-4).normalized());
        dirs.push(Vec3::new(libm::cos(a), libm::sin(a), -1.0e-4).normalized());
    }
    let mut worst = 0.0f64;
    for d in &dirs {
        let dec = oct_decode(oct_encode(*d));
        worst = worst.max(angle_deg(dec, *d));
    }
    eprintln!("worst oct16 normal error over {} directions: {worst:.5} degrees", dirs.len());
    assert!(worst <= 0.01, "worst 16-bit octahedral error {worst} degrees exceeds CON-14's 0.01");
}

// spec: GEN-019, CON-14
#[test]
fn a_tiles_stored_normals_equal_the_cpu_normal_within_the_bound() {
    for (f, l, x, y) in [(0u8, 3u8, 2u32, 5u32), (2, 8, 40, 77), (4, 14, 9, 9000), (5, 22, 1_000_000, 17)] {
        let t = id(f, l, x, y);
        let c = compact_tile_field(&Reference(1), &TangentWarp, t, 32, R);
        let m = tile_mesh_field(&Reference(1), &TangentWarp, t, 32, R);
        let mut worst = 0.0f64;
        for (k, v) in m.vertices.iter().take(33 * 33).enumerate() {
            let n = Vec3::new(f64::from(v.normal[0]), f64::from(v.normal[1]), f64::from(v.normal[2]));
            let dec = oct_decode([c.texels[k][1], c.texels[k][2]]);
            worst = worst.max(angle_deg(dec, n));
        }
        assert!(worst <= 0.01, "tile {t:?}: stored normal off the CPU normal by {worst} degrees");
    }
}

fn eroded() -> Box<dyn HeightField> {
    // Small in-memory bake: the eroded field adds its fine detail procedurally, so it has relief at every level.
    let grid = bake(7, &TangentWarp, 16, 2).unwrap();
    let def = WorldDefinition::single_planet(R, 7);
    let src = Box::leak(Box::new(GridSource(Box::leak(Box::new(grid)))));
    build_field(HeightFieldKind::Eroded, &GenContext::new(&def), BodyId::PLANET, src, [1; 16]).unwrap()
}

// spec: GEN-019, CON-13, CON-14
#[test]
fn the_height_step_is_under_a_millimetre_at_the_finest_levels_and_bounds_the_height_error() {
    let field = eroded();
    let mut rng = SplitMix64(5);
    let mut checked = 0;
    for level in COMPACT_MIN_LEVEL..=26u8 {
        for _ in 0..1 {
            let n = 1u32 << level;
            let t = id((rng.next_u64() % 6) as u8, level, (rng.next_u64() % u64::from(n)) as u32, (rng.next_u64() % u64::from(n)) as u32);
            let c = compact_tile_field(field.as_ref(), &TangentWarp, t, 32, R);
            let m = tile_mesh_field(field.as_ref(), &TangentWarp, t, 32, R);
            // Every texel is within half a step of the exact height (plus the f32 rounding of the sum, as the product has).
            let mut worst = 0.0f64;
            for j in 0..=32usize {
                for i in 0..=32usize {
                    let exact = f64::from(m.vertices[j * 33 + i].height);
                    let got = f64::from(c.h_origin) + f64::from(c.rel_height(i, j));
                    worst = worst.max((exact - got).abs());
                }
            }
            let slack = 0.5 * f64::from(c.hr_step) + 4.0e-7 * f64::from(m.vertices[0].height.abs().max(1.0)) + 1.0e-7 * c.relief_m;
            assert!(worst <= slack, "tile {t:?}: height error {worst} m beyond half a step {slack}");
            // CON-14's mixed bound max(1e-6 |v|, 1 mm) holds wherever the step is at most 2 mm; and every texel level is inside 0..=65535.
            if f64::from(c.hr_step) <= 2.0e-3 {
                assert!(worst <= 1.0e-6 * f64::from(m.vertices[0].height.abs()).max(1.0e3) + 1.0e-3, "tile {t:?}: {worst}");
            }
            // The range is covered without clamping: the lowest texel is level 0 and the highest the last or next to last.
            let (qmin, qmax) = c.texels.iter().fold((u16::MAX, 0u16), |(lo, hi), t| (lo.min(t[0]), hi.max(t[0])));
            if c.relief_m > 0.0 {
                assert!(qmin == 0 && qmax >= 65_534, "tile {t:?}: quantisation levels {qmin}..{qmax}");
            }
            if level >= 16 {
                assert!(c.step_within_con13(), "level {level}: step {} m exceeds 1 mm", c.hr_step);
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 24);
}

// spec: GEN-019, CON-13
#[test]
fn the_format_follows_the_level_and_the_relief_of_a_finest_tile() {
    // Levels 0-2 keep the vertex format; the compact format starts at level 3.
    assert_eq!(COMPACT_MIN_LEVEL, 3);
    for l in 0..3u8 {
        assert_eq!(vertex_reason(l, 22, 0.0), Some(VertexReason::CoarseLevel));
    }
    assert_eq!(vertex_reason(3, 22, 1.0e4), None, "a coarse tile with large relief stays compact: CON-13 binds the finest level only");
    // At the finest level the guard is the 1 mm step: relief / 65,535 > 1 mm.
    assert!((FALLBACK_RELIEF_M - 65.535).abs() < 1e-9);
    assert_eq!(vertex_reason(22, 22, 65.0), None);
    assert_eq!(vertex_reason(22, 22, 66.0), Some(VertexReason::Relief));
    assert_eq!(vertex_reason(21, 22, 66.0), None);
}

// spec: GEN-019, CON-13
#[test]
fn a_steep_finest_tile_falls_back_to_the_vertex_format_and_a_gentle_one_does_not() {
    // The steep stress field has kilometres of relief per coarse tile: declaring such a tile the finest makes it exceed 65 m.
    let steep = Steep(1);
    let t = id(4, 9, 100, 100);
    let probe = compact_tile_field(&steep, &TangentWarp, t, 32, R);
    assert!(probe.relief_m > FALLBACK_RELIEF_M, "fixture: the steep tile has {} m of relief", probe.relief_m);
    match tile_data_field(&steep, &TangentWarp, t, 32, R, 9) {
        RenderTile::Vertex(m) => assert_eq!(m, tile_mesh_field(&steep, &TangentWarp, t, 32, R), "the fallback is the product mesh"),
        RenderTile::Compact(_) => panic!("a finest tile with {} m of relief must fall back", probe.relief_m),
    }
    // The same tile one level above the finest is compact (CON-13 binds the finest level).
    assert!(tile_data_field(&steep, &TangentWarp, t, 32, R, 10).is_compact());
    // The format depends on the view's finest level but the key does not: a consumer re-checks a reused tile.
    let RenderTile::Compact(c10) = tile_data_field(&steep, &TangentWarp, t, 32, R, 10) else { unreachable!() };
    assert!(c10.allowed_at_finest_level(10) && !c10.allowed_at_finest_level(9));
    // A fine tile of the reference field has centimetres of relief: compact, with the same key as its mesh.
    let g = id(4, 22, 1000, 2000);
    let d = tile_data_field(&Reference(1), &TangentWarp, g, 32, R, 22);
    assert!(d.is_compact());
    assert_eq!(d.key(), mesh_key_field(&Reference(1), &TangentWarp, g, 32, R));
    // Levels 0-2 are vertex format whatever the field.
    for l in 0..3u8 {
        assert!(!tile_data_field(&Reference(1), &TangentWarp, id(1, l, 0, 0), 32, R, 22).is_compact());
    }
    assert_eq!(
        tile_data_field(&Reference(1), &TangentWarp, id(1, 2, 3, 3), 8, R, 22).cells(),
        8,
        "the format decision does not change the grid size"
    );
}

fn tile_hashes(threads: usize) -> Vec<u64> {
    let ids = [id(0, 3, 1, 2), id(2, 11, 700, 12), id(3, 18, 5, 77_000), id(5, 24, 3_000_000, 4_000_000)];
    let field = Reference(9);
    let mut out = vec![0u64; ids.len()];
    std::thread::scope(|s| {
        for (chunk_ids, chunk_out) in ids.chunks(ids.len().div_ceil(threads)).zip(out.chunks_mut(ids.len().div_ceil(threads))) {
            let field = &field;
            s.spawn(move || {
                for (t, o) in chunk_ids.iter().zip(chunk_out) {
                    *o = compact_tile_field(field, &TangentWarp, *t, 16, R).content_hash();
                }
            });
        }
    });
    out
}

// spec: GEN-019, CON-14
#[test]
fn compact_tiles_are_bit_identical_across_thread_counts_and_match_the_known_answers() {
    let one = tile_hashes(1);
    assert_eq!(one, tile_hashes(4), "thread count changed a tile");
    assert_eq!(one, tile_hashes(2));
    // Known answers (CI runs this test on Windows and Linux): recorded with MESH_VERSION 5,
    // A change of the construction bumps MESH_VERSION and re-records these.
    let expected: [u64; 4] = KNOWN_ANSWERS;
    assert_eq!(one, expected, "compact tile content hashes changed: {one:016x?}");
}

const KNOWN_ANSWERS: [u64; 4] = [0xb841458cf0446591, 0x2f92866cf801bbb0, 0x92b8a9197accb576, 0xa7eb556caa77c84f];

// spec: GEN-019
#[test]
fn a_flat_tile_quantises_without_dividing_by_zero() {
    // A field with no relief inside a tile: step stays positive and finite, every texel level is the same.
    struct Flat;
    impl HeightField for Flat {
        fn height(&self, _: Vec3) -> f64 {
            12.5
        }
        fn seed(&self) -> u64 {
            0
        }
        fn implementation_id(&self) -> &'static str {
            "flat-test"
        }
        fn version(&self) -> u32 {
            1
        }
        fn max_abs_height_m(&self) -> f64 {
            12.5
        }
    }
    let c: CompactTile = compact_tile_field(&Flat, &TangentWarp, id(1, 10, 3, 4), 8, R);
    assert!(c.hr_step.is_finite() && c.hr_step > 0.0);
    assert!(c.texels.iter().all(|t| t[0] == c.texels[0][0]));
    assert_eq!(c.relief_m, 0.0);
    assert_eq!(c.shade_all().len(), vertex_count(8));
}
