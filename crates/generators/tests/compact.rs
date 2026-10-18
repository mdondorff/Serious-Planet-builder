//! The compact tile builder (GEN-019, GEN-025, ADR 0033 point 4, ADR 0062): the slot layout and bytes, 15-bit octahedral normals,
//! the 32-bit height step, the CON-13 relief guard and cliff tiles, and determinism. GPU-free.

use planet_core::cube::TangentWarp;
use planet_core::hash::SplitMix64;
use planet_core::{Face, TileId, Vec3};
use planet_generators::compact::{
    compact_tile_field, oct_decode, oct_encode, pack_normal, slot_bytes, slot_texel_bytes, tile_data_field, tile_data_field_guarded,
    unpack_normal, vertex_reason, CompactTile, RenderTile, VertexReason, COMPACT_MIN_LEVEL, FALLBACK_RELIEF_M, SLOT_CLIMATE_OFFSET,
    SLOT_HEADER_BYTES, SLOT_PAD_OFFSET, SLOT_TEXEL_OFFSET, TEXEL_BYTES,
};
use planet_generators::global::{bake, GridSource};
use planet_generators::height::{HeightField, Reference};
use planet_generators::height_fields::{build_field, HeightFieldKind};
use planet_generators::mesh::{finest_class_key, mesh_key_field, tile_mesh_field, vertex_count};
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

// spec: GEN-019, GEN-025
#[test]
fn a_tile_is_a_slot_of_a_header_a_climate_block_a_pad_and_8_byte_texels() {
    assert_eq!(TEXEL_BYTES, 8);
    assert_eq!((SLOT_HEADER_BYTES, SLOT_CLIMATE_OFFSET, SLOT_PAD_OFFSET, SLOT_TEXEL_OFFSET), (128, 128, 452, 456));
    // GEN-025: the stride is 456 + 8 (cells + 1)^2, a multiple of 16 for every even cell count.
    assert_eq!((slot_texel_bytes(32), slot_bytes(32)), (8_712, 9_168));
    assert_eq!((slot_bytes(16), slot_bytes(64)), (2_768, 34_256));
    for cells in [16u32, 32, 64] {
        assert_eq!(slot_bytes(cells) % 16, 0, "cells {cells}");
        let t = compact_tile_field(&Reference(1), &TangentWarp, id(2, 12, 100, 200), cells, R);
        let n = (cells as usize + 1) * (cells as usize + 1);
        assert_eq!(t.texels.len(), n);
        assert_eq!(t.texel_data().len() as u64, slot_texel_bytes(cells));
        assert_eq!(t.resident_bytes(), slot_bytes(cells));
        let slot = t.slot_data();
        assert_eq!(slot.len() as u64, slot_bytes(cells));
        // The header, climate block, pad and texels sit at the offsets of the table.
        assert_eq!(slot[..128], t.header_bytes());
        let word = |at: u64| u32::from_le_bytes(slot[at as usize..at as usize + 4].try_into().unwrap());
        for k in 0..81u64 {
            assert_eq!(word(SLOT_CLIMATE_OFFSET + 4 * k), t.climate_words[k as usize]);
        }
        assert_eq!(word(SLOT_PAD_OFFSET), 0, "the pad word is zero");
        assert_eq!(slot[SLOT_TEXEL_OFFSET as usize..], t.texel_data()[..]);
        for k in [0usize, 1, n / 2, n - 1] {
            assert_eq!(word(SLOT_TEXEL_OFFSET + 8 * k as u64), t.texels[k][0]);
            assert_eq!(word(SLOT_TEXEL_OFFSET + 8 * k as u64 + 4), t.texels[k][1]);
        }
        // No texel carries climate; the two top bits of the normal word are zero.
        assert!(t.texels.iter().all(|x| x[1] >> 30 == 0));
        assert!(t.climate_words.iter().any(|w| *w != 0), "the climate block is filled");
        assert_eq!(t.climate().to_words(), t.climate_words);
        // The header carries the control points and parameters verbatim.
        let word_f = |k: usize| f32::from_le_bytes(slot[k * 4..k * 4 + 4].try_into().unwrap());
        assert_eq!([word_f(0), word_f(1), word_f(2)], t.ctrl[0]);
        assert_eq!([word_f(24), word_f(25), word_f(26)], t.ctrl[8]);
        assert_eq!([word_f(27), word_f(28), word_f(29), word_f(30)], [t.hr_min, t.hr_step, t.h_origin, t.skirt_depth]);
        assert_eq!(word(124), u32::from(t.flip));
        // The slot bytes decode to the tile (the pack payload and the block hold the same bytes).
        let back = CompactTile::from_slot_data(t.id, cells, t.key, t.origin, t.relief_m, &slot).unwrap();
        assert_eq!(back, t);
    }
}

