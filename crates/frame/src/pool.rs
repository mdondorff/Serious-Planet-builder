//! The terrain pools of the compact format (STRM-006, ADR 0033 points 1 and 3, ADR 0054, ADR 0062): the slot size and the share of
//! a tier, as the numbers `planet_streaming::PoolConfig` counts in. GPU-free; the renderer allocates exactly this.
//!
//! The slot is the compact tile of GEN-025 at the given `cells`: 9,168 B at cells 32 (ADR 0063; 2,768 B at 16, 34,256 B at 64). The
//! share is a byte ceiling, so the slot count follows from the slot: 219,585 slots at the mid share, 336,697 at the high share at
//! cells 32 (231,090 and 354,338 before the slot of 9,168 B).

use planet_generators::compact::slot_bytes;
use planet_generators::mesh::grid_indices;
use planet_streaming::{PoolConfig, Tier};

/// Largest number of planned nodes the per-node record buffer holds (a plan beyond it is refused, not allocated for).
pub const MAX_PLAN_NODES: u64 = 32_768;

/// The per-node bytes the share reserves for node records (the renderer needs 2 x (192 B record + 20 B argument record) plus
/// 2 x 96 B of globals; 512 leaves room).
pub const NODE_RECORD_STRIDE_MAX: u64 = 512;

/// Bytes of one block of the main pool: the largest storage-buffer binding every adapter offers (ADR 0054 point 1,
/// `max_storage_buffer_binding_size` of wgpu's default limits, which the adapter gate requires by name, VIEW-001).
pub const COMPACT_BLOCK_BYTES: u64 = 128 << 20;

/// Slots of a block of at most `block_bytes` for tiles of `cells`: `block_bytes / slot_bytes` (GEN-025; 14,639 at cells 32 in 128 MiB).
pub fn block_slots_in(block_bytes: u64, cells: u32) -> u32 {
    u32::try_from(block_bytes / slot_bytes(cells)).unwrap_or(u32::MAX)
}

/// The compact pools of a terrain share for blocks of `block_slots` slots (ADR 0054): a block is one storage buffer of
/// `block_slots * slot_bytes` bytes (the slots of GEN-025, header, climate block and texels, slot after slot; no record buffer).
pub fn compact_format_pool_with(share_bytes: u64, cells: u32, block_slots: u32, frames_in_flight: u64) -> Result<PoolConfig, String> {
    if block_slots == 0 {
        return Err(format!("a block of 0 slots holds no tile of {cells} cells"));
    }
    let index_bytes = grid_indices(cells).len() as u64 * 4;
    PoolConfig::fit_share(
        share_bytes,
        slot_bytes(cells),
        block_slots,
        index_bytes,
        MAX_PLAN_NODES * NODE_RECORD_STRIDE_MAX,
        frames_in_flight,
    )
}

/// [`compact_format_pool_with`] for blocks of [`COMPACT_BLOCK_BYTES`].
pub fn compact_format_pool(share_bytes: u64, cells: u32, frames_in_flight: u64) -> Result<PoolConfig, String> {
    compact_format_pool_with(share_bytes, cells, block_slots_in(COMPACT_BLOCK_BYTES, cells), frames_in_flight)
}

/// The compact pools of a hardware tier's terrain share: 15 blocks = 219,585 slots at the mid share, 23 = 336,697 at the high
/// share at cells 32 (ADR 0033 point 3 as amended by ADR 0062 and 0063).
pub fn tier_compact_pool(tier: Tier, cells: u32, frames_in_flight: u64) -> Result<PoolConfig, String> {
    compact_format_pool(tier.terrain_share_bytes(), cells, frames_in_flight)
}
