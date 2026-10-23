//! STRM-043: the global coupled run stored as a channel.

use std::fs;
use std::path::PathBuf;

use planet_cache::global_store::{
    decode_coupled_block, encode_coupled_block, load_coupled, store_coupled, BlockCodecError, CoupledBlock, CoupledSpec, GlobalWorld,
    COUPLED_CODEC_VERSION,
};
use planet_cache::pack::{TileStore, DEFAULT_MAX_PACK_BYTES};
use planet_core::{Face, TileId};
use planet_generators::coupled::{run, CoupledInput, CoupledParams, Lattice};
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;
const N: u32 = 16;

fn dir(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("planet-coupled-store-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&p);
    p
}

fn world(spec: CoupledSpec) -> GlobalWorld {
    // A block limit of 4 gives blocks of 4 x 4 cells at level 2.
    GlobalWorld::from_definition(&WorldDefinition::single_planet(R, 3)).unwrap().with_block_limit(4).with_coupled(spec)
}

fn spec() -> CoupledSpec {
    CoupledSpec { n: N, steps: 6, bake_res: N, params: CoupledParams::default(), inputs: 0 }
}

fn state() -> planet_generators::coupled::CoupledState {
    let lat = Lattice::new(N as usize, R);
    let z: Vec<f32> = (0..lat.len())
        .map(|c| {
            let d = lat.direction(c);
            (3000.0 * (d.x * d.y + 0.5 * d.z * d.x) + 200.0) as f32
        })
        .collect();
    let input = CoupledInput { uplift: vec![1.0e-3; z.len()], erodibility: vec![2.0e-5; z.len()], z };
    run(&lat, &input, &spec().params, spec().steps as usize)
}

// spec: STRM-043
#[test]
fn a_stored_run_reads_back_bit_for_bit() {
    let d = dir("roundtrip");
    let w = world(spec());
    let s = state();
    let written = store_coupled(&d, &w, &s, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert_eq!(written, 6 * 16, "six faces of 4 x 4 blocks");
    let store = TileStore::open(&d, w.params()).unwrap();
    let back = load_coupled(&store, &w).unwrap();
    assert_eq!(back.n, N);
    assert!(back.z.iter().zip(&s.z).all(|(a, b)| a.to_bits() == b.to_bits()));
    assert!(back.area_m2.iter().zip(&s.area_m2).all(|(a, b)| a.to_bits() == b.to_bits()));
    assert_eq!(back.receiver, s.receiver);
    let _ = fs::remove_dir_all(&d);
}

// spec: STRM-043
#[test]
fn a_changed_run_parameter_the_steps_or_the_base_resolution_changes_every_key() {
    let tile = TileId::new(Face(2), 2, 1, 3).unwrap();
    let base = world(spec()).coupled_key(tile, 4);
    assert_eq!(base, world(spec()).coupled_key(tile, 4));
    let mut seen = vec![base];
    let variants = [
        CoupledSpec { steps: 7, ..spec() },
        CoupledSpec { bake_res: N * 2, ..spec() },
        CoupledSpec { params: CoupledParams { jitter: 0.5, ..spec().params }, ..spec() },
        CoupledSpec { params: CoupledParams { area_exp: 0.5, ..spec().params }, ..spec() },
        CoupledSpec { params: CoupledParams { dt_years: 2.0e5, ..spec().params }, ..spec() },
        CoupledSpec { params: CoupledParams { seed: 1, ..spec().params }, ..spec() },
        CoupledSpec { params: CoupledParams { min_height_m: 1.0, ..spec().params }, ..spec() },
        CoupledSpec { n: N * 2, ..spec() },
        CoupledSpec { inputs: 1, ..spec() },
    ];
    for v in variants {
        let k = world(v).coupled_key(tile, 4);
        assert!(!seen.contains(&k), "{v:?} gave a key already seen");
        seen.push(k);
    }
    assert_ne!(world(spec()).coupled_key(tile, 4), world(spec()).coupled_key(TileId::new(Face(2), 2, 1, 2).unwrap(), 4));
}

fn block(cells: u32) -> CoupledBlock {
    let n = (cells * cells) as usize;
    CoupledBlock {
        cells,
        // Finite values, a signalling NaN, a negative quiet NaN, -0.0, a subnormal and infinity: the bits must survive.
        z: (0..n)
            .map(|i| match i {
                1 => f32::from_bits(0x7F80_0001),
                2 => f32::from_bits(0xFFC0_1234),
                3 => -0.0,
                4 => f32::from_bits(1),
                5 => f32::INFINITY,
                _ => f32::from_bits(0x4000_0000 ^ (i as u32) << 5),
            })
            .collect(),
        area_m2: (0..n).map(|i| i as f32 * 1.5e6 + 0.25).collect(),
        receiver: (0..n).map(|i| if i % 7 == 0 { u32::MAX } else { i as u32 * 3 }).collect(),
    }
}

// spec: STRM-043
#[test]
fn a_block_round_trips_and_a_bad_payload_is_refused() {
    let b = block(4);
    let bytes = encode_coupled_block(&b).unwrap();
    assert_eq!(bytes[0], COUPLED_CODEC_VERSION);
    let back = decode_coupled_block(&bytes).unwrap();
    assert!(back.z.iter().zip(&b.z).all(|(x, y)| x.to_bits() == y.to_bits()), "the bits of z survive");
    assert_eq!((&back.area_m2, &back.receiver, back.cells), (&b.area_m2, &b.receiver, b.cells));
    assert!(matches!(decode_coupled_block(&bytes[..5]), Err(BlockCodecError::Short)));
    let mut other = bytes.clone();
    other[0] = 9;
    assert!(matches!(decode_coupled_block(&other), Err(BlockCodecError::CoupledVersion(9))));
    let mut cells = bytes.clone();
    cells[1..5].copy_from_slice(&8u32.to_le_bytes());
    assert!(matches!(decode_coupled_block(&cells), Err(BlockCodecError::RawLength { .. })));
    assert!(decode_coupled_block(&bytes[..bytes.len() - 3]).is_err());
    let mut short = block(4);
    short.z.pop();
    assert!(matches!(encode_coupled_block(&short), Err(BlockCodecError::Samples { .. })));
}

// spec: STRM-043
#[test]
fn a_missing_or_foreign_block_is_an_error_naming_the_block() {
    let d = dir("missing");
    let w = world(spec());
    store_coupled(&d, &w, &state(), DEFAULT_MAX_PACK_BYTES).unwrap();
    let store = TileStore::open(&d, w.params()).unwrap();
    // The same store read under another run's key: nothing is stored under it.
    let err = load_coupled(&store, &world(CoupledSpec { steps: 7, ..spec() })).err().expect("another run has no blocks here");
    assert!(err.to_string().contains("coupled block"), "{err}");
    let _ = fs::remove_dir_all(&d);
}
