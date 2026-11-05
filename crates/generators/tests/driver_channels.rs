//! Tier A, GEN-119 (ground-channels-2b PR 2): the tile builder fills the driver block from the baked channels a field reads, the mapping of the channels to the codes, the keys of such a field, and the
//! wrappers that forward both. GPU-free.

use std::sync::{Arc, RwLock};

use planet_core::cube::TangentWarp;
use planet_core::hash::mix64;
use planet_core::{Face, TileId, Vec3};
use planet_generators::climate::{CLIMATE_CELLS, CLIMATE_EDGE};
use planet_generators::compact::{compact_tile_field, slot_bytes, CompactTile};
use planet_generators::drivers::{
    apply_channels, apply_depths, channel_word, decode_depth, decode_soil, decode_unit, encode_depth, encode_soil, encode_unit,
    pack_channel_word, unpack_channel_word, unpack_driver_word, wetness_of_area_log2, ChannelSample, DriverParams, AREA_LOG2_DRY,
    AREA_LOG2_WET, CHANNEL_BLOCK_VERSION, DEPTH_MAX_M, DRIVER_MAP_VERSION,
};
use planet_generators::height::{HeightField, Reference};
use planet_generators::mesh::mesh_key_field;
use planet_generators::stamp::StampStage;
use planet_world_def::{BodyId, WorldDefinition, DEFAULT_ITEM_LAYER};

const R: f64 = 6_371_000.0;
const CHANNEL_WORD_1: u64 = 2339158436747412340;

fn tile() -> TileId {
    TileId::new(Face(1), 6, 9, 4).unwrap()
}

/// The channels of a made-up planet: the deposited depth follows the height of the point's z, the area its x.
fn sample_at(d: Vec3) -> ChannelSample {
    ChannelSample {
        soil_m: Some((0.5 + 3.0 * d.z.abs()) as f32),
        area_log2_cells: Some((2.0 + 6.0 * d.x.abs()) as f32),
        fill_m: Some((40.0 * d.y.abs()) as f32),
        erosion_m: Some((9.0 * d.x.abs() + 1.0) as f32),
    }
}

/// `Reference(seed)` with the channels of [`sample_at`] when `identity` is some, keyed like a field that reads them.
struct WithChannels(Reference, Option<u64>);

impl HeightField for WithChannels {
    fn height(&self, dir: Vec3) -> f64 {
        self.0.height(dir)
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
        match self.1 {
            Some(id) => mix64(self.0.climate_version_word() ^ channel_word(id)),
            None => self.0.climate_version_word(),
        }
    }
    fn driver_channels(&self, dirs: &[Vec3]) -> Option<Vec<ChannelSample>> {
        self.1.map(|_| dirs.iter().map(|&d| sample_at(d)).collect())
    }
}

fn texel_dirs() -> Vec<Vec3> {
    (0..CLIMATE_EDGE)
        .flat_map(|b| (0..CLIMATE_EDGE).map(move |a| tile().sample_direction(&TangentWarp, CLIMATE_CELLS, a as u32, b as u32)))
        .collect()
}

