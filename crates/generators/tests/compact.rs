//! The compact tile builder (GEN-019, GEN-025, ADR 0033 point 4, ADR 0062): the slot layout and bytes, 15-bit octahedral normals,
//! the 32-bit height step, the CON-13 relief guard and cliff tiles, and determinism. GPU-free.

use planet_core::cube::TangentWarp;
use planet_core::hash::SplitMix64;
use planet_core::{Face, TileId, Vec3};
use planet_generators::compact::{
    compact_tile_field, oct_decode, oct_encode, pack_normal, slot_bytes, slot_texel_bytes, unpack_normal, CompactTile, COMPACT_MIN_LEVEL,
    HEIGHT_STEPS, SLOT_CLIMATE_OFFSET, SLOT_HEADER_BYTES, SLOT_PAD_OFFSET, SLOT_TEXEL_OFFSET, TEXEL_BYTES, WIDE_RELIEF_1MM_M,
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
            // CON-14's mixed bound max(1e-6 |v|, 1 mm) holds wherever the step is at most 2 mm; and every texel level is inside 0..=2^24-1.
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
                    f64::from(qmin) * f64::from(c.hr_step) <= slack
                        && f64::from(HEIGHT_STEPS as u32 - qmax) * f64::from(c.hr_step) <= slack,
                    "tile {t:?}: quantisation levels {qmin}..{qmax}"
                );
            }
            // Every level converts to f32 exactly, so the CPU mirror and the shader form the same `f32(q)` (GEN-019).
            assert!(
                qmax <= HEIGHT_STEPS as u32 && c.texels.iter().all(|t| (t[0] as f32) as u32 == t[0]),
                "tile {t:?}: a level is not exact in f32"
            );
            // The step is at most 1 mm while the relief stays under WIDE_RELIEF_1MM_M (these tiles have far less).
            assert!(c.relief_m < WIDE_RELIEF_1MM_M && c.step_within_con13(), "level {level}: step {} m exceeds 1 mm", c.hr_step);
            checked += 1;
        }
    }
    assert_eq!(checked, 24);
}

/// A test field whose height is `amplitude * dir.x`: a ramp, so a tile's relief is a known multiple of `amplitude`.
struct Ramp(f64);

impl HeightField for Ramp {
    fn height(&self, dir: Vec3) -> f64 {
        self.0 * dir.x
    }
    fn seed(&self) -> u64 {
        1
    }
    fn implementation_id(&self) -> &'static str {
        "test-ramp"
    }
    fn version(&self) -> u32 {
        1
    }
    fn max_abs_height_m(&self) -> f64 {
        self.0.abs()
    }
}

// spec: GEN-019, CON-13
#[test]
fn the_height_step_is_relief_over_2_pow_24_minus_1_so_one_millimetre_holds_to_16_7_km_and_grows_beyond() {
    // The compact format starts at level 3 and has no relief guard and no fallback (ADR 0062): a tile with more relief than
    // WIDE_RELIEF_1MM_M keeps its format and its step is relief / (2^24 - 1), a documented bound (GEN-019), not a failure.
    assert_eq!(COMPACT_MIN_LEVEL, 3);
    assert!((WIDE_RELIEF_1MM_M - 16_777.215).abs() < 1e-9, "1 mm x 2^24-1 steps");
    let t = id(2, 3, 3, 4);
    // Calibrate the ramp so the tile's relief is the target (the relief of a ramp is linear in its amplitude).
    let unit = compact_tile_field(&Ramp(1.0), &TangentWarp, t, 32, R).relief_m;
    assert!(unit > 0.0);
    let mut worst_step_error = 0.0f64;
    for (target, within_mm) in
        [(1_000.0f64, true), (16_700.0, true), (WIDE_RELIEF_1MM_M * 0.999, true), (20_000.0, false), (40_000.0, false)]
    {
        let tile = compact_tile_field(&Ramp(target / unit), &TangentWarp, t, 32, R);
        assert!((tile.relief_m - target).abs() <= 1e-6 * target, "relief {} against the target {target}", tile.relief_m);
        let step = f64::from(tile.hr_step);
        let expected = target / HEIGHT_STEPS;
        assert!((step - expected).abs() <= 2.0e-7 * expected, "relief {target}: step {step} against {expected}");
        assert_eq!(tile.step_within_con13(), within_mm, "relief {target} m: step {step:e} m");
        worst_step_error = worst_step_error.max((step - expected).abs() / expected);
        // The format does not change with the relief: every tile of level 3 and finer is a CompactTile with the same slot.
        assert_eq!(tile.slot_data().len() as u64, slot_bytes(32));
        // The quantisation error of a height is at most half a step.
        assert!((tile.height_error_bound_m() - 0.5 * step).abs() < 1e-12);
    }
    // The documented figures: 1 mm at 16.7 km, about 1.19 mm at 20 km.
    let at_20km = compact_tile_field(&Ramp(20_000.0 / unit), &TangentWarp, t, 32, R);
    assert!((f64::from(at_20km.hr_step) * 1.0e3 - 1.19209).abs() < 1e-3, "{} mm at 20 km", f64::from(at_20km.hr_step) * 1.0e3);
    println!("worst relative error of the step against relief / (2^24 - 1): {worst_step_error:.2e}");
}

