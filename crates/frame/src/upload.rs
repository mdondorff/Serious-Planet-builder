//! The per-frame upload budget (STRM-006 upload cap, STRM-017; ADR 0033 point 9, ADR 0032, CON-21: uploads <= 2 ms a frame).
//! GPU-free (CON-03): the managed loop asks this module which of the finished tiles may be written to the pool this frame.
//!
//! **What the cap counts.** ADR 0033 point 9 applies the cap to *tile data* (the slot's texels and record), "about 118
//! compact slots" for 1 MiB; so [`UploadBudget::tile_bytes_per_frame`] is a count of tile bytes, each tile's size being what
//! the sink says it uploads ([`crate::managed::TileSink::upload_bytes`], a function of the tile's format). The per-frame node
//! records (the indirect path's `indirect_frame_bytes`, a CPU write into a ring) are not tiles and cannot be deferred: the
//! frame needs them. They count against CON-21's 2 ms (the owner's decision, ADR 0033's status line) as a separate
//! accounting line, reported every frame as `node_bytes`; the cap value follows from the 2 ms in a perf window, so
//! [`UploadBudget::node_records_in_cap`] lets the cap cover both (the tile allowance shrinks by the node bytes of the
//! previous frame's plan) without a code change. **The value is ADR 0033 point 9** (accepted 2026-10-16 after M4 perf window 5: the dive
//! reaches 0.973 MiB per frame at an upload p99 of 0.79 ms).
//!
//! **Order.** Finished tiles wait in class order: the draw set D and its ancestors (and the six roots) first, ancestors
//! before children, then in plan order; then the residency set R (and its ancestors); then the prefetch set; then
//! anything else (the manager's own ancestor requests of other classes). A tile that does not fit the frame's remaining
//! allowance stops the frame: the tiles behind it, even smaller ones, wait too, so the order never changes (a tile is deferred,
//! never dropped, never overtaken). **At least one tile is written every frame** (a tile larger than the cap would otherwise
//! starve), so the cap is exceeded only by that one tile. The cover plan (STRM-016) draws a deepest resident ancestor
//! meanwhile, so the view is never blank.

use planet_core::TileId;

/// Starting value of the cap: 1 MiB of tile data per frame (ADR 0033 point 9, accepted; about 114 compact slots of 9,168 B,
/// 17 vertex-format slots of 58,608 B at cells 32 (the 44-byte vertex and its climate)).
pub const DEFAULT_TILE_BYTES_PER_FRAME: u64 = 1 << 20;

/// The per-frame upload budget. Change it here (or build the loop with another) after the perf window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UploadBudget {
    /// Bytes of tile data written to the pool in one frame; `u64::MAX` is no cap.
    pub tile_bytes_per_frame: u64,
    /// Let the node records of the frame count inside the cap: the tile allowance is the cap minus the node bytes of the
    /// previous drawn plan (never below the one-tile rule). Off by default: the 1 MiB is tile data (ADR 0033 point 9) and the
    /// node records are an own line against the 2 ms.
    pub node_records_in_cap: bool,
}

impl Default for UploadBudget {
    fn default() -> Self {
        Self { tile_bytes_per_frame: DEFAULT_TILE_BYTES_PER_FRAME, node_records_in_cap: false }
    }
}

impl UploadBudget {
    /// No cap (a harness that measures the uncapped fill).
    pub fn unlimited() -> Self {
        Self { tile_bytes_per_frame: u64::MAX, node_records_in_cap: false }
    }

    /// Tile bytes this frame may write, given the node-record bytes of the previous frame.
    pub fn allowance(&self, node_bytes: u64) -> u64 {
        if self.node_records_in_cap {
            self.tile_bytes_per_frame.saturating_sub(node_bytes)
        } else {
            self.tile_bytes_per_frame
        }
    }
}

/// The request class of a finished tile (ADR 0031 point 2), in upload order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UploadClass {
    /// The draw set, its ancestors and the roots.
    Draw,
    /// The residency set and its ancestors.
    Residency,
    Prefetch,
    Other,
}

/// A finished tile waiting for its turn.
#[derive(Clone, Copy, Debug)]
pub struct Waiting {
    pub tile: TileId,
    pub class: UploadClass,
    /// Within the draw class: the plan order (lower first); other classes use the wants' priority or 0.
    pub priority: f64,
    pub bytes: u64,
}

/// Sort into upload order: class, then level (ancestors before children), then priority, then tile (a total order, so the
/// result does not depend on the order the tiles were found in).
pub fn sort_waiting(waiting: &mut [Waiting]) {
    waiting.sort_by(|a, b| {
        a.class.cmp(&b.class).then(a.tile.level().cmp(&b.tile.level())).then(a.priority.total_cmp(&b.priority)).then(a.tile.cmp(&b.tile))
    });
}

/// How many tiles of the sorted `waiting` list are written this frame: the longest prefix whose bytes fit `allowance`, and at
/// least one when anything waits (the one-tile rule).
pub fn upload_count(waiting: &[Waiting], allowance: u64) -> usize {
    let mut used = 0u64;
    let mut n = 0;
    for w in waiting {
        if n > 0 && used.saturating_add(w.bytes) > allowance {
            break;
        }
        used = used.saturating_add(w.bytes);
        n += 1;
    }
    n
}
