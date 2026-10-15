//! Terrain's VRAM share, counted in slots (STRM-006, ADR 0033 points 1 and 3): a pure model of what the terrain pools
//! allocate, so the cap is asserted at tier A on any machine (CON-20). The renderer allocates exactly what
//! [`PoolConfig`] says; a test compares the two.

use crate::slots::SlotPools;

pub const GIB: u64 = 1 << 30;

/// Reserved for the fixed items inside the share (ADR 0033 point 1): the vertex pool of levels 0-2, index buffers and the
/// per-node record buffer. The pools of the main format get the rest.
pub const FIXED_RESERVE_BYTES: u64 = GIB / 8;

/// Slots of the small vertex pool (levels 0-2 are at most 126 tiles; the rest takes the CON-13 fallback tiles; ADR 0033
/// point 3). A fallback beyond it is refused with the tile id rather than breaching the share.
pub const VERTEX_POOL_SLOTS: u32 = 512;

/// The hardware tiers of CON-21.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    /// 8 GiB of VRAM.
    Mid,
    /// 14 GiB of VRAM.
    High,
}

impl Tier {
    /// The tier's VRAM cap (CON-21).
    pub fn vram_cap_bytes(self) -> u64 {
        match self {
            Tier::Mid => 8 * GIB,
            Tier::High => 14 * GIB,
        }
    }

    /// Terrain's share of the cap (ADR 0033 point 1; configurable, this is the default): a ceiling, not a target.
    pub fn terrain_share_bytes(self) -> u64 {
        match self {
            Tier::Mid => 2 * GIB,
            Tier::High => 3 * GIB,
        }
    }
}

/// What the terrain pools allocate, as numbers. Every field is a count or a size, nothing is measured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoolConfig {
    /// Bytes of one slot of the main pool (the tile format of the day).
    pub slot_bytes: u64,
    /// Slots per block (an atlas or a vertex buffer): blocks are allocated on demand.
    pub block_slots: u32,
    /// Most blocks the main pool may allocate: the share's ceiling.
    pub main_blocks: u32,
    /// Bytes of one slot of the small vertex pool.
    pub vertex_slot_bytes: u64,
    /// Slots of the small vertex pool (0: no small pool, one pool for every level).
    pub vertex_slots: u32,
    /// Index buffers.
    pub index_bytes: u64,
    /// The per-node record buffer at its largest.
    pub node_record_bytes: u64,
    /// A freed slot is reusable this many frames after its release (frames in flight).
    pub frames_in_flight: u64,
    /// Bytes one block really allocates when that is not `slot_bytes * block_slots` (a compact atlas is a whole texture whose
    /// slots do not fill it); 0 means `slot_bytes * block_slots` (the vertex-format blocks).
    pub block_alloc_bytes: u64,
    /// Bytes of the per-slot record of the compact format (ADR 0033: 128), held for every slot of the main pool in one
    /// buffer allocated with the renderer; 0 for the vertex format.
    pub slot_record_bytes: u64,
    /// Slots of the small vertex pool the renderer keeps for the CON-13 fallback tiles of the compact format (ADR 0033 points 3
    /// and 5): the top `fallback_slots` of `vertex_slots`; the residency manager sees only the rest. 0 for the vertex format.
    pub fallback_slots: u32,
}

impl PoolConfig {
    /// A configuration whose main pool takes everything the `share_bytes` leave after [`FIXED_RESERVE_BYTES`], in whole
    /// blocks. `Err` when the fixed items do not fit their reserve or the share holds no block.
    pub fn fit_share(
        share_bytes: u64,
        slot_bytes: u64,
        block_slots: u32,
        vertex_slot_bytes: u64,
        index_bytes: u64,
        node_record_bytes: u64,
        frames_in_flight: u64,
    ) -> Result<PoolConfig, String> {
        Self::fit_share_with(
            share_bytes,
            slot_bytes,
            block_slots,
            0,
            0,
            0,
            vertex_slot_bytes,
            index_bytes,
            node_record_bytes,
            frames_in_flight,
        )
    }

