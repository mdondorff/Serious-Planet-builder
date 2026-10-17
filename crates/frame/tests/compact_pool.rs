//! The compact pools of the terrain share (ADR 0033 point 3, STRM-006): storage-buffer blocks of at most 128 MiB (ADR 0054), a record per
//! slot, the vertex pool for levels 0-2 and fallback tiles, all inside the tier share and counted without a device.

use planet_frame::pool::{block_slots_in, tier_compact_pool, COMPACT_BLOCK_BYTES, FALLBACK_SLOTS};
use planet_streaming::budget::GIB;
use planet_streaming::Tier;

// spec: STRM-006, REND-016
#[test]
fn the_compact_shares_hold_the_slots_the_adr_counts() {
    let mid = tier_compact_pool(Tier::Mid, 32, 2).unwrap();
    let high = tier_compact_pool(Tier::High, 32, 2).unwrap();
    // 33 x 33 texels of 8 bytes = 8,712 B per slot: a block of 128 MiB holds 15,406 slots (the atlas of ADR 0033 held 15,376).
    assert_eq!(block_slots_in(COMPACT_BLOCK_BYTES, 32), 15_406);
    assert_eq!((mid.block_slots, mid.block_bytes()), (15_406, 134_217_072));
    assert!(mid.block_bytes() <= COMPACT_BLOCK_BYTES, "a block binds as one storage buffer of at most 128 MiB");
    assert_eq!((mid.main_blocks, mid.main_slots()), (15, 231_090));
    assert_eq!((high.main_blocks, high.main_slots()), (23, 354_338));
    // The same block counts as the atlases, and the blocks fit the share minus the 0.125 GiB reserve of the fixed items.
    assert!(u64::from(mid.main_blocks) * mid.block_bytes() <= 2 * GIB - GIB / 8);
    assert!(u64::from(mid.main_blocks + 1) * mid.block_bytes() > 2 * GIB - GIB / 8, "one more block would not fit");
    assert!(u64::from(high.main_blocks) * high.block_bytes() <= 3 * GIB - GIB / 8);
    assert!(u64::from(high.main_blocks + 1) * high.block_bytes() > 3 * GIB - GIB / 8, "one more block would not fit");
    // Slots: 8,712 B of texels, a 128 B record, the vertex pool of 512 vertex-format tiles (the top 256 for fallbacks).
    assert_eq!((mid.slot_bytes, mid.slot_record_bytes), (8_712, 128));
    assert_eq!((mid.vertex_slots, mid.fallback_slots, mid.managed_vertex_slots()), (512, FALLBACK_SLOTS, 256));
    // The model never exceeds the share and the fixed items stay inside their reserve (STRM-006).
    for (c, tier) in [(mid, Tier::Mid), (high, Tier::High)] {
        assert!(c.allocated_bytes() <= tier.terrain_share_bytes(), "{} > {}", c.allocated_bytes(), tier.terrain_share_bytes());
        assert!(c.fixed_bytes() <= GIB / 8, "fixed items {} B over the reserve", c.fixed_bytes());
        assert_eq!(c.allocated_bytes_with(0, false) + c.vertex_slot_bytes * u64::from(c.vertex_slots), c.fixed_bytes());
    }
}

// spec: STRM-006
#[test]
fn cells_64_compact_pools_fit_the_shares_too() {
    for tier in [Tier::Mid, Tier::High] {
        let c = tier_compact_pool(tier, 64, 2).unwrap();
        assert_eq!((c.block_slots, c.slot_bytes), (3_970, 65 * 65 * 8));
        assert!(c.allocated_bytes() <= tier.terrain_share_bytes());
    }
}

// spec: STRM-006
#[test]
fn small_tiles_get_fewer_blocks_so_the_slot_records_fit_their_reserve() {
    // The record buffer (128 B per slot of the main pool) belongs to the fixed items. At cells 16 a block holds 58,052 slots (7.4 MB
    // of records), so the mid share's bytes would give 15 blocks and 110 MB of records, over the 128 MiB reserve with the vertex pool
    // and node records: the pool takes fewer blocks (the share is a ceiling), never breaches the reserve.
    let c16 = tier_compact_pool(Tier::Mid, 16, 2).unwrap();
    assert!(c16.main_blocks < 15 && c16.main_blocks >= 10, "{} blocks", c16.main_blocks);
    for tier in [Tier::Mid, Tier::High] {
        for cells in [8u32, 16, 32, 64] {
            match tier_compact_pool(tier, cells, 2) {
                Ok(c) => {
                    assert!(c.fixed_bytes() <= GIB / 8, "cells {cells}: fixed items {} B over the reserve", c.fixed_bytes());
                    assert!(c.allocated_bytes() <= tier.terrain_share_bytes());
                    assert!(c.slot_record_total_bytes() < 128 << 20, "a record buffer binds as one storage buffer");
                }
                Err(e) => assert!(cells == 8 && e.contains("reserve"), "cells {cells}: {e}"),
            }
        }
    }
    // A vertex pool too small for 128 coarse tiles plus the fallback region is refused.
    let tiny = planet_streaming::PoolConfig::fit_share_with(2 * GIB, 8_712, 15_406, 128, 256, 1_000_000, 25_000, 1 << 20, 1);
    assert!(tiny.is_err(), "{tiny:?}");
}