// spec: GEN-119
#[test]
fn the_mapping_writes_the_soil_and_moisture_codes_and_leaves_the_insolation_and_a_texel_without_a_channel() {
    let p = DriverParams::default();
    let mut words = [0u32; 81];
    for (k, w) in words.iter_mut().enumerate() {
        *w = planet_generators::drivers::pack_driver_word([10, 20, 30], (k % 200) as u8);
    }
    let before = words;
    let mut samples = vec![ChannelSample { soil_m: None, area_log2_cells: None, ..Default::default() }; 81];
    samples[0] = ChannelSample { soil_m: Some(2.0), area_log2_cells: Some(5.0), ..Default::default() };
    samples[1] = ChannelSample { soil_m: Some(2.0), area_log2_cells: None, ..Default::default() };
    samples[2] = ChannelSample { soil_m: None, area_log2_cells: Some(AREA_LOG2_WET as f32 + 3.0), ..Default::default() };
    apply_channels(&mut words, &samples);
    // The soil code is the square-root code of the depth, the moisture the code of the ramp; the insolation and the neutral insolation stay.
    assert_eq!(unpack_driver_word(words[0]), ([encode_soil(2.0, &p), encode_unit(0.5), 30], 0));
    assert_eq!(unpack_driver_word(words[1]), ([encode_soil(2.0, &p), 20, 30], 1));
    assert_eq!(unpack_driver_word(words[2]), ([10, 255, 30], 2));
    assert_eq!(&words[3..], &before[3..], "a texel without a channel is as it was");
    // The ramp: 0 at the dry end, 1 at the wet end, linear in the log2 between, clamped.
    assert_eq!(
        (wetness_of_area_log2(AREA_LOG2_DRY - 2.0), wetness_of_area_log2(AREA_LOG2_DRY), wetness_of_area_log2(AREA_LOG2_WET)),
        (0.0, 0.0, 1.0)
    );
    assert!((wetness_of_area_log2((AREA_LOG2_DRY + AREA_LOG2_WET) / 2.0) - 0.5).abs() < 1e-12);
    assert_eq!(encode_soil(0.3, &p), 49);
    assert!((decode_soil(49, &p) - 0.3).abs() < 0.0065, "the soil code resolves a third of a metre to half a code step");
    // A sample that is not finite is no channel.
    let mut one = [planet_generators::drivers::pack_driver_word([10, 20, 30], 40); 81];
    let kept = one;
    apply_channels(
        &mut one,
        &vec![ChannelSample { soil_m: Some(f32::NAN), area_log2_cells: Some(f32::INFINITY), ..Default::default() }; 81],
    );
    assert_eq!(one, kept);
    assert_eq!(wetness_of_area_log2(f64::NAN), 0.0);
}

// spec: GEN-119
#[test]
fn the_builder_fills_the_driver_block_from_a_fields_channels_and_a_field_without_them_keeps_the_neutral_block() {
    let (plain, with) = (WithChannels(Reference(7), None), WithChannels(Reference(7), Some(42)));
    let neutral = compact_tile_field(&plain, &TangentWarp, tile(), 32, R).driver_words;
    let neutral_of_reference = compact_tile_field(&Reference(7), &TangentWarp, tile(), 32, R).driver_words;
    assert_eq!(neutral, neutral_of_reference, "a field that reads no channels has the neutral block of every other field");
    let filled = compact_tile_field(&with, &TangentWarp, tile(), 32, R).driver_words;
    assert_ne!(filled, neutral);
    let p = DriverParams::default();
    for (k, d) in texel_dirs().into_iter().enumerate() {
        let s = sample_at(d);
        let (codes, neutral_insolation) = unpack_driver_word(filled[k]);
        let (n_codes, n_neutral) = unpack_driver_word(neutral[k]);
        assert_eq!(codes[0], encode_soil(f64::from(s.soil_m.unwrap()), &p), "texel {k}: the soil code");
        assert_eq!(codes[1], encode_unit(wetness_of_area_log2(f64::from(s.area_log2_cells.unwrap()))), "texel {k}: the moisture code");
        assert_eq!((codes[2], neutral_insolation), (n_codes[2], n_neutral), "texel {k}: the insolation is the neutral one");
        assert!(decode_unit(codes[1]) <= 1.0);
    }
    // Nothing else of the tile changes: the heights are the field's.
    let (a, b) = (compact_tile_field(&with, &TangentWarp, tile(), 32, R), compact_tile_field(&plain, &TangentWarp, tile(), 32, R));
    assert_eq!((a.texels, a.climate_words), (b.texels, b.climate_words));
}