    /// [`fit_share`](PoolConfig::fit_share) for a main pool whose block is a texture of `block_alloc_bytes` (0: `slot_bytes *
    /// block_slots`) with a record of `slot_record_bytes` per slot, and `fallback_slots` of the vertex pool kept for fallback
    /// tiles. The record buffer (`main_slots * slot_record_bytes`) counts with the fixed items.
    #[allow(clippy::too_many_arguments)]
    pub fn fit_share_with(
        share_bytes: u64,
        slot_bytes: u64,
        block_slots: u32,
        block_alloc_bytes: u64,
        slot_record_bytes: u64,
        fallback_slots: u32,
        vertex_slot_bytes: u64,
        index_bytes: u64,
        node_record_bytes: u64,
        frames_in_flight: u64,
    ) -> Result<PoolConfig, String> {
        let mut c = PoolConfig {
            slot_bytes,
            block_slots,
            main_blocks: 0,
            vertex_slot_bytes,
            vertex_slots: VERTEX_POOL_SLOTS,
            index_bytes,
            node_record_bytes,
            frames_in_flight,
            block_alloc_bytes,
            slot_record_bytes,
            fallback_slots,
        };
        // A big slot makes the vertex pool outgrow its reserve: halve it (never under 128 slots, the 126 tiles of levels 0-2)
        // and give up the small pool (one pool for every level) when even that does not fit.
        while c.fixed_bytes() > FIXED_RESERVE_BYTES && c.vertex_slots > 0 {
            c.vertex_slots = if c.vertex_slots / 2 >= 128 { c.vertex_slots / 2 } else { 0 };
        }
        if c.fixed_bytes() > FIXED_RESERVE_BYTES {
            return Err(format!("the fixed items take {} B, more than their reserve of {FIXED_RESERVE_BYTES} B", c.fixed_bytes()));
        }
        if c.fallback_slots > 0 && c.vertex_slots < c.fallback_slots + 128 {
            return Err(format!(
                "a vertex pool of {} slots cannot hold 128 coarse tiles and {} fallback tiles",
                c.vertex_slots, c.fallback_slots
            ));
        }
        let block_bytes = c.block_bytes();
        if block_bytes == 0 || share_bytes < FIXED_RESERVE_BYTES {
            return Err(format!("a share of {share_bytes} B leaves nothing after the {FIXED_RESERVE_BYTES} B reserve"));
        }
        c.main_blocks = u32::try_from((share_bytes - FIXED_RESERVE_BYTES) / block_bytes).map_err(|e| e.to_string())?;
        if c.main_blocks == 0 {
            return Err(format!("a share of {share_bytes} B holds no block of {block_bytes} B"));
        }
        // The slot records scale with the main pool: they belong to the fixed items and must fit their reserve with the rest.
        if c.fixed_bytes() > FIXED_RESERVE_BYTES {
            return Err(format!(
                "the fixed items with {} slot records take {} B, more than their reserve of {FIXED_RESERVE_BYTES} B",
                c.main_slots(),
                c.fixed_bytes()
            ));
        }
        Ok(c)
    }

    /// Bytes one block of the main pool allocates.
    pub fn block_bytes(&self) -> u64 {
        if self.block_alloc_bytes > 0 {
            self.block_alloc_bytes
        } else {
            self.slot_bytes * u64::from(self.block_slots)
        }
    }

    /// Bytes of the slot-record buffer: one record per slot of the main pool at its ceiling.
    pub fn slot_record_total_bytes(&self) -> u64 {
        self.slot_record_bytes * u64::from(self.main_slots())
    }

    /// Slots of the small vertex pool the residency manager places tiles in (levels 0-2); the top `fallback_slots` are the
    /// renderer's own, for fallback tiles.
    pub fn managed_vertex_slots(&self) -> u32 {
        self.vertex_slots.saturating_sub(self.fallback_slots)
    }

    /// Slots of the main pool at its ceiling: what the residency manager counts its capacity in.
    pub fn main_slots(&self) -> u32 {
        self.main_blocks * self.block_slots
    }

    /// The fixed items: the small vertex pool, index buffers, node records and the slot records.
    pub fn fixed_bytes(&self) -> u64 {
        self.vertex_slot_bytes * u64::from(self.vertex_slots) + self.index_bytes + self.node_record_bytes + self.slot_record_total_bytes()
    }

    /// Bytes allocated with `main_blocks_live` blocks of the main pool in use and the vertex pool allocated or not
    /// (`vertex_pool_live`): what the renderer holds at that moment.
    pub fn allocated_bytes_with(&self, main_blocks_live: u32, vertex_pool_live: bool) -> u64 {
        let vertex = if vertex_pool_live { self.vertex_slot_bytes * u64::from(self.vertex_slots) } else { 0 };
        u64::from(main_blocks_live) * self.block_bytes()
            + vertex
            + self.index_bytes
            + self.node_record_bytes
            + self.slot_record_total_bytes()
    }

    /// The most the pools can ever allocate: all blocks, the vertex pool, index buffers and node records. This is the
    /// contract of STRM-006: it never exceeds the terrain share.
    pub fn allocated_bytes(&self) -> u64 {
        self.allocated_bytes_with(self.main_blocks, true)
    }

    /// Blocks of the main pool needed to hold slots up to index `high_water` (one past the highest slot ever used).
    pub fn blocks_for(&self, high_water: u32) -> u32 {
        high_water.div_ceil(self.block_slots.max(1))
    }

    /// The streamer's pools for this configuration.
    pub fn slot_pools(&self) -> SlotPools {
        SlotPools::new(self.main_slots(), self.managed_vertex_slots(), self.frames_in_flight)
    }
}

/// `tiles` as a fraction of `slots` (the headroom pins of ADR 0033 point 2).
pub fn slot_fraction(tiles: usize, slots: u64) -> f64 {
    tiles as f64 / slots as f64
}
