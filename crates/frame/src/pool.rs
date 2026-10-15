//! The terrain pools of the vertex format (STRM-006, ADR 0033 points 1 and 3): the slot size and the share of a tier, as the
//! numbers `planet_streaming::PoolConfig` counts in. GPU-free; the renderer allocates exactly this.
//!
//! The slot is the vertex-format tile at the given `cells`: `(cells + 1)^2` grid vertices plus `4 (cells + 1)` skirt vertices,
//! 44 bytes each (53,724 B at cells 32). ADR 0033's slot counts (230,640 at the mid share) are for the compact tile; the
//! vertex format has fewer, and the share is a byte ceiling, so the slot count follows from the format.

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
    vertex_count(cells) as u64 * MeshVertex::STRIDE
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