// spec: GEN-019, CON-13
#[test]
fn a_finest_tile_is_compact_with_the_base_key_whatever_its_relief() {
    // The steep stress field has kilometres of relief per coarse tile: declaring such a tile the finest used to send it to the vertex
    // format under a key of its class. There is no fallback and no class: the tile is compact and carries the base mesh key.
    let steep = Steep(1);
    let t = id(4, 9, 100, 100);
    let tile = compact_tile_field(&steep, &TangentWarp, t, 32, R);
    assert!(tile.relief_m > 65.535, "fixture: the steep tile has {} m of relief", tile.relief_m);
    let base = mesh_key_field(&steep, &TangentWarp, t, 32, R);
    assert_eq!(tile.key(), base, "a finest tile has its base mesh key, as every other tile");
    // The key a finest tile carried until `remove-vertex-render-path` (the base key rehashed with the finest-level class): a record
    // stored under it is not what a request now computes, so it is orphaned, never returned for other content.
    let legacy_class_key = planet_core::hash::hash_u64s(0x0066_696e, &[base]);
    assert_ne!(tile.key(), legacy_class_key);
    // Level by level, the key never equals the legacy class key of any tile of the sample (a collision would be a 64-bit hash coincidence).
    let mut rng = SplitMix64(7);
    let mut bases = Vec::new();
    for level in [3u8, 5, 9, 14, 22] {
        let n = 1u64 << level;
        for _ in 0..8 {
            let tid = id((rng.next_u64() % 6) as u8, level, (rng.next_u64() % n) as u32, (rng.next_u64() % n) as u32);
            bases.push(mesh_key_field(&Reference(1), &TangentWarp, tid, 32, R));
        }
    }
    let legacy: std::collections::HashSet<u64> = bases.iter().map(|b| planet_core::hash::hash_u64s(0x0066_696e, &[*b])).collect();
    assert!(bases.iter().all(|b| !legacy.contains(b)), "a base key equals the legacy class key of another tile");
    // A fine tile of the reference field has centimetres of relief: compact, with the same key as its mesh.
    let g = id(4, 22, 1000, 2000);
    let d = compact_tile_field(&Reference(1), &TangentWarp, g, 32, R);
    assert_eq!(d.key(), mesh_key_field(&Reference(1), &TangentWarp, g, 32, R));
    assert_eq!(compact_tile_field(&Reference(1), &TangentWarp, id(1, 2, 3, 3), 16, R).cells(), 16);
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
    // Known answers (CI runs this test on Windows and Linux): recorded with MESH_VERSION 7 (the climate in the fourth channel), re-recorded with MESH_VERSION 9 (the lattice normals of ADR 0073; vertex heights and positions unchanged), earlier with MESH_VERSION 8 (the slot of GEN-025: wide texel, climate block and header in the slot), earlier re-recorded with GLOBAL_VERSION 3 (the climate values moved with the latitude axis; the geometry bits did not) and again with GLOBAL_VERSION 5 (the hill band moved the land's heights and, through them, the climate), earlier with GLOBAL_VERSION 4 (the ocean channel changes no sample, but the version word is in the tile's key and its content hash).
    // A change of the construction bumps MESH_VERSION and re-records these.
    assert_eq!(one, KNOWN_ANSWERS, "compact tile content hashes changed: {one:016x?}");
}

