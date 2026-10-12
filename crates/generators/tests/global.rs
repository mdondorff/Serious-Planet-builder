//! GEN-013: the global-coarse stage (continents, elevation, climate).

use std::collections::HashMap;

use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId, Vec3};
use planet_generators::dump::NET_BACKGROUND;
use planet_generators::global::{
    bake, bake_block, bake_blocks, equirect_product, grid_hash, product, sample, BakeError, Channel, GlobalGrid, GlobalSample,
    GLOBAL_VERSION, MAX_BAKE_RES, MAX_BLOCK_CELLS, SEA_LEVEL_C, SEA_LEVEL_F32,
};

const SEED: u64 = 7;

fn grid(res: u32, threads: usize) -> GlobalGrid {
    bake(SEED, &TangentWarp, res, threads).unwrap()
}

fn bits(s: &GlobalSample) -> [u32; 4] {
    [s.continent.to_bits(), s.elevation_m.to_bits(), s.temperature_c.to_bits(), s.precipitation_mm.to_bits()]
}

// spec: GEN-013
#[test]
fn the_bake_is_bit_identical_for_every_thread_count_and_equals_the_pointwise_field() {
    let one = grid(16, 1);
    for threads in [2, 3, 7, 64] {
        assert_eq!(one, grid(16, threads), "{threads} threads");
    }
    assert_eq!(grid_hash(&one), grid_hash(&grid(16, 5)));
    // Every baked sample is the pointwise field at the direction of the same lattice vertex of a level-0 tile.
    for f in 0..6u8 {
        let root = TileId::new(Face(f), 0, 0, 0).unwrap();
        for (k, l) in (0..=16).flat_map(|l| (0..=16).map(move |k| (k, l))) {
            let d = root.sample_direction(&TangentWarp, 16, k, l);
            assert_eq!(bits(&one.at(Face(f), k, l)), bits(&sample(SEED, d)), "face {f} vertex ({k}, {l})");
        }
    }
}

// spec: GEN-013
#[test]
fn samples_on_a_shared_face_edge_are_one_value() {
    // Border agreement across faces (GEN-004 for the global bake): vertices that are the same direction have the same bits.
    let g = grid(8, 2);
    let mut by_direction: HashMap<[u64; 3], [u32; 4]> = HashMap::new();
    let mut shared = 0;
    for f in 0..6u8 {
        let root = TileId::new(Face(f), 0, 0, 0).unwrap();
        for l in 0..=8 {
            for k in 0..=8 {
                let d: Vec3 = root.sample_direction(&TangentWarp, 8, k, l);
                let key = [d.x.to_bits(), d.y.to_bits(), d.z.to_bits()];
                let v = bits(&g.at(Face(f), k, l));
                if let Some(prev) = by_direction.insert(key, v) {
                    assert_eq!(prev, v, "face {f} vertex ({k}, {l}) disagrees with the face that shares it");
                    shared += 1;
                }
            }
        }
    }
    assert_eq!(shared, 100, "6 x 81 listed vertices minus 386 distinct directions: every edge and corner vertex is shared");
}

// spec: GEN-013
#[test]
fn the_field_has_continents_oceans_mountains_and_latitude_bands() {
    let g = grid(64, 4);
    let (mut land, mut total) = (0usize, 0usize);
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    let (mut t_equator, mut n_equator, mut t_pole, mut n_pole) = (0.0f64, 0, 0.0f64, 0);
    let (mut p_itcz, mut n_itcz, mut p_desert, mut n_desert) = (0.0f64, 0, 0.0f64, 0);
    for f in 0..6u8 {
        let root = TileId::new(Face(f), 0, 0, 0).unwrap();
        for l in 0..=64 {
            for k in 0..=64 {
                let s = g.at(Face(f), k, l);
                let y = root.sample_direction(&TangentWarp, 64, k, l).y;
                total += 1;
                land += usize::from(s.is_land());
                lo = lo.min(s.elevation_m);
                hi = hi.max(s.elevation_m);
                assert!(s.elevation_m.is_finite() && s.temperature_c.is_finite() && s.precipitation_mm >= 0.0);
                // Land is above sea level in elevation and ocean below (the shelf starts at the coast).
                if s.is_land() {
                    assert!(s.elevation_m > 0.0, "land below sea level at {y}: {s:?}");
                } else {
                    assert!(s.elevation_m <= 0.0, "ocean above sea level: {s:?}");
                }
                if y.abs() < 0.1 && !s.is_land() {
                    t_equator += f64::from(s.temperature_c);
                    n_equator += 1;
                    p_itcz += f64::from(s.precipitation_mm);
                    n_itcz += 1;
                }
                if y.abs() > 0.95 && !s.is_land() {
                    t_pole += f64::from(s.temperature_c);
                    n_pole += 1;
                }
                if (0.40..0.50).contains(&y.abs()) && !s.is_land() {
                    p_desert += f64::from(s.precipitation_mm);
                    n_desert += 1;
                }
            }
        }
    }
    let land_fraction = land as f64 / total as f64;
    assert!((0.15..0.5).contains(&land_fraction), "land fraction {land_fraction}: continents and oceans both exist");
    assert!(lo < -2000.0 && hi > 2000.0, "elevation range {lo} .. {hi}: deep ocean and mountains exist");
    assert!(n_equator > 20 && n_pole > 20 && n_itcz > 20 && n_desert > 20, "too few samples to judge the climate");
    assert!(t_equator / f64::from(n_equator) > t_pole / f64::from(n_pole) + 30.0, "the equator is much warmer than the poles");
    assert!(p_itcz / f64::from(n_itcz) > p_desert / f64::from(n_desert), "the rain belt at the equator is wetter than the subtropics");
}

