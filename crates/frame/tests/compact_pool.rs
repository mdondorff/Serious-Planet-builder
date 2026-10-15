//! The compact pools of the terrain share (ADR 0033 point 3, STRM-006): atlases of 4096 x 4096 `Rgba16Uint` texels, a record per
//! slot, the vertex pool for levels 0-2 and fallback tiles, all inside the tier share and counted without a device.

use planet_frame::pool::{atlas_columns, tier_compact_pool, ATLAS_SIDE, FALLBACK_SLOTS};
use planet_streaming::budget::GIB;
use planet_streaming::Tier;

// spec: STRM-006, REND-016
#[test]
fn the_compact_shares_hold_the_slots_the_adr_counts() {
    let mid = tier_compact_pool(Tier::Mid, 32, 2).unwrap();
    let high = tier_compact_pool(Tier::High, 32, 2).unwrap();
    // 33 x 33 texels per slot: 124 x 124 = 15,376 slots per 4096 x 4096 atlas of 8-byte texels (134,217,728 B).
    assert_eq!(atlas_columns(ATLAS_SIDE, 32), 124);
    assert_eq!((mid.block_slots, mid.block_bytes()), (15_376, 134_217_728));
    assert_eq!((mid.main_blocks, mid.main_slots()), (15, 230_640));
    assert_eq!((high.main_blocks, high.main_slots()), (23, 353_648));
    // The atlases fill the share minus the 0.125 GiB reserve of the fixed items exactly.
    assert_eq!(u64::from(mid.main_blocks) * mid.block_bytes(), 2 * GIB - GIB / 8);
    assert_eq!(u64::from(high.main_blocks) * high.block_bytes(), 3 * GIB - GIB / 8);
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
        assert_eq!((c.block_slots, c.slot_bytes), (63 * 63, 65 * 65 * 8));
        assert!(c.allocated_bytes() <= tier.terrain_share_bytes());
    }
}

// spec: STRM-006
#[test]
fn a_tile_size_whose_slot_records_overflow_the_reserve_is_refused_with_the_numbers() {
    // The record buffer (128 B per slot of the main pool) belongs to the fixed items: at cells 16 the mid share would need 110 MB of
    // records beside the vertex pool and node records, over the 128 MiB reserve. Compact pools start at cells 32 (the production
    // size); smaller tiles use the vertex format or a smaller share.
    for cells in [8u32, 16] {
        let e = tier_compact_pool(Tier::Mid, cells, 2).unwrap_err();
        assert!(e.contains("slot records") && e.contains("reserve"), "{e}");
    }
    // A vertex pool too small for 128 coarse tiles plus the fallback region is refused.
    let tiny = planet_streaming::PoolConfig::fit_share_with(2 * GIB, 8_712, 15_376, 134_217_728, 128, 256, 1_000_000, 25_000, 1 << 20, 1);
    assert!(tiny.is_err(), "{tiny:?}");
}