// spec: GEN-025
#[test]
fn a_slot_with_a_wrong_length_or_reserved_bits_is_refused() {
    let t = compact_tile_field(&Reference(1), &TangentWarp, id(2, 12, 100, 200), 16, R);
    let ok = t.slot_data();
    let dec = |b: &[u8]| CompactTile::from_slot_data(t.id, 16, t.key, t.origin, t.relief_m, b);
    assert!(dec(&ok).is_ok());
    assert!(dec(&ok[..ok.len() - 1]).unwrap_err().contains("bytes"));
    let mut b = ok.clone();
    b[124] = 3;
    assert!(dec(&b).unwrap_err().contains("flags"));
    let mut b = ok.clone();
    b[SLOT_PAD_OFFSET as usize] = 1;
    assert!(dec(&b).unwrap_err().contains("pad"));
    let mut b = ok;
    b[SLOT_TEXEL_OFFSET as usize + 7] = 0x40;
    assert!(dec(&b).unwrap_err().contains("reserved"));
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
fn fifteen_bit_octahedral_normals_stay_inside_the_normal_bound() {
    // The measurement over random directions, the six axes, the octahedron's folds and edges, and a dense sweep around the equator of
    // the fold (z = 0) where the encoding is coarsest.
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
        let e = oct_encode(*d);
        assert!(e.iter().all(|c| c.abs() <= 16_383), "components stay inside 15 signed bits");
        assert_eq!(unpack_normal(pack_normal(e)), e, "the packing round-trips");
        assert_eq!(pack_normal(e) >> 30, 0, "bits 30-31 are zero");
        worst = worst.max(angle_deg(oct_decode(e), *d));
    }
    eprintln!("worst oct15 normal error over {} directions: {worst:.5} degrees", dirs.len());
    assert!(worst <= 0.01, "worst 15-bit octahedral error {worst} degrees exceeds CON-14's 0.01");
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
            let dec = oct_decode(c.normal_components(k));
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
            // CON-14's mixed bound max(1e-6 |v|, 1 mm) holds wherever the step is at most 2 mm; and every texel level is inside 0..=2^32-1.
            if f64::from(c.hr_step) <= 2.0e-3 {
                assert!(worst <= 1.0e-6 * f64::from(m.vertices[0].height.abs()).max(1.0e3) + 1.0e-3, "tile {t:?}: {worst}");
            }
            // The range is covered without clamping: the lowest texel is level 0 and the highest the last or next to last.
            let (qmin, qmax) = c.texels.iter().fold((u32::MAX, 0u32), |(lo, hi), t| (lo.min(t[0]), hi.max(t[0])));
            if c.relief_m > 0.0 {
                // The range is rounded outwards in f32: level 0 is at most an f32 ulp of the minimum below the lowest height, and the top
                // level at most an ulp and the rounded-up step above the highest.
                let slack = 3.0e-7 * (f64::from(c.hr_min).abs() + c.relief_m) + f64::from(c.hr_step);
                assert!(
                    f64::from(qmin) * f64::from(c.hr_step) <= slack && f64::from(u32::MAX - qmax) * f64::from(c.hr_step) <= slack,
                    "tile {t:?}: quantisation levels {qmin}..{qmax}"
                );
            }
            // The step is at most 1 mm at every level (relief up to 30 km in one tile).
            assert!(c.step_within_con13(), "level {level}: step {} m exceeds 1 mm", c.hr_step);
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
        RenderTile::Vertex(m) => {
            let product = tile_mesh_field(&steep, &TangentWarp, t, 32, R);
            assert_eq!(m.vertices, product.vertices, "the fallback is the product mesh");
            assert_eq!(m.key, finest_class_key(product.key, t, 9), "under the key of its finest-level class");
        }
        RenderTile::Compact(_) => panic!("a finest tile with {} m of relief must fall back", probe.relief_m),
    }
    // The same tile one level above the finest is compact (CON-13 binds the finest level).
    assert!(tile_data_field(&steep, &TangentWarp, t, 32, R, 10).is_compact());
    // The format depends on the view's finest level, and the key says which class it was decided for (LOD-010): the fallback tile and
    // the compact tile of the same tile have different keys, so one is never reused for the other.
    assert_ne!(tile_data_field(&steep, &TangentWarp, t, 32, R, 9).key(), tile_data_field(&steep, &TangentWarp, t, 32, R, 10).key());
    let RenderTile::Compact(c10) = tile_data_field(&steep, &TangentWarp, t, 32, R, 10) else { unreachable!() };
    assert!(c10.allowed_at_finest_level(10) && !c10.allowed_at_finest_level(9));
    // A fine tile of the reference field has centimetres of relief: compact, with the same key as its mesh.
    let g = id(4, 22, 1000, 2000);
    let d = tile_data_field(&Reference(1), &TangentWarp, g, 32, R, 22);
    assert!(d.is_compact());
    assert_eq!(
        d.key(),
        finest_class_key(mesh_key_field(&Reference(1), &TangentWarp, g, 32, R), g, 22),
        "the key of a finest tile carries its class"
    );
    let below = tile_data_field(&Reference(1), &TangentWarp, g, 32, R, 23);
    assert_eq!(below.key(), mesh_key_field(&Reference(1), &TangentWarp, g, 32, R), "a tile below the cap has the base key");
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
    // Known answers (CI runs this test on Windows and Linux): recorded with MESH_VERSION 7 (the climate in the fourth channel), re-recorded with MESH_VERSION 8 (the slot of GEN-025: wide texel, climate block and header in the slot), earlier re-recorded with GLOBAL_VERSION 3 (the climate values moved with the latitude axis; the geometry bits did not) and again with GLOBAL_VERSION 4 (the ocean channel changes no sample, but the version word is in the tile's key and its content hash).
    // A change of the construction bumps MESH_VERSION and re-records these.
    assert_eq!(one, KNOWN_ANSWERS, "compact tile content hashes changed: {one:016x?}");
}

