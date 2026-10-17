//! The terrain pools of the vertex format (STRM-006, ADR 0033 points 1 and 3): the slot size and the share of a tier, as the
//! numbers `planet_streaming::PoolConfig` counts in. GPU-free; the renderer allocates exactly this.
//!
//! The slot is the vertex-format tile at the given `cells`: `(cells + 1)^2` grid vertices plus `4 (cells + 1)` skirt vertices,
//! 44 bytes each (53,724 B at cells 32). ADR 0033's slot counts (231,090 at the mid share) are for the compact tile; the
//! vertex format has fewer, and the share is a byte ceiling, so the slot count follows from the format.

use planet_generators::compact::{slot_texel_bytes, COMPACT_RECORD_BYTES};
use planet_generators::mesh::{grid_indices, vertex_count, MeshVertex};
use planet_streaming::{PoolConfig, Tier};

/// Largest number of planned nodes the per-node record buffer holds (a plan beyond it is refused, not allocated for).
pub const MAX_PLAN_NODES: u64 = 32_768;

/// The per-node bytes the share reserves for node records (the renderer needs 2 x (192 B record + 20 B argument record) plus
/// 2 x 96 B of globals; 512 leaves room).
pub const NODE_RECORD_STRIDE_MAX: u64 = 512;

/// Largest bytes of one block (vertex buffer) of the main pool: well under wgpu's default buffer size limit of 256 MiB.
pub const MAX_BLOCK_BYTES: u64 = 64 << 20;

/// Slots per block at most: blocks are allocated on demand, so this is the granule of the allocation.
pub const BLOCK_SLOTS: u32 = 256;

/// Bytes of one vertex-format tile of `cells`.
pub fn vertex_slot_bytes(cells: u32) -> u64 {
    vertex_count(cells) as u64 * MeshVertex::GPU_STRIDE
}

/// The pools of a terrain share (in bytes) for the vertex format at `cells`.
pub fn vertex_format_pool(share_bytes: u64, cells: u32, frames_in_flight: u64) -> Result<PoolConfig, String> {
    let slot = vertex_slot_bytes(cells);
    let block_slots = (MAX_BLOCK_BYTES / slot).clamp(1, u64::from(BLOCK_SLOTS)) as u32;
    let index_bytes = grid_indices(cells).len() as u64 * 4;
    PoolConfig::fit_share(share_bytes, slot, block_slots, slot, index_bytes, MAX_PLAN_NODES * NODE_RECORD_STRIDE_MAX, frames_in_flight)
}

/// The pools of a hardware tier's terrain share (2 GiB mid, 3 GiB high; ADR 0033 point 1).
pub fn tier_pool(tier: Tier, cells: u32, frames_in_flight: u64) -> Result<PoolConfig, String> {
    vertex_format_pool(tier.terrain_share_bytes(), cells, frames_in_flight)
}

/// Bytes of one block of the compact main pool: the largest storage-buffer binding every adapter offers (ADR 0054 point 1,
/// `max_storage_buffer_binding_size` of wgpu's default limits, which the adapter gate requires by name, VIEW-001).
pub const COMPACT_BLOCK_BYTES: u64 = 128 << 20;

/// Vertex-pool slots kept for the CON-13 fallback tiles of the compact format (the top of the small vertex pool; ADR 0033 point 3
/// fixes the pool at 512 slots, levels 0-2 need at most 126 of them).
pub const FALLBACK_SLOTS: u32 = 256;

/// Slots of a block of at most `block_bytes` for tiles of `cells`: `block_bytes / slot_texel_bytes` (15,406 at cells 32 in 128 MiB).
pub fn block_slots_in(block_bytes: u64, cells: u32) -> u32 {
    u32::try_from(block_bytes / slot_texel_bytes(cells)).unwrap_or(u32::MAX)
}

/// The compact pools of a terrain share for blocks of `block_slots` slots (ADR 0054): a block is one storage buffer of
/// `block_slots * slot_texel_bytes` bytes (the texels of 8 B, slot after slot), a 128 B record per slot in one buffer, and the vertex
/// pool (53,724 B slots at cells 32) for levels 0-2 and the fallback tiles.
pub fn compact_format_pool_with(share_bytes: u64, cells: u32, block_slots: u32, frames_in_flight: u64) -> Result<PoolConfig, String> {
    if block_slots == 0 {
        return Err(format!("a block of 0 slots holds no tile of {cells} cells"));
    }
    let vertex_slot = vertex_slot_bytes(cells);
    let index_bytes = grid_indices(cells).len() as u64 * 4;
    PoolConfig::fit_share_with(
        share_bytes,
        slot_texel_bytes(cells),
        block_slots,
        COMPACT_RECORD_BYTES,
        FALLBACK_SLOTS,
        vertex_slot,
        index_bytes,
        MAX_PLAN_NODES * NODE_RECORD_STRIDE_MAX,
        frames_in_flight,
    )
}

/// [`compact_format_pool_with`] for blocks of [`COMPACT_BLOCK_BYTES`].
pub fn compact_format_pool(share_bytes: u64, cells: u32, frames_in_flight: u64) -> Result<PoolConfig, String> {
    compact_format_pool_with(share_bytes, cells, block_slots_in(COMPACT_BLOCK_BYTES, cells), frames_in_flight)
}

/// The compact pools of a hardware tier's terrain share: 15 blocks = 231,090 slots at the mid share, 23 = 354,338 at the high
/// share at cells 32 (ADR 0033 point 3).
pub fn tier_compact_pool(tier: Tier, cells: u32, frames_in_flight: u64) -> Result<PoolConfig, String> {
    compact_format_pool(tier.terrain_share_bytes(), cells, frames_in_flight)
}
