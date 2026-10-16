//! The terrain pools of the vertex format (STRM-006, ADR 0033 points 1 and 3): the slot size and the share of a tier, as the
//! numbers `planet_streaming::PoolConfig` counts in. GPU-free; the renderer allocates exactly this.
//!
//! The slot is the vertex-format tile at the given `cells`: `(cells + 1)^2` grid vertices plus `4 (cells + 1)` skirt vertices,
//! 44 bytes each (53,724 B at cells 32). ADR 0033's slot counts (230,640 at the mid share) are for the compact tile; the
//! vertex format has fewer, and the share is a byte ceiling, so the slot count follows from the format.

use planet_generators::compact::{slot_texel_bytes, COMPACT_RECORD_BYTES, TEXEL_BYTES};
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

/// Side of one atlas texture in texels (ADR 0033 point 3: 4096 x 4096 atlases).
pub const ATLAS_SIDE: u32 = 4096;

/// Vertex-pool slots kept for the CON-13 fallback tiles of the compact format (the top of the small vertex pool; ADR 0033 point 3
/// fixes the pool at 512 slots, levels 0-2 need at most 126 of them).
pub const FALLBACK_SLOTS: u32 = 256;

/// Slots along one side of an atlas of `side` texels for tiles of `cells`: `side / (cells + 1)`.
pub fn atlas_columns(side: u32, cells: u32) -> u32 {
    side / (cells + 1)
}

/// The compact pools of a terrain share for atlases of `side` x `side` texels (ADR 0033 point 3): a block is one atlas texture of
/// `Rgba16Uint` (8 B per texel), `columns^2` slots of `(cells + 1)^2` texels, a 128 B record per slot, and the vertex pool
/// (53,724 B slots at cells 32) for levels 0-2 and the fallback tiles. `slot_bytes` is the texel bytes of one slot.
pub fn compact_format_pool_with(share_bytes: u64, cells: u32, side: u32, frames_in_flight: u64) -> Result<PoolConfig, String> {
    let columns = atlas_columns(side, cells);
    if columns == 0 {
        return Err(format!("an atlas of {side} texels holds no tile of {cells} cells"));
    }
    let vertex_slot = vertex_slot_bytes(cells);
    let index_bytes = grid_indices(cells).len() as u64 * 4;
    PoolConfig::fit_share_with(
        share_bytes,
        slot_texel_bytes(cells),
        columns * columns,
        u64::from(side) * u64::from(side) * TEXEL_BYTES,
        COMPACT_RECORD_BYTES,
        FALLBACK_SLOTS,
        vertex_slot,
        index_bytes,
        MAX_PLAN_NODES * NODE_RECORD_STRIDE_MAX,
        frames_in_flight,
    )
}

/// [`compact_format_pool_with`] for the 4096-texel atlases.
pub fn compact_format_pool(share_bytes: u64, cells: u32, frames_in_flight: u64) -> Result<PoolConfig, String> {
    compact_format_pool_with(share_bytes, cells, ATLAS_SIDE, frames_in_flight)
}

/// The compact pools of a hardware tier's terrain share: 15 atlases = 230,640 slots at the mid share, 23 = 353,648 at the high
/// share at cells 32 (ADR 0033 point 3).
pub fn tier_compact_pool(tier: Tier, cells: u32, frames_in_flight: u64) -> Result<PoolConfig, String> {
    compact_format_pool(tier.terrain_share_bytes(), cells, frames_in_flight)
}
