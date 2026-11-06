//! GEN-021: the climate field of a tile (ADR 0039 point 2, ADR 0027): f16 rounding, the integer reader's values, known answers,
//! thread independence, shared borders, the compact tile's spare channel and the keys.

use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId};
use planet_generators::climate::{
    analytic_climate, climate_tile, climate_version_word, f16_from_f32, f16_to_f32, ClimateTexture, CLIMATE_BYTES, CLIMATE_CELLS,
    CLIMATE_EDGE, CLIMATE_TEXELS,
};
use planet_generators::compact::compact_tile_field;
use planet_generators::global::{bake, read_tile_sample, GridSource};
use planet_generators::height::{HeightField, Reference};
use planet_generators::mesh::{mesh_key_field, tile_mesh_field};

const SEED: u64 = 7;
const R: f64 = 6_371_000.0;

fn id(face: u8, level: u8, x: u32, y: u32) -> TileId {
    TileId::new(Face(face), level, x, y).unwrap()
}

// spec: GEN-021
#[test]
fn f16_conversion_is_exact_one_way_and_round_to_nearest_even_the_other() {
    // Every finite f16 survives the round trip through f32.
    let mut finite = 0;
    for h in 0..=u16::MAX {
        let exp = (h >> 10) & 0x1f;
        if exp == 0x1f {
            continue;
        }
        finite += 1;
        let x = f16_to_f32(h);
        assert_eq!(f16_from_f32(x), h, "{h:#06x} -> {x}");
    }
    assert_eq!(finite, 63_488);
    // Known values.
    for (x, h) in
        [(0.0f32, 0u16), (1.0, 0x3c00), (-2.0, 0xc000), (0.5, 0x3800), (65_504.0, 0x7bff), (6.103_515_6e-5, 0x0400), (5.960_464_5e-8, 1)]
    {
        assert_eq!(f16_from_f32(x), h, "{x}");
        assert_eq!(f16_to_f32(h), x, "{h:#06x}");
    }
    // Ties go to even: 2049 lies between 2048 (0x6800, even) and 2050 (0x6801, odd) and rounds to 2048; 2051 to 2052.
    assert_eq!(f16_from_f32(2049.0), 0x6800);
    assert_eq!(f16_from_f32(2051.0), 0x6802);
    // Just above and below a tie.
    assert_eq!(f16_from_f32(f32::from_bits(2049.0f32.to_bits() + 1)), 0x6801);
    assert_eq!(f16_from_f32(f32::from_bits(2049.0f32.to_bits() - 1)), 0x6800);
    // Overflow, subnormal underflow and the smallest subnormal's tie.
    assert_eq!(f16_from_f32(70_000.0), 0x7c00);
    assert_eq!(f16_from_f32(65_520.0), 0x7c00, "the tie above the largest finite value rounds to even, which is infinity");
    assert_eq!(f16_from_f32(2.0e-8), 0, "below half the smallest subnormal");
    assert_eq!(f16_from_f32(2.980_232_2e-8), 0, "exactly half the smallest subnormal: tie to even (zero)");
    assert_eq!(f16_from_f32(-1.0e-30), 0x8000);
    // The rounding error is at most 2^-11 of the value for normal numbers.
    for k in 1..2000 {
        let x = k as f32 * 2.347;
        let back = f16_to_f32(f16_from_f32(x));
        assert!(((back - x) / x).abs() <= 1.0 / 2048.0, "{x} -> {back}");
    }
}