// spec: GEN-013
#[test]
fn land_is_above_and_ocean_at_or_below_sea_level_for_any_direction() {
    // Land is decided from the stored (f32) continentalness, the same number that shapes the elevation, so the two never
    // disagree, also within rounding of the coast; the field is continuous across it (no coastal cliff).
    let (mut land, mut coastal) = (0, 0);
    for i in 0..60_000u64 {
        let h = |k: u64| planet_core::hash::unit_f64(planet_core::hash::hash_u64s(SEED, &[i, k])) * 2.0 - 1.0;
        let v = Vec3::new(h(0), h(1), h(2));
        if v.length() < 1e-3 {
            continue;
        }
        let s = sample(SEED, v.normalized());
        if s.is_land() {
            land += 1;
            assert!(s.elevation_m > 0.0, "land at or below sea level: {s:?}");
        } else {
            assert!(s.elevation_m <= 0.0, "ocean above sea level: {s:?}");
        }
        if (f64::from(s.continent) - SEA_LEVEL_C).abs() < 0.01 {
            coastal += 1;
            assert!(s.elevation_m.abs() < 60.0, "a coastal sample far from sea level (a cliff): {s:?}");
        }
    }
    assert!(land > 10_000 && coastal > 100, "{land} land and {coastal} coastal samples");
}

// spec: GEN-013
#[test]
fn a_cold_peak_and_a_warm_lowland_follow_the_lapse_rate() {
    // Temperature falls with height over land: among land samples at similar latitude the high ones are colder.
    let g = grid(128, 4);
    let root = TileId::new(Face(4), 0, 0, 0).unwrap();
    let (mut high, mut low) = ((0.0f64, 0u32), (0.0f64, 0u32));
    for l in 0..=128 {
        for k in 0..=128 {
            let s = g.at(Face(4), k, l);
            let y = root.sample_direction(&TangentWarp, 128, k, l).y;
            if !s.is_land() || !(0.3..0.5).contains(&y) {
                continue;
            }
            if s.elevation_m > 2500.0 {
                high = (high.0 + f64::from(s.temperature_c), high.1 + 1);
            } else if s.elevation_m < 500.0 {
                low = (low.0 + f64::from(s.temperature_c), low.1 + 1);
            }
        }
    }
    assert!(high.1 > 3 && low.1 > 3, "need both high and low land in the band: {} / {}", high.1, low.1);
    assert!(high.0 / f64::from(high.1) < low.0 / f64::from(low.1) - 8.0, "high ground is colder than lowland");
}

// spec: GEN-013
#[test]
fn another_seed_is_another_world_and_the_same_seed_the_same() {
    assert_ne!(grid_hash(&grid(8, 1)), grid_hash(&bake(8, &TangentWarp, 8, 1).unwrap()));
    assert_eq!(grid_hash(&grid(8, 1)), grid_hash(&grid(8, 1)));
}

// spec: GEN-013
#[test]
fn a_bake_refuses_resolutions_it_cannot_hold() {
    for res in [0, 3, 100, MAX_BAKE_RES * 2] {
        let e = bake(SEED, &TangentWarp, res, 1).unwrap_err();
        assert_eq!(e, BakeError::BadResolution(res));
        assert!(e.to_string().contains(&MAX_BAKE_RES.to_string()), "the message names the limit: {e}");
    }
    assert!(bake(SEED, &TangentWarp, 1, 1).is_ok());
}