// spec: GEN-119
#[test]
fn the_keys_of_a_field_with_channels_differ_from_one_without_and_from_other_channels_or_another_mapping() {
    let key = |f: &dyn HeightField| mesh_key_field(f, &TangentWarp, tile(), 32, R);
    let (plain, a, a2, b) = (
        WithChannels(Reference(7), None),
        WithChannels(Reference(7), Some(1)),
        WithChannels(Reference(7), Some(1)),
        WithChannels(Reference(7), Some(2)),
    );
    assert_eq!(key(&plain), key(&Reference(7)), "a field without channels keeps the keys it had");
    assert_ne!(key(&a), key(&plain));
    assert_eq!(key(&a), key(&a2), "equal channels give equal keys");
    assert_ne!(key(&a), key(&b), "other channels give other keys");
    assert_ne!(channel_word(1), channel_word(2));
    // Known answer of the channel identity word: it folds DRIVER_MAP_VERSION, the identity, the soil scale, the two area bounds, CHANNEL_BLOCK_VERSION and DEPTH_MAX_M, so a change of any of them
    // (a new depth scale, a new layout) re-keys the tiles of a field that reads channels.
    assert_eq!(channel_word(1), CHANNEL_WORD_1, "the channel word changed: bump what changed and record the new value");
    assert_eq!(DRIVER_MAP_VERSION, 1, "a new mapping raises the version, which enters the word");
}

// spec: GEN-119
#[test]
fn the_wrappers_of_a_field_forward_its_channels() {
    let with = WithChannels(Reference(7), Some(5));
    let dirs = texel_dirs();
    let want = with.driver_channels(&dirs).unwrap();
    // A boxed field and a stamp stage over it give the same channels and the same version word.
    let boxed: Box<dyn HeightField> = Box::new(WithChannels(Reference(7), Some(5)));
    assert_eq!(boxed.driver_channels(&dirs), Some(want.clone()));
    let stage = StampStage::new(
        Arc::new(WithChannels(Reference(7), Some(5))),
        Arc::new(RwLock::new(WorldDefinition::single_planet(R, 7))),
        BodyId::PLANET,
        DEFAULT_ITEM_LAYER,
        0,
        7,
    )
    .expect("a stage");
    assert_eq!(stage.driver_channels(&dirs), Some(want));
    assert_eq!(stage.climate_version_word(), with.climate_version_word());
}

// spec: GEN-119
#[test]
fn every_source_file_that_forwards_the_climate_version_word_forwards_the_channels_too() {
    // A wrapper that forwards the version word and not the channels would hand its tiles the neutral block under the keys of the field that fills it: the files that define one define the other
    // (terrain.rs is a leaf field that reads no channels).
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut stack = vec![root.clone()];
    let mut missing = Vec::new();
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = e.path();
            if path.is_dir() {
                let name = e.file_name().to_string_lossy().into_owned();
                if name != "target" && name != "tests" && name != "examples" && name != "benches" {
                    stack.push(path);
                }
            } else if path.extension().is_some_and(|x| x == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                if text.contains("fn climate_version_word(&self)") && !text.contains("fn driver_channels(&self") {
                    missing.push(path.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/"));
                }
            }
        }
    }
    missing.sort();
    assert_eq!(missing, ["generators/src/terrain.rs"], "a file that defines the version word and not the channels: {missing:?}");
}