const KNOWN_ANSWERS: [u64; 4] = [16317442497310121000, 12915908104420781678, 675929460719006013, 16976385179961748364];

// spec: GEN-019, CON-13, CON-14
#[test]
fn cliff_tiles_hold_one_millimetre_with_the_guard_off() {
    // SPIKE-12 T6: finest tiles of the steep stress field (1,875 m of relief at level 9, 805 m at 12, 142 m at 16, 6.7 m at 22) built
    // compact with the guard off, against the canonical mesh through the CPU mirror.
    let steep = Steep(1);
    let mut rng = SplitMix64(9);
    for level in [9u8, 12, 16, 22] {
        let n = 1u32 << level;
        let mut worst_h = 0.0f64;
        let mut worst_n = 0.0f64;
        let mut max_step = 0.0f64;
        let mut max_relief = 0.0f64;
        for _ in 0..3 {
            let t = id((rng.next_u64() % 6) as u8, level, (rng.next_u64() % u64::from(n)) as u32, (rng.next_u64() % u64::from(n)) as u32);
            let RenderTile::Compact(c) = tile_data_field_guarded(&steep, &TangentWarp, t, 32, R, level, false) else {
                panic!("the guard is off: tile {t:?} must be compact")
            };
            let m = tile_mesh_field(&steep, &TangentWarp, t, 32, R);
            let shaded = c.shade_all();
            for (k, v) in m.vertices.iter().enumerate().take(33 * 33) {
                worst_h = worst_h.max((f64::from(v.height) - f64::from(shaded[k].height)).abs());
                let n = Vec3::new(f64::from(v.normal[0]), f64::from(v.normal[1]), f64::from(v.normal[2]));
                worst_n = worst_n.max(angle_deg(shaded[k].normal, n));
            }
            max_step = max_step.max(f64::from(c.hr_step));
            max_relief = max_relief.max(c.relief_m);
        }
        eprintln!(
            "steep level {level}: relief {max_relief:.1} m, step {max_step:.2e} m, height error {worst_h:.2e} m, normal {worst_n:.4} deg"
        );
        assert!(max_step <= 1.0e-3, "level {level}: step {max_step} m");
        assert!(worst_h <= 1.0e-3, "level {level}: height error {worst_h} m");
        assert!(worst_n <= 0.01, "level {level}: normal error {worst_n} degrees");
    }
}

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
    let c: CompactTile = compact_tile_field(&Flat, &TangentWarp, id(1, 10, 3, 4), 16, R);
    assert!(c.hr_step.is_finite() && c.hr_step > 0.0);
    assert!(c.texels.iter().all(|t| t[0] == c.texels[0][0]));
    assert_eq!(c.relief_m, 0.0);
    assert_eq!(c.shade_all().len(), vertex_count(16));
}
