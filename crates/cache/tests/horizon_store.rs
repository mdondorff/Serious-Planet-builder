//! STRM-045: the coarse horizon of the global run as a stored channel.

use std::fs;
use std::path::PathBuf;

use planet_cache::global_store::{BlockCodecError, CoupledSpec, GlobalWorld, HorizonSpec};
use planet_cache::horizon_store::{
    decode_horizon_block, encode_horizon_block, load_horizon, store_horizon, HorizonBlock, HORIZON_CODEC_VERSION,
};
use planet_cache::pack::{TileStore, DEFAULT_MAX_PACK_BYTES};
use planet_core::{Face, TileId};
use planet_generators::coarsehorizon::{coarse_horizon, CoarseParams, MAX_AZIMUTHS};
use planet_generators::coupled::{run, CoupledInput, CoupledParams, Lattice};
use planet_world_def::WorldDefinition;

const R: f64 = 100_000.0;
const N: u32 = 16;
const M: u32 = 8;

fn dir(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("planet-horizon-store-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&p);
    p
}

fn coupled() -> CoupledSpec {
    CoupledSpec { n: N, steps: 4, bake_res: N, params: CoupledParams::default(), inputs: 0 }
}

fn horizon() -> HorizonSpec {
    HorizonSpec { m: M, params: CoarseParams { azimuths: 8, range_m: 30_000.0, step_m: 6_000.0, radius_m: R } }
}

fn world(c: CoupledSpec, h: HorizonSpec) -> GlobalWorld {
    // A block limit of 4 gives blocks of 4 x 4 observer cells at level 1.
    GlobalWorld::from_definition(&WorldDefinition::single_planet(R, 3)).unwrap().with_block_limit(4).with_coupled(c).with_horizon(h)
}

/// The map of a small run: heights from the direction, run for a few steps, marched from the observer lattice.
fn map() -> Vec<u8> {
    let lat = Lattice::new(N as usize, R);
    let z: Vec<f32> = (0..lat.len())
        .map(|c| {
            let d = lat.direction(c);
            (3000.0 * (d.x * d.y + 0.5 * d.z * d.x) + 200.0) as f32
        })
        .collect();
    let input = CoupledInput { uplift: vec![1.0e-3; z.len()], erodibility: vec![2.0e-5; z.len()], z };
    let state = run(&lat, &input, &coupled().params, coupled().steps as usize);
    coarse_horizon(&lat, &state.z, M as usize, &horizon().params, 2).unwrap()
}

// spec: STRM-045
#[test]
fn a_stored_coarse_horizon_reads_back_bit_for_bit() {
    let d = dir("roundtrip");
    let w = world(coupled(), horizon());
    let m = map();
    assert!(m.iter().any(|&q| q > 0), "the map shades something");
    let written = store_horizon(&d, &w, &m, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert_eq!(written, 6 * 4, "six faces of 2 x 2 blocks");
    let store = TileStore::open(&d, w.params()).unwrap();
    let back = load_horizon(&store, &w).unwrap();
    assert_eq!((back.m, back.azimuths), (M, 8));
    assert_eq!(back.bytes, m);
    let _ = fs::remove_dir_all(&d);
}

// spec: STRM-045
#[test]
fn the_block_of_a_tile_holds_the_observer_cells_under_it() {
    let w = world(coupled(), horizon());
    let m = map();
    let (n, a) = (M as usize, 8usize);
    let records = w.horizon_records(&m).unwrap();
    for (face, tx, ty) in [(0u8, 0u32, 0u32), (2, 1, 1), (5, 0, 1), (3, 1, 0)] {
        let tile = TileId::new(Face(face), 1, tx, ty).unwrap();
        let block = decode_horizon_block(&records.iter().find(|r| r.tile == tile).expect("a record per tile").payload).unwrap();
        assert_eq!((block.cells, block.azimuths), (4, 8));
        for j in 0..4usize {
            for i in 0..4usize {
                let cell = usize::from(face) * n * n + (ty as usize * 4 + j) * n + tx as usize * 4 + i;
                assert_eq!(
                    &block.bytes[(j * 4 + i) * a..(j * 4 + i + 1) * a],
                    &m[cell * a..(cell + 1) * a],
                    "tile ({face},{tx},{ty}) cell ({i},{j})"
                );
            }
        }
    }
}

// spec: STRM-045
#[test]
fn a_changed_march_the_observer_lattice_or_the_coupled_run_changes_every_key() {
    let tile = TileId::new(Face(2), 1, 1, 0).unwrap();
    let base = world(coupled(), horizon()).horizon_key(tile, 4);
    assert_eq!(base, world(coupled(), horizon()).horizon_key(tile, 4));
    let h = horizon();
    let mut seen = vec![base];
    let variants = [
        (coupled(), HorizonSpec { params: CoarseParams { azimuths: 16, ..h.params }, ..h }),
        (coupled(), HorizonSpec { params: CoarseParams { range_m: 40_000.0, ..h.params }, ..h }),
        (coupled(), HorizonSpec { params: CoarseParams { step_m: 5_000.0, ..h.params }, ..h }),
        (coupled(), HorizonSpec { params: CoarseParams { radius_m: R * 2.0, ..h.params }, ..h }),
        (coupled(), HorizonSpec { m: M * 2, ..h }),
        (CoupledSpec { steps: 7, ..coupled() }, h),
        (CoupledSpec { inputs: 1, ..coupled() }, h),
        (CoupledSpec { n: N * 2, ..coupled() }, h),
        (CoupledSpec { bake_res: N * 2, ..coupled() }, h),
        (CoupledSpec { params: CoupledParams { jitter: 0.6, ..coupled().params }, ..coupled() }, h),
        (CoupledSpec { params: CoupledParams { seed: 5, ..coupled().params }, ..coupled() }, h),
    ];
    for (c, spec) in variants {
        let k = world(c, spec).horizon_key(tile, 4);
        assert!(!seen.contains(&k), "{c:?} / {spec:?} gave a key already seen");
        seen.push(k);
    }
    let w = world(coupled(), horizon());
    assert_ne!(w.horizon_key(tile, 4), w.horizon_key(TileId::new(Face(2), 1, 1, 1).unwrap(), 4));
    // The horizon's keys are not the coupled run's: the two channels never share a record.
    assert_ne!(w.horizon_key(tile, 4), w.coupled_key(tile, 4));
}

fn block(cells: u32, azimuths: u32) -> HorizonBlock {
    let n = (cells * cells * azimuths) as usize;
    HorizonBlock { cells, azimuths, bytes: (0..n).map(|i| (i * 7 % 256) as u8).collect() }
}

// spec: STRM-045
#[test]
fn a_block_round_trips_and_a_bad_payload_is_refused() {
    let b = block(4, 8);
    let bytes = encode_horizon_block(&b).unwrap();
    assert_eq!(bytes[0], HORIZON_CODEC_VERSION);
    assert_eq!(decode_horizon_block(&bytes).unwrap(), b);
    assert!(matches!(decode_horizon_block(&bytes[..5]), Err(BlockCodecError::Short)));
    let mut other = bytes.clone();
    other[0] = 9;
    assert!(matches!(decode_horizon_block(&other), Err(BlockCodecError::HorizonVersion(9))));
    let mut cells = bytes.clone();
    cells[1..5].copy_from_slice(&8u32.to_le_bytes());
    assert!(matches!(decode_horizon_block(&cells), Err(BlockCodecError::RawLength { .. })));
    for bad in [0u32, MAX_AZIMUTHS + 1, u32::MAX] {
        let mut az = bytes.clone();
        az[5..9].copy_from_slice(&bad.to_le_bytes());
        assert!(matches!(decode_horizon_block(&az), Err(BlockCodecError::HorizonAzimuths(_))), "azimuths {bad}");
    }
    let mut odd = bytes.clone();
    odd[1..5].copy_from_slice(&3u32.to_le_bytes());
    assert!(matches!(decode_horizon_block(&odd), Err(BlockCodecError::Cells(3))));
    assert!(decode_horizon_block(&bytes[..bytes.len() - 3]).is_err());
    let mut short = block(4, 8);
    short.bytes.pop();
    assert!(matches!(encode_horizon_block(&short), Err(BlockCodecError::Samples { .. })));
    assert!(matches!(encode_horizon_block(&HorizonBlock { azimuths: 0, ..block(4, 1) }), Err(BlockCodecError::HorizonAzimuths(0))));
}

// spec: STRM-045
#[test]
fn a_missing_or_foreign_block_is_an_error_naming_the_block_and_a_wrong_map_is_refused() {
    let d = dir("missing");
    let w = world(coupled(), horizon());
    store_horizon(&d, &w, &map(), DEFAULT_MAX_PACK_BYTES).unwrap();
    let store = TileStore::open(&d, w.params()).unwrap();
    // The same store read under another march's key: nothing is stored under it.
    let other = world(coupled(), HorizonSpec { params: CoarseParams { range_m: 40_000.0, ..horizon().params }, ..horizon() });
    let err = load_horizon(&store, &other).err().expect("another march has no blocks here");
    assert!(err.to_string().contains("coarse horizon block"), "{err}");
    assert!(w.horizon_records(&map()[1..]).is_err(), "a map of the wrong size");
    let _ = fs::remove_dir_all(&d);
}

// spec: STRM-045
#[test]
fn a_damaged_stored_block_a_wrong_lattice_and_a_default_that_follows_the_body_are_refused_or_derived() {
    let d = dir("damaged");
    let w = world(coupled(), horizon());
    let m = map();
    // An observer lattice that does not divide the run's, or an azimuth count a block cannot hold, writes nothing.
    assert!(world(coupled(), HorizonSpec { m: 3, ..horizon() }).horizon_records(&m).is_err());
    assert!(world(coupled(), HorizonSpec { m: N * 2, ..horizon() }).horizon_records(&m).is_err());
    let many = HorizonSpec { params: CoarseParams { azimuths: MAX_AZIMUTHS + 1, ..horizon().params }, ..horizon() };
    assert!(matches!(world(coupled(), many).horizon_records(&m), Err(BlockCodecError::HorizonAzimuths(_))));
    // A block stored under the right key but cut short is an error naming the block, not a default.
    let mut records = w.horizon_records(&m).unwrap();
    let bad = records[0].payload.len() - 2;
    records[0].payload.truncate(bad);
    let mut writer = planet_cache::pack::PackWriter::open(&d, w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
    for r in &records {
        writer.append(r).unwrap();
    }
    writer.finish().unwrap();
    let store = TileStore::open(&d, w.params()).unwrap();
    let err = load_horizon(&store, &w).err().expect("a damaged block");
    assert!(err.to_string().contains("coarse horizon block"), "{err}");
    // The default march takes the body's radius (an Earth-sized default on a smaller body would key and march with the wrong one).
    let defaulted = GlobalWorld::from_definition(&WorldDefinition::single_planet(R, 3)).unwrap().horizon_spec();
    assert_eq!(defaulted.params.radius_m, R);
    assert_eq!(defaulted.m, 512);
    let _ = fs::remove_dir_all(&d);
}
