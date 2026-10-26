//! The compact pools of the terrain share (ADR 0033 point 3 as amended by ADR 0062 and 0063, STRM-006): storage-buffer blocks of at
//! most 128 MiB (ADR 0054) of slots of 9,488 B since MESH_VERSION 10 (the record, the climate block and the driver block in the slot, GEN-025, ADR 0076/GEN-071; 9,168 B before), the vertex pool for levels
//! 0-2 and fallback tiles, all inside the tier share and counted without a device.

use planet_frame::pool::{block_slots_in, tier_compact_pool, COMPACT_BLOCK_BYTES};
use planet_generators::compact::slot_bytes;
use planet_streaming::budget::GIB;
use planet_streaming::Tier;

// spec: STRM-006, REND-016, GEN-025
#[test]
fn the_compact_shares_hold_the_slots_the_specs_count() {
    let mid = tier_compact_pool(Tier::Mid, 32, 2).unwrap();
    let high = tier_compact_pool(Tier::High, 32, 2).unwrap();
    // MESH_VERSION 10, driver block, ADR 0076/GEN-071: the slot is 9,488 B (776 B of header, climate block and driver block, 33 x 33 texels of 8 B): a block
    // of 128 MiB holds 14,146 slots (14,639 of 9,168 B before).
    assert_eq!(slot_bytes(32), 9_488);
    assert_eq!(block_slots_in(COMPACT_BLOCK_BYTES, 32), 14_146);
    assert_eq!((mid.block_slots, mid.block_bytes()), (14_146, 134_217_248));
    assert!(mid.block_bytes() <= COMPACT_BLOCK_BYTES, "a block binds as one storage buffer of at most 128 MiB");
    assert_eq!((mid.main_blocks, mid.main_slots()), (15, 212_190));
    assert_eq!((high.main_blocks, high.main_slots()), (23, 325_358));
    // -8.2 % against the former 231,090 and 354,338 slots (ADR 0063), -3.4 % against the 219,585 and 336,697 slots of 9,168 B.
    assert!((1.0 - f64::from(mid.main_slots()) / 231_090.0 - 0.0818).abs() < 0.001);
    assert!((1.0 - f64::from(high.main_slots()) / 354_338.0 - 0.0818).abs() < 0.001);
    assert!((1.0 - f64::from(mid.main_slots()) / 219_585.0 - 0.0337).abs() < 0.001);
    // The blocks fit the share minus the 0.125 GiB reserve of the fixed items.
    assert!(u64::from(mid.main_blocks) * mid.block_bytes() <= 2 * GIB - GIB / 8);
    assert!(u64::from(mid.main_blocks + 1) * mid.block_bytes() > 2 * GIB - GIB / 8, "one more block would not fit");
    assert!(u64::from(high.main_blocks) * high.block_bytes() <= 3 * GIB - GIB / 8);
    assert!(u64::from(high.main_blocks + 1) * high.block_bytes() > 3 * GIB - GIB / 8, "one more block would not fit");
    // The record is part of the slot: no per-slot item remains, and no pool of another format (ADR 0062).
    assert_eq!(mid.slot_bytes, 9_488);
    // The model never exceeds the share and the fixed items stay inside their reserve (STRM-006).
    for (c, tier) in [(mid, Tier::Mid), (high, Tier::High)] {
        assert!(c.allocated_bytes() <= tier.terrain_share_bytes(), "{} > {}", c.allocated_bytes(), tier.terrain_share_bytes());
        assert!(c.fixed_bytes() <= GIB / 8, "fixed items {} B over the reserve", c.fixed_bytes());
        assert_eq!(c.allocated_bytes_with(0), c.fixed_bytes());
    }
}

// spec: STRM-006
#[test]
fn cells_64_compact_pools_fit_the_shares_too() {
    for tier in [Tier::Mid, Tier::High] {
        let c = tier_compact_pool(tier, 64, 2).unwrap();
        assert_eq!((c.block_slots, c.slot_bytes), (3_881, 776 + 65 * 65 * 8));
        assert_eq!(c.slot_bytes, 34_576);
        assert!(c.allocated_bytes() <= tier.terrain_share_bytes());
    }
}

// spec: STRM-006
#[test]
fn the_fixed_reserve_holds_no_per_slot_item_so_no_tile_size_overflows_it() {
    // Strides of GEN-025 at cells 16, 32 and 64.
    assert_eq!((slot_bytes(16), slot_bytes(32), slot_bytes(64)), (3_088, 9_488, 34_576));
    for tier in [Tier::Mid, Tier::High] {
        for cells in [8u32, 16, 32, 64] {
            let c = tier_compact_pool(tier, cells, 2).unwrap();
            assert_eq!(c.slot_bytes, slot_bytes(cells));
            assert!(c.fixed_bytes() <= GIB / 8, "cells {cells}: fixed items {} B over the reserve", c.fixed_bytes());
            assert!(c.allocated_bytes() <= tier.terrain_share_bytes());
            // The fixed items do not grow with the main pool: the same configuration with one block more has the same fixed bytes.
            let more = planet_streaming::PoolConfig { main_blocks: c.main_blocks + 1, ..c };
            assert_eq!(more.fixed_bytes(), c.fixed_bytes(), "cells {cells}: a per-slot item is charged to the reserve");
        }
    }
    // The share is a ceiling: small tiles take as many blocks as the bytes allow (15 at the mid share at cells 16 too).
    assert_eq!(tier_compact_pool(Tier::Mid, 16, 2).unwrap().main_blocks, 15);
    // Fixed items over their reserve are refused.
    let tiny = planet_streaming::PoolConfig::fit_share(2 * GIB, 9_488, 14_146, 25_000, 2 * GIB, 1);
    assert!(tiny.is_err(), "{tiny:?}");
}