const KNOWN_ANSWERS: [u64; 4] = [14590730449213597702, 13202256900304499467, 14422221813819168315, 15511102764023661304];

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
            let c = compact_tile_field(&steep, &TangentWarp, t, 32, R);
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

// spec: GEN-019, CON-13
#[test]
fn the_one_millimetre_step_bound_is_falsifiable_and_ends_at_16_7_km_of_relief() {
    // A cliff of `h` metres across a tile: the step is relief / (2^24 - 1), at most 1 mm exactly while the relief is under 16,777 m.
    struct Cliff(f64);
    impl HeightField for Cliff {
        fn height(&self, d: Vec3) -> f64 {
            if d.y > 0.05 {
                self.0
            } else {
                0.0
            }
        }
        fn seed(&self) -> u64 {
            0
        }
        fn implementation_id(&self) -> &'static str {
            "cliff-test"
        }
        fn version(&self) -> u32 {
            1
        }
        fn max_abs_height_m(&self) -> f64 {
            self.0
        }
    }
    assert!((WIDE_RELIEF_1MM_M - 16_777.215).abs() < 1e-9);
    for (relief, ok) in [(16_000.0, true), (20_000.0, false)] {
        let c = compact_tile_field(&Cliff(relief), &TangentWarp, id(0, 2, 2, 2), 16, R);
        assert!((c.relief_m - relief).abs() < 1e-6, "fixture: the tile straddles the cliff");
        assert_eq!(c.step_within_con13(), ok, "relief {relief} m: step {} m", c.hr_step);
        assert!(c.texels.iter().all(|t| (t[0] as f32) as u32 == t[0]), "levels are exact in f32");
    }
}

// spec: GEN-019, CON-13
#[test]
fn a_ramp_tile_of_about_8_km_reconstructs_within_a_millimetre() {
    // The reconstruction error (not the step) is what CON-13 bounds: `hr_min + f32(q) * hr_step` is exact to 1 mm while the relief stays
    // under 8,192 m, where the f32 spacing of the sum is 2^-11 m. Measured against the f64 height relative to the stored origin height.
    struct Ramp(f64);
    impl HeightField for Ramp {
        fn height(&self, d: Vec3) -> f64 {
            self.0 * d.y
        }
        fn seed(&self) -> u64 {
            0
        }
        fn implementation_id(&self) -> &'static str {
            "ramp-test"
        }
        fn version(&self) -> u32 {
            1
        }
        fn max_abs_height_m(&self) -> f64 {
            self.0.abs()
        }
    }
    let field = Ramp(20_000.0);
    let t = id(0, 2, 2, 2);
    let c = compact_tile_field(&field, &TangentWarp, t, 16, R);
    assert!(c.relief_m > 6_000.0 && c.relief_m < 8_192.0, "fixture: {} m of relief", c.relief_m);
    let mut worst = 0.0f64;
    for j in 0..=16u32 {
        for i in 0..=16u32 {
            let exact = field.height(t.sample_direction(&TangentWarp, 16, i, j)) - f64::from(c.h_origin);
            worst = worst.max((exact - f64::from(c.rel_height(i as usize, j as usize))).abs());
        }
    }
    eprintln!("ramp tile: relief {:.0} m, reconstruction error {worst:.2e} m", c.relief_m);
    assert!(worst <= 1.0e-3, "reconstruction error {worst} m at {} m of relief", c.relief_m);
}
