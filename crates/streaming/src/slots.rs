//! Pooled fixed-size slots (STRM-006, STRM-007, ADR 0033 point 3): the accounting and placement of tiles in a few large
//! GPU pools, without a device (CON-03). A pool hands out slot indices from a free list; a slot that is given back is
//! reusable only after the frames that may still read it have retired (frames in flight, counted in frames). The renderer
//! executes the placement (`slot -> block, offset`); it decides nothing.

use planet_core::TileId;
use std::collections::{BTreeSet, VecDeque};

/// Tiles of level 0 to this level live in the small vertex pool (ADR 0033 point 3: at most 126 tiles).
pub const VERTEX_POOL_MAX_LEVEL: u8 = 2;

/// Which pool a slot belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PoolKind {
    /// Levels 3 and finer: the large pool (atlases or vertex blocks, whatever the tile format of the day).
    Main,
    /// Levels 0-2 and the CON-13 fallback tiles: a small pool of the vertex format, fixed capacity.
    Vertex,
}

/// A slot: pool and index within it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SlotRef {
    pub pool: PoolKind,
    pub index: u32,
}

/// One pool: a fixed capacity of slots, a free list, and slots waiting out their frames in flight.
///
/// Invariant (STRM-006): `in_use + retiring + free == capacity` at every moment. Slots are handed out lowest index first, so
/// a pool that is mostly empty touches few blocks and the allocated bytes stay small.
#[derive(Clone, Debug)]
pub struct SlotPool {
    capacity: u32,
    retire_frames: u64,
    /// Slots below this index have been handed out at least once; the rest are fresh (never used, never in `returned`).
    fresh: u32,
    /// Slots that came back and are reusable now.
    returned: BTreeSet<u32>,
    /// `(first frame at which the slot is reusable, slot)`, in release order (frames never go down).
    retiring: VecDeque<(u64, u32)>,
    in_use: u32,
    /// `used[i]` is true while slot `i` is handed out (double release is refused).
    used: Vec<bool>,
}

impl SlotPool {
    /// `retire_frames`: how many frames after its release a slot becomes reusable (the frames in flight; 0 for a renderer
    /// that finishes a frame before the next begins).
    pub fn new(capacity: u32, retire_frames: u64) -> Self {
        Self { capacity, retire_frames, fresh: 0, returned: BTreeSet::new(), retiring: VecDeque::new(), in_use: 0, used: Vec::new() }
    }

    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    pub fn in_use(&self) -> u32 {
        self.in_use
    }

    pub fn retiring(&self) -> u32 {
        self.retiring.len() as u32
    }

    /// Slots that are neither in use nor retiring (reusable at once, or never used).
    pub fn free(&self) -> u32 {
        self.capacity - self.in_use - self.retiring()
    }

    /// One more than the highest slot index ever handed out: the part of the pool that was ever touched.
    pub fn high_water(&self) -> u32 {
        self.fresh
    }

    /// Free slots at `frame`: [`free`](SlotPool::free) plus the retiring slots that are reusable by then.
    pub fn free_at(&self, frame: u64) -> u32 {
        self.free() + self.retiring.iter().take_while(|(at, _)| *at <= frame).count() as u32
    }

    fn advance(&mut self, frame: u64) {
        while self.retiring.front().is_some_and(|(at, _)| *at <= frame) {
            let (_, slot) = self.retiring.pop_front().expect("front");
            self.returned.insert(slot);
        }
    }

    /// The lowest free slot, or `None` when the pool is full (every slot in use or still retiring at `frame`).
    pub fn acquire(&mut self, frame: u64) -> Option<u32> {
        self.advance(frame);
        let slot = if let Some(&s) = self.returned.iter().next() {
            self.returned.remove(&s);
            s
        } else if self.fresh < self.capacity {
            self.fresh += 1;
            self.fresh - 1
        } else {
            return None;
        };
        if self.used.len() <= slot as usize {
            self.used.resize(slot as usize + 1, false);
        }
        self.used[slot as usize] = true;
        self.in_use += 1;
        Some(slot)
    }

    /// Give `slot` back at `frame`; it can be acquired again from `frame + retire_frames`. False when it was not in use.
    pub fn release(&mut self, slot: u32, frame: u64) -> bool {
        if !self.used.get(slot as usize).copied().unwrap_or(false) {
            return false;
        }
        self.used[slot as usize] = false;
        self.in_use -= 1;
        self.retiring.push_back((frame + self.retire_frames, slot));
        true
    }

    /// The accounting invariant: `in_use + retiring + free == capacity`, and no slot is in two states.
    pub fn check(&self) -> Result<(), String> {
        let total = self.in_use + self.retiring() + self.returned.len() as u32 + (self.capacity - self.fresh);
        if total != self.capacity {
            return Err(format!(
                "slot accounting: in use {} + retiring {} + returned {} + fresh {} = {total}, capacity {}",
                self.in_use,
                self.retiring(),
                self.returned.len(),
                self.capacity - self.fresh,
                self.capacity
            ));
        }
        if self.used.iter().filter(|u| **u).count() as u32 != self.in_use {
            return Err("slot accounting: the in-use marks do not match the count".into());
        }
        Ok(())
    }
}

/// The two pools of a streamer. With `vertex_slots == 0` there is one pool and every tile uses it.
#[derive(Clone, Debug)]
pub struct SlotPools {
    pub main: SlotPool,
    pub vertex: Option<SlotPool>,
}

impl SlotPools {
    pub fn new(main_slots: u32, vertex_slots: u32, retire_frames: u64) -> Self {
        Self {
            main: SlotPool::new(main_slots, retire_frames),
            vertex: (vertex_slots > 0).then(|| SlotPool::new(vertex_slots, retire_frames)),
        }
    }

    /// The pool a tile of `tile`'s level lives in.
    pub fn kind_of(&self, tile: TileId) -> PoolKind {
        if self.vertex.is_some() && tile.level() <= VERTEX_POOL_MAX_LEVEL {
            PoolKind::Vertex
        } else {
            PoolKind::Main
        }
    }

    pub fn pool(&self, kind: PoolKind) -> &SlotPool {
        match kind {
            PoolKind::Main => &self.main,
            PoolKind::Vertex => self.vertex.as_ref().unwrap_or(&self.main),
        }
    }

    pub fn pool_mut(&mut self, kind: PoolKind) -> &mut SlotPool {
        match kind {
            PoolKind::Main => &mut self.main,
            PoolKind::Vertex => self.vertex.as_mut().unwrap_or(&mut self.main),
        }
    }

    pub fn acquire(&mut self, kind: PoolKind, frame: u64) -> Option<SlotRef> {
        self.pool_mut(kind).acquire(frame).map(|index| SlotRef { pool: kind, index })
    }

    pub fn release(&mut self, slot: SlotRef, frame: u64) -> bool {
        self.pool_mut(slot.pool).release(slot.index, frame)
    }

    pub fn check(&self) -> Result<(), String> {
        self.main.check()?;
        self.vertex.as_ref().map_or(Ok(()), SlotPool::check)
    }
}