// spec: GEN-013
#[test]
fn the_data_products_show_the_world_in_every_channel() {
    let g = grid(32, 2);
    for ch in Channel::ALL {
        let img = product(&g, ch);
        assert_eq!((img.width, img.height), (128, 96), "{}", ch.name());
        // The four cells outside the cross are background, the six faces are not, and the picture is not flat.
        assert_eq!(img.pixel(32, 0), NET_BACKGROUND, "{}", ch.name());
        let mut colours = std::collections::BTreeSet::new();
        for y in 32..64 {
            for x in 0..128 {
                let p = img.pixel(x, y);
                assert_ne!(p, NET_BACKGROUND, "{} pixel ({x}, {y}) is on a face", ch.name());
                colours.insert(p);
            }
        }
        assert!(colours.len() > 20, "{} is a flat picture ({} colours)", ch.name(), colours.len());
    }
    let map = equirect_product(SEED, 128, Channel::Elevation);
    assert_eq!((map.width, map.height), (128, 64));
    assert_ne!(map.pixel(0, 0), map.pixel(64, 32));

    // Written for review (not goldens): target/review/global-coarse/<channel>-net.png and <channel>-map.png.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/review/global-coarse");
    let big = bake(SEED, &TangentWarp, 64, 4).unwrap();
    for ch in Channel::ALL {
        product(&big, ch).write_png(&dir.join(format!("{}-net.png", ch.name()))).unwrap();
        equirect_product(SEED, 256, ch).write_png(&dir.join(format!("{}-map.png", ch.name()))).unwrap();
    }
}

// spec: GEN-013
#[test]
fn known_answers_pin_the_field_on_every_platform() {
    // Pointwise: bit patterns of the four channels at fixed directions. A change here is a change of the field and needs
    // GLOBAL_VERSION and these answers bumped together (ADR 0006).
    let dirs = [
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, 0.0, -1.0),
        Vec3::new(0.36, -0.48, 0.8),
        Vec3::new(-0.6, 0.64, 0.48),
    ];
    let got: Vec<[u32; 4]> = dirs.iter().map(|d| bits(&sample(SEED, *d))).collect();
    assert_eq!(format!("{got:08x?}"), KNOWN_POINTS, "pointwise known answers");
    assert_eq!(format!("{:016x}", grid_hash(&grid(16, 3))), KNOWN_GRID_HASH, "the hash of the 16-cell bake");
    // The sea level is part of the field's contract.
    assert_eq!(SEA_LEVEL_C, f64::from(0.1f32));
    assert_eq!(GLOBAL_VERSION, RECORDED_VERSION, "the known answers belong to this version");
}

// spec: GEN-013
#[test]
fn the_sea_level_is_exactly_an_f32_so_readers_in_f32_and_f64_agree() {
    // ADR 0026 point 3: a stored 0.1f32 is sea for an f32 reader and for the bake alike.
    assert_eq!(SEA_LEVEL_F32.to_bits(), 0x3dcc_cccd);
    assert_eq!(f64::from(SEA_LEVEL_F32), SEA_LEVEL_C);
    let at = |c: f32| GlobalSample { continent: c, elevation_m: 0.0, temperature_c: 0.0, precipitation_mm: 0.0 };
    assert!(!at(f32::from_bits(0x3dcc_cccd)).is_land(), "exactly the sea level is sea");
    assert!(at(f32::from_bits(0x3dcc_ccce)).is_land(), "the next f32 up is land");
}

// spec: GEN-013
#[test]
fn border_directions_are_unique_and_a_block_equals_the_level_zero_lattice_bit_for_bit() {
    // At a production-like resolution the lattice has exactly 12 (res - 1) + 8 distinct directions on the cube's edges (each
    // shared by two or three faces) and 6 (res - 1)^2 + 12 (res - 1) + 8 in all: no two lattice vertices are one direction
    // unless they are the same vertex of the cube.
    let res = 256u32;
    let mut all: std::collections::HashSet<[u64; 3]> = std::collections::HashSet::new();
    let mut border: std::collections::HashSet<[u64; 3]> = std::collections::HashSet::new();
    for f in 0..6u8 {
        let root = TileId::new(Face(f), 0, 0, 0).unwrap();
        for l in 0..=res {
            for k in 0..=res {
                let d = root.sample_direction(&TangentWarp, res, k, l);
                let key = [d.x.to_bits(), d.y.to_bits(), d.z.to_bits()];
                all.insert(key);
                if k == 0 || k == res || l == 0 || l == res {
                    border.insert(key);
                }
            }
        }
    }
    let r = res as usize;
    assert_eq!(border.len(), 12 * (r - 1) + 8);
    assert_eq!(all.len(), 6 * (r - 1) * (r - 1) + 12 * (r - 1) + 8);

    // A block of a level-L tile is the corresponding window of the level-0 lattice of `cells << L` cells, bit for bit.
    let (cells, level) = (8u32, 2u8);
    let whole = grid(cells << level, 3);
    for f in 0..6u8 {
        for ty in 0..4u32 {
            for tx in 0..4u32 {
                let tile = TileId::new(Face(f), level, tx, ty).unwrap();
                let block = bake_block(SEED, &TangentWarp, tile, cells);
                for l in 0..=cells {
                    for k in 0..=cells {
                        let want = whole.at(Face(f), tx * cells + k, ty * cells + l);
                        assert_eq!(
                            bits(&block[(l * (cells + 1) + k) as usize]),
                            bits(&want),
                            "face {f} tile ({tx}, {ty}) sample ({k}, {l})"
                        );
                    }
                }
            }
        }
    }
}