// spec: GEN-021
#[test]
fn the_climate_of_a_tile_is_the_integer_readers_value_rounded_to_f16() {
    let g = bake(SEED, &TangentWarp, 64, 2).unwrap();
    let src = GridSource(&g);
    for t in [id(0, 0, 0, 0), id(2, 3, 5, 2), id(5, 6, 40, 63), id(3, 9, 100, 200)] {
        let c = climate_tile(&src, t).unwrap();
        for b in 0..CLIMATE_EDGE {
            for a in 0..CLIMATE_EDGE {
                let s = read_tile_sample(&src, t, CLIMATE_CELLS, a as i64, b as i64).unwrap();
                let (tc, pm) = c.texel(a, b);
                assert_eq!(tc, f16_to_f32(f16_from_f32(s.temperature_c)), "{t:?} ({a}, {b}) temperature");
                assert_eq!(pm, f16_to_f32(f16_from_f32(s.precipitation_mm)), "{t:?} ({a}, {b}) precipitation");
                // The declared quantisation against the bake: 2^-11 of the value.
                assert!((f64::from(tc) - f64::from(s.temperature_c)).abs() <= f64::from(s.temperature_c).abs() / 2048.0 + 1e-6);
                assert!((f64::from(pm) - f64::from(s.precipitation_mm)).abs() <= f64::from(s.precipitation_mm).abs() / 2048.0 + 1e-6);
            }
        }
    }
}