// spec: GEN-126
#[test]
fn the_depth_codes_are_square_root_codes_of_the_range_to_64_metres_and_a_texel_without_a_depth_is_zero() {
    assert_eq!((encode_depth(0.0), encode_depth(DEPTH_MAX_M), encode_depth(10.0 * DEPTH_MAX_M), encode_depth(-3.0)), (0, 255, 255, 0));
    // The resolution: 1 mm at the first code, about 3 cm at 0.3 m, 6 cm at 1 m, 0.5 m at the top.
    assert!((decode_depth(1) - 0.001).abs() < 1e-4, "{}", decode_depth(1));
    assert!((decode_depth(32) - 1.0).abs() < 0.02 && (decode_depth(255) - DEPTH_MAX_M).abs() < 1e-9);
    let step_at = |m: f64| decode_depth(encode_depth(m).saturating_add(1)) - decode_depth(encode_depth(m));
    assert!(
        step_at(0.3) < 0.04 && step_at(0.5) < 0.05 && step_at(1.0) < 0.07 && step_at(2.0) < 0.1,
        "the steps at the thresholds of SPIKE-100 D1"
    );
    for m in [0.01, 0.2, 1.0, 7.5, 40.0, 63.0] {
        let code = encode_depth(m);
        let step = decode_depth(code.saturating_add(1)) - decode_depth(code);
        assert!((decode_depth(code) - m).abs() <= step.max(1e-3), "{m} m through code {code}");
    }
    // Rounded, not truncated: 255 sqrt(1 / 64) = 31.9 is code 32; and every code is its own round trip.
    assert_eq!(encode_depth(1.0), 32);
    assert!((0..=255u8).all(|c| encode_depth(decode_depth(c)) == c), "decode then encode is the identity");
    assert_eq!(unpack_channel_word(pack_channel_word(7, 200)), [7, 200]);
    assert_eq!(pack_channel_word(255, 255) >> 16, 0, "bits 16-31 stay zero");
    let mut words = [pack_channel_word(9, 9); 81];
    let mut samples = vec![ChannelSample::default(); 81];
    samples[0] = ChannelSample { fill_m: Some(4.0), erosion_m: Some(100.0), ..Default::default() };
    samples[1] = ChannelSample { fill_m: Some(f32::NAN), erosion_m: Some(f32::INFINITY), ..Default::default() };
    samples[2] = ChannelSample { fill_m: Some(2.0), erosion_m: None, ..Default::default() };
    apply_depths(&mut words, &samples);
    assert_eq!(unpack_channel_word(words[0]), [encode_depth(4.0), encode_depth(100.0)]);
    assert_eq!(words[1], 0, "a depth that is not a finite number is no depth");
    assert_eq!(unpack_channel_word(words[2]), [encode_depth(2.0), 0]);
    assert!(words[3..].iter().all(|w| *w == 0), "a texel without a depth is 0");
    assert_eq!(CHANNEL_BLOCK_VERSION, 1, "a new layout or scale raises the version, which enters the channel word");
}

// spec: GEN-126
#[test]
fn the_builder_writes_the_depths_of_a_fields_channels_in_the_channel_block_and_a_field_without_them_has_the_zero_block() {
    let (plain, with) = (WithChannels(Reference(7), None), WithChannels(Reference(7), Some(42)));
    assert_eq!(compact_tile_field(&plain, &TangentWarp, tile(), 32, R).channel_words, [0u32; 81]);
    assert_eq!(compact_tile_field(&Reference(7), &TangentWarp, tile(), 32, R).channel_words, [0u32; 81]);
    let t = compact_tile_field(&with, &TangentWarp, tile(), 32, R);
    for (k, d) in texel_dirs().into_iter().enumerate() {
        let s = sample_at(d);
        assert_eq!(
            unpack_channel_word(t.channel_words[k]),
            [encode_depth(f64::from(s.fill_m.unwrap())), encode_depth(f64::from(s.erosion_m.unwrap()))],
            "texel {k}"
        );
        assert_eq!(t.channel_words[k] >> 16, 0, "texel {k}: the reserved bits are zero");
    }
    // The driver block and the heights are the ones of GEN-119 and the field: the depths add no other change.
    let b = compact_tile_field(&plain, &TangentWarp, tile(), 32, R);
    assert_eq!((&t.texels, t.climate_words), (&b.texels, b.climate_words));
    assert_ne!(t.driver_words, b.driver_words);
    // The slot holds the block after the driver block, round-trips, and a reserved bit is refused.
    let bytes = t.slot_data();
    assert_eq!(bytes.len() as u64, slot_bytes(32));
    assert_eq!(bytes.len(), 9_812);
    let back = CompactTile::from_slot_data(t.id, 32, t.key, t.origin, t.relief_m, &bytes).expect("a slot");
    assert_eq!(back.channel_words, t.channel_words);
    assert_eq!(back, t);
    let mut bad = bytes.clone();
    bad[planet_generators::compact::SLOT_CHANNEL_OFFSET as usize + 2] = 1;
    let e = CompactTile::from_slot_data(t.id, 32, t.key, t.origin, t.relief_m, &bad).unwrap_err();
    assert!(e.contains("channel word 0") && e.contains("reserved"), "{e}");
    // Keys: the channel identity moves the key (GEN-119), and the mesh version is the one of this layout.
    assert_eq!(planet_generators::mesh::MESH_VERSION, 11);
}