// spec: GEN-013
#[test]
fn a_tiled_bake_is_thread_and_block_size_independent_in_order_and_holds_at_most_one_batch() {
    let collect = |level: u8, cells: u32, threads: usize| {
        let mut out: Vec<(TileId, Vec<GlobalSample>)> = Vec::new();
        let stats = bake_blocks(SEED, &TangentWarp, level, cells, threads, &mut |t, b| out.push((t, b))).unwrap();
        (stats, out)
    };
    let (stats1, one) = collect(2, 4, 1);
    let (stats5, five) = collect(2, 4, 5);
    assert_eq!(one, five, "the thread count changes nothing, not even the order");
    assert_eq!(stats1.blocks, 6 * 16);
    assert_eq!((stats1.peak_resident_blocks, stats5.peak_resident_blocks), (1, 5), "at most `threads` blocks are alive at once, counted");
    // The documented order, built explicitly: face, then row, then column; each tile exactly once.
    let expected: Vec<TileId> =
        (0..6u8).flat_map(|f| (0..4u32).flat_map(move |y| (0..4u32).map(move |x| TileId::new(Face(f), 2, x, y).unwrap()))).collect();
    assert_eq!(one.iter().map(|b| b.0).collect::<Vec<_>>(), expected);
    // The same grid in other blocks: the samples at the same lattice vertices are equal (compare a face-0 window).
    let (_, coarse_blocks) = collect(1, 8, 2);
    let a = &coarse_blocks[0].1; // tile (0, 0) at level 1, 8 cells: lattice vertices (0..=8, 0..=8) of a 16-cell face
    let b = &one[0].1; // tile (0, 0) at level 2, 4 cells: vertices (0..=4, 0..=4)
    for l in 0..=4usize {
        for k in 0..=4usize {
            assert_eq!(bits(&a[l * 9 + k]), bits(&b[l * 5 + k]), "vertex ({k}, {l})");
        }
    }
    // Refusals.
    for cells in [0, 3, MAX_BLOCK_CELLS * 2] {
        assert_eq!(bake_blocks(SEED, &TangentWarp, 0, cells, 1, &mut |_, _| {}).unwrap_err(), BakeError::BadBlock(cells));
    }
    assert!(
        matches!(bake_blocks(SEED, &TangentWarp, 8, 1024, 1, &mut |_, _| {}), Err(BakeError::BadResolution(_))),
        "a 262144-cell grid is too fine"
    );
    // Levels beyond the limit are refused before anything is allocated or shifted (also absurd ones, in debug builds).
    for level in [13u8, 16, 64, 255] {
        assert_eq!(bake_blocks(SEED, &TangentWarp, level, 1, 1, &mut |_, _| {}).unwrap_err(), BakeError::BadLevel(level));
    }
}

// spec: GEN-013
#[test]
fn the_peak_of_live_blocks_is_counted_between_creation_and_delivery() {
    // What the bake itself keeps alive is at most one batch, whatever `threads` is.
    for threads in [1usize, 4, 16] {
        let mut delivered = 0usize;
        let stats = bake_blocks(SEED, &TangentWarp, 1, 2, threads, &mut |_, _| delivered += 1).unwrap();
        assert_eq!(delivered, 24);
        assert!(stats.peak_resident_blocks >= 1 && stats.peak_resident_blocks <= threads.min(24), "{threads} threads: {stats:?}");
    }
}

/// The version the known answers below were recorded against (ADR 0006): changing the output without a bump fails here.
const RECORDED_VERSION: u32 = 2;
const KNOWN_POINTS: &str = "[[3f3ef54b, 44cb75f7, 41a18f38, 45032416], [be83b213, c4c37fdb, c1cfa1d5, 442c4f54], [bed97260, c52f51c0, 41eb8eac, 451e5422], [3e25822e, 42c5afbf, 4192eb50, 430d7a6e], [beff7496, c54fe3c7, 40aa0957, 446de42a]]";
const KNOWN_GRID_HASH: &str = "4e56a28cf452a416";