// spec: GEN-021
#[test]
fn neighbouring_tiles_share_their_border_texels_also_across_a_face_edge() {
    let g = bake(SEED, &TangentWarp, 64, 2).unwrap();
    let src = GridSource(&g);
    // Same face: tile (x, y) and (x + 1, y) at level 4.
    let (l, r) = (climate_tile(&src, id(1, 4, 6, 9)).unwrap(), climate_tile(&src, id(1, 4, 7, 9)).unwrap());
    for b in 0..CLIMATE_EDGE {
        assert_eq!(l.texel(CLIMATE_EDGE - 1, b), r.texel(0, b), "row {b}");
    }
    // Across face edges: every pair of texels of two tiles of different faces that have bit-equal directions has equal values.
    let mut matched = 0;
    for f in 0..6u8 {
        for other in (f + 1)..6u8 {
            for (x, y) in [(0u32, 0u32), (0, 3), (3, 0), (3, 3), (0, 1), (2, 3), (3, 2)] {
                for (x2, y2) in [(0u32, 0u32), (0, 3), (3, 0), (3, 3), (0, 1), (2, 3), (3, 2)] {
                    let (a, b) = (id(f, 2, x, y), id(other, 2, x2, y2));
                    let (ca, cb) = (climate_tile(&src, a).unwrap(), climate_tile(&src, b).unwrap());
                    for ia in 0..CLIMATE_EDGE {
                        for ib in 0..CLIMATE_EDGE {
                            let da = a.sample_direction(&TangentWarp, CLIMATE_CELLS, ia as u32, ib as u32);
                            for ja in 0..CLIMATE_EDGE {
                                for jb in 0..CLIMATE_EDGE {
                                    let d2 = b.sample_direction(&TangentWarp, CLIMATE_CELLS, ja as u32, jb as u32);
                                    if (da.x.to_bits(), da.y.to_bits(), da.z.to_bits()) == (d2.x.to_bits(), d2.y.to_bits(), d2.z.to_bits())
                                    {
                                        assert_eq!(ca.texel(ia, ib), cb.texel(ja, jb), "faces {f} and {other}");
                                        matched += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(matched > 0, "no shared border texels were compared across faces");
}

fn hashes(threads: usize) -> Vec<u64> {
    let g = bake(SEED, &TangentWarp, 64, 2).unwrap();
    let tiles: Vec<TileId> = (0..24u32).map(|k| id((k % 6) as u8, 5, (k * 7) % 32, (k * 13) % 32)).collect();
    let mut out = vec![0u64; tiles.len()];
    std::thread::scope(|s| {
        let per = tiles.len().div_ceil(threads);
        for (chunk_t, chunk_o) in tiles.chunks(per).zip(out.chunks_mut(per)) {
            let g = &g;
            s.spawn(move || {
                let src = GridSource(g);
                for (t, o) in chunk_t.iter().zip(chunk_o) {
                    *o = climate_tile(&src, *t).unwrap().content_hash();
                }
            });
        }
    });
    out
}

// spec: GEN-021, CON-14
#[test]
fn the_climate_is_bit_identical_across_thread_counts_and_matches_the_known_answers() {
    let one = hashes(1);
    assert_eq!(one, hashes(5), "thread count changed a climate texture");
    assert_eq!(one, hashes(24));
    // Known answers (CI runs this on Windows and Linux): CLIMATE_VERSION 1, READER_VERSION 1, GLOBAL_VERSION 5 (4 -> 5: the hill band moved the heights of land and so the temperature texels; the construction is unchanged, so CLIMATE_VERSION stays; 3 -> 4: the ocean channel, no texel moves; 2 -> 3: latitude from COORD's +z pole instead of d.y; the texel construction is unchanged, so CLIMATE_VERSION stays and the version word moves through GLOBAL_VERSION).
    let probe: Vec<u64> = vec![one[0], one[1], one[2], one[23]];
    assert_eq!(probe, KNOWN_CLIMATE, "climate content hashes changed: {probe:016x?}: bump CLIMATE_VERSION and re-record");
    assert_eq!(climate_version_word(), KNOWN_VERSION_WORD, "the version word changed: {:#018x}", climate_version_word());
}

const KNOWN_CLIMATE: [u64; 4] = [0xa462d5f57fe8be1f, 0x4066977cddc7c722, 0x13affc8130ddf2d4, 0x910e00dd1bec3c30];
const KNOWN_VERSION_WORD: u64 = 0xbe72ef1acd37fcd8; // moved with GLOBAL_VERSION 8 (the shelf profile, GEN-118; was 0x04c6bc3b08350c89 under 7); then GLOBAL_VERSION 7 (the orographic climate, GEN-117; was 0x2097a6a7c9a9271c under 6)

// spec: GEN-021
#[test]
fn the_analytic_climate_is_a_pure_function_of_seed_and_tile() {
    let t = id(4, 7, 33, 90);
    let a = analytic_climate(SEED, &TangentWarp, t);
    assert_eq!(a, analytic_climate(SEED, &TangentWarp, t));
    assert_ne!(a, analytic_climate(SEED + 1, &TangentWarp, t));
    assert_eq!(a, Reference(SEED).climate(&TangentWarp, t), "the reference field's default is the analytic climate");
    let equator = analytic_climate(SEED, &TangentWarp, id(0, 3, 4, 4));
    // Face 4 is the +z face: its centre tile is at the north pole of COORD (latitude about 90), face 0 (+x) is on the equator.
    let pole = analytic_climate(SEED, &TangentWarp, id(4, 3, 4, 4));
    let (t_eq, t_pole) = (equator.texel(4, 4).0, pole.texel(4, 4).0);
    assert!(t_eq > t_pole + 30.0, "equator {t_eq} C, pole {t_pole} C");
    let south = analytic_climate(SEED, &TangentWarp, id(5, 3, 4, 4));
    assert!(t_eq > south.texel(4, 4).0 + 30.0, "the south pole (-z face) is cold too");
}

// spec: GEN-021
#[test]
fn the_texture_round_trips_through_its_bytes_and_the_slot_words() {
    let a = analytic_climate(SEED, &TangentWarp, id(1, 6, 10, 20));
    assert_eq!(ClimateTexture::from_bytes(&a.to_bytes()), a);
    assert_eq!(a.to_bytes().len(), CLIMATE_BYTES);
    assert_eq!(CLIMATE_BYTES, 324);
    // The slot's climate block (GEN-025): 81 words, temperature in the low half, precipitation in the high half.
    let words = a.to_words();
    assert_eq!(words.len(), CLIMATE_TEXELS);
    assert_eq!(ClimateTexture::from_words(&words), a);
    for (k, w) in words.iter().enumerate() {
        assert_eq!((w & 0xffff) as u16, a.temperature[k]);
        assert_eq!((w >> 16) as u16, a.precipitation[k]);
    }
}

// spec: GEN-021, GEN-019
#[test]
fn a_compact_tile_carries_the_climate_of_its_field_and_a_vertex_tile_the_same() {
    let t = id(2, 8, 77, 31);
    let f = Reference(SEED);
    let c = compact_tile_field(&f, &TangentWarp, t, 32, R);
    let m = tile_mesh_field(&f, &TangentWarp, t, 32, R);
    assert_eq!(c.climate(), f.climate(&TangentWarp, t));
    assert_eq!(m.climate, c.climate(), "a tile has one climate whichever format it is in");
    // The texels hold 8,712 bytes whatever the climate; the climate block is 324 bytes of the slot (GEN-025).
    assert_eq!(c.texel_data().len(), 8_712);
    let slot = c.slot_data();
    assert_eq!(slot[128..452], c.climate().to_bytes(), "the slot's climate block is the pack's 324 bytes");
    assert!(c.texels.iter().all(|t| t[1] >> 30 == 0), "no texel carries climate");
}

struct Other<'a>(&'a Reference);

impl HeightField for Other<'_> {
    fn height(&self, d: planet_core::Vec3) -> f64 {
        self.0.height(d)
    }
    fn seed(&self) -> u64 {
        self.0.seed()
    }
    fn implementation_id(&self) -> &'static str {
        self.0.implementation_id()
    }
    fn version(&self) -> u32 {
        self.0.version()
    }
    fn max_abs_height_m(&self) -> f64 {
        self.0.max_abs_height_m()
    }
    fn climate_version_word(&self) -> u64 {
        12345
    }
}

// spec: GEN-021
#[test]
fn the_mesh_key_names_the_climate_version_and_the_bilinear_lookup_is_exact_at_texels() {
    let f = Reference(SEED);
    let t = id(2, 8, 77, 31);
    let k = mesh_key_field(&f, &TangentWarp, t, 32, R);
    assert_ne!(k, mesh_key_field(&Other(&f), &TangentWarp, t, 32, R), "a field with another climate version keys differently");
    // The lookup is the texel at texel positions and linear between.
    let c = analytic_climate(SEED, &TangentWarp, t);
    for (a, b) in [(0usize, 0usize), (3, 5), (8, 8), (8, 0)] {
        assert_eq!(c.sample(a as f32 / 8.0, b as f32 / 8.0), c.texel(a, b));
    }
    let (m0, m1) = (c.texel(2, 3), c.texel(3, 3));
    let mid = c.sample(2.5 / 8.0, 3.0 / 8.0);
    assert!((mid.0 - 0.5 * (m0.0 + m1.0)).abs() < 1e-5 && (mid.1 - 0.5 * (m0.1 + m1.1)).abs() < 1e-3);
    // A vertex at a texel position takes the texel's value.
    assert_eq!(c.vertex_value(8, 12, 32), [c.temperature[3 * 9 + 2], c.precipitation[3 * 9 + 2]]);
}

// spec: GEN-021, GEN-010
#[test]
fn fields_over_a_bake_take_their_climate_from_the_integer_reader_and_key_with_the_bake_version_word() {
    use planet_generators::erosion::ErosionGenerator;
    use planet_generators::terrain::TerrainGenerator;
    use planet_world_def::{BodyId, GenContext, WorldDefinition};
    let g = bake(SEED, &TangentWarp, 64, 2).unwrap();
    let src = GridSource(&g);
    let def = WorldDefinition::single_planet(R, SEED);
    let t = id(3, 6, 20, 41);
    let want = climate_tile(&src, t).unwrap();
    let key = [7u8; 16];
    let terrain = TerrainGenerator.field(&GenContext::new(&def), BodyId::PLANET, &src, key).unwrap();
    let eroded = ErosionGenerator.field(&GenContext::new(&def), BodyId::PLANET, &src, key).unwrap();
    for (name, f) in [("terrain", &terrain as &dyn HeightField), ("eroded", &eroded as &dyn HeightField)] {
        assert_eq!(f.climate(&TangentWarp, t), want, "{name}: the climate is the reader's, not the analytic function's");
        assert_eq!(f.climate_version_word(), climate_version_word(), "{name}");
        assert_ne!(f.climate(&TangentWarp, t), analytic_climate(SEED, &TangentWarp, t), "{name}: the bake and the analytic climate differ");
    }
    assert_eq!(climate_tile(&src, t).unwrap(), want);
    assert_ne!(climate_version_word(), planet_generators::climate::analytic_version_word());
}

/// A field of a constant height (its own world, not the generator's).
struct Level(f64);

impl HeightField for Level {
    fn height(&self, _: planet_core::Vec3) -> f64 {
        self.0
    }
    fn seed(&self) -> u64 {
        SEED
    }
    fn implementation_id(&self) -> &'static str {
        "test-level"
    }
    fn version(&self) -> u32 {
        1
    }
    fn max_abs_height_m(&self) -> f64 {
        self.0.abs()
    }
}

// spec: GEN-021
#[test]
fn the_climate_of_a_field_without_a_bake_is_the_formula_over_its_own_heights_and_not_the_generators_world() {
    use planet_generators::climate::{analytic_version_word, height_climate};
    use planet_generators::global::climate_from_elevation;
    use planet_generators::height::height_at;
    let (f, t) = (Reference(SEED), id(2, 8, 77, 31));
    let got = f.climate(&TangentWarp, t);
    // Exactly the formula applied to the reference generator's own height at each texel.
    let (mut temp, mut precip) = ([0f32; CLIMATE_TEXELS], [0f32; CLIMATE_TEXELS]);
    for b in 0..CLIMATE_EDGE {
        for a in 0..CLIMATE_EDGE {
            let d = t.sample_direction(&TangentWarp, CLIMATE_CELLS, a as u32, b as u32);
            (temp[b * CLIMATE_EDGE + a], precip[b * CLIMATE_EDGE + a]) = climate_from_elevation(SEED, d, height_at(SEED, d), 0.0);
        }
    }
    assert_eq!(got, ClimateTexture::from_f32(&temp, &precip), "the reference field's climate is the formula over its own heights");
    assert_eq!(got, height_climate(SEED, &TangentWarp, t, &|d| height_at(SEED, d)));
    // Not the generator's: the same seed's global-coarse field (its continents) gives other texels.
    let differs = (0..6u8)
        .flat_map(|face| (0..4u32).map(move |x| id(face, 2, x, 1)))
        .any(|tile| f.climate(&TangentWarp, tile) != analytic_climate(SEED, &TangentWarp, tile));
    assert!(
        differs,
        "the reference field's climate does not read the generator's world: some tile of level 2 differs from the generator's"
    );
    // The version word names the function, not the generator: it does not fold GLOBAL_VERSION (a generator change moves no such field's keys).
    assert_ne!(analytic_version_word(), climate_version_word());
}

// spec: GEN-021
#[test]
fn the_temperature_of_a_field_without_a_bake_follows_its_own_altitude() {
    let t = id(1, 6, 10, 20);
    let (low, high) = (Level(0.0).climate(&TangentWarp, t), Level(2_000.0).climate(&TangentWarp, t));
    for b in 0..CLIMATE_EDGE {
        for a in 0..CLIMATE_EDGE {
            let drop = low.texel(a, b).0 - high.texel(a, b).0;
            // 6.5 degrees per km over 2 km, up to the f16 rounding of both values.
            assert!((drop - 13.0).abs() < 0.05, "texel ({a}, {b}) cools by {drop}, not 13 degrees, over 2 km");
            assert_eq!(low.texel(a, b).1, high.texel(a, b).1, "the rain does not depend on the altitude");
        }
    }
    // A field below sea level is as warm as one at sea level (the altitude is clamped at 0, as in the generator's formula).
    assert_eq!(Level(-500.0).climate(&TangentWarp, t), low);
}
