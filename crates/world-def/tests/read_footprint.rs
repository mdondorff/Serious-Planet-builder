//! Tier A: the reads of a tile's footprint without its result (WDEF-015, SPIKE-29): `GenContext::read_footprint` records what
//! `query_footprint` records, for tiles coarser and finer than the cell level, with the same hash.

use planet_core::{TileId, Vec3};
use planet_world_def::{BodyId, GenContext, ItemSpec, WorldDefinition};

const R: f64 = 6_371_000.0;

fn unit(lat_deg: f64, lon_deg: f64) -> Vec3 {
    let (lat, lon) = (lat_deg.to_radians(), lon_deg.to_radians());
    Vec3::new(libm::cos(lat) * libm::cos(lon), libm::cos(lat) * libm::sin(lon), libm::sin(lat))
}

// spec: WDEF-015
#[test]
fn the_memoised_hash_of_an_empty_subtree_is_the_chain_of_hashes_for_every_pair_of_levels() {
    use planet_world_def::summary::{cell_hash128, empty_hash128, node_hash128};
    for cell_level in 0..=12u8 {
        for level in 0..=cell_level {
            let mut h = cell_hash128(&[]);
            for l in (level..cell_level).rev() {
                h = node_hash128(l, &[h; 4]);
            }
            // Twice: the first call fills the memo, the second reads it.
            assert_eq!(empty_hash128(level, cell_level), h, "level {level} of {cell_level}");
            assert_eq!(empty_hash128(level, cell_level), h, "level {level} of {cell_level}, from the memo");
        }
    }
}

// spec: WDEF-015
#[test]
fn the_reads_without_the_result_are_the_reads_of_the_query_for_every_level_and_halo() {
    let mut def = WorldDefinition::single_planet(R, 3);
    // A cluster and some items elsewhere, in two layers.
    for id in 1..=120u128 {
        let (la, lo) = (20.0 + (id % 17) as f64 * 0.01, 30.0 + (id % 23) as f64 * 0.01);
        let layer = if id % 5 == 0 { "other" } else { "stamps" };
        let spec = ItemSpec {
            id,
            layer: layer.into(),
            priority: (id % 3) as i32,
            dir: unit(la, lo),
            content_hash: id * 11,
            reach_m: 50.0 + (id % 40) as f64 * 100.0,
            max_delta_m: 2.0,
            stamp: None,
        };
        def.put_item_spec(BodyId::PLANET, spec).unwrap();
    }
    let mut compared = 0;
    for level in [0u8, 3, 5, 8, 12, 20] {
        for (la, lo) in [(20.1, 30.1), (-40.0, 150.0), (35.264, 45.0), (89.99, 10.0)] {
            let tile = TileId::from_direction(&planet_core::TangentWarp, unit(la, lo), level);
            for halo in [0.0, 0.25, 1.0] {
                for layer in ["stamps", "other", "missing"] {
                    let (a, b) = (GenContext::new(&def), GenContext::new(&def));
                    a.query_footprint(BodyId::PLANET, layer, tile, halo).unwrap();
                    b.read_footprint(BodyId::PLANET, layer, tile, halo).unwrap();
                    assert_eq!(a.read_records(), b.read_records(), "level {level} halo {halo} {layer}");
                    assert_eq!(a.read_set_hash(), b.read_set_hash(), "level {level} halo {halo} {layer}: the hash");
                    compared += 1;
                }
            }
        }
    }
    assert_eq!(compared, 6 * 4 * 3 * 3);
    // The refusals are the same.
    let tile = TileId::from_direction(&planet_core::TangentWarp, unit(20.0, 30.0), 8);
    assert!(GenContext::new(&def).read_footprint(BodyId::PLANET, "stamps", tile, 1.5).is_err());
    assert!(GenContext::new(&def).read_footprint(BodyId::PLANET, "stamps", tile, f64::NAN).is_err());
    assert!(GenContext::new(&def).read_footprint(BodyId(9), "stamps", tile, 0.25).is_err());
}
