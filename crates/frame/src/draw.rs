//! The argument records of an indirect multi-draw (REND-015, review item C): GPU-free and built from a plan's slots, so the
//! renderer only copies them into a buffer (CON-03: the CPU decides what is drawn, the GPU culls nothing).
//!
//! One record per plan node, in the plan's order: `first_instance` is the node's index (the shader reads its node record at
//! that index). A compact tile has no vertex buffer (the vertex stage pulls from the block, ADR 0054), so `base_vertex` is 0. A draw
//! names its block by being in a *run*: consecutive nodes whose slots are in the same block of the pool form one run, and the
//! renderer binds that block once and issues one multi-draw per run. The order of the draws is the plan's, so the picture is the
//! per-draw loop's even where two tiles tie in depth.

use planet_streaming::{PoolKind, SlotRef};

/// Bytes of one `DrawIndexedIndirect` record (five 32-bit words, tightly packed, as the API reads them).
pub const DRAW_RECORD_BYTES: u64 = 20;

/// Bytes of one node record in the storage buffer the shader indexes with the instance index (origin, colour, morph interval
/// nine grid phases and the up vector with the inverse radius of the tile origin, 16-byte aligned).
pub const NODE_RECORD_BYTES: u64 = 48 + 16 * 9 + 16 + GROUND_DETAIL_BYTES;

/// SPIKE G1: the ground detail block of a node record (four lattices: integer cell of the tile origin, fraction and cells per metre).
pub const GROUND_DETAIL_BYTES: u64 = 128;

/// Bytes written per frame for `nodes` planned nodes on the indirect path: node records plus argument records.
pub fn indirect_frame_bytes(nodes: usize) -> u64 {
    nodes as u64 * (NODE_RECORD_BYTES + DRAW_RECORD_BYTES)
}

/// One `DrawIndexedIndirect` argument record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrawRecord {
    pub index_count: u32,
    pub instance_count: u32,
    pub first_index: u32,
    /// First vertex in the bound buffer, in vertices: 0 for a compact tile (no vertex buffer).
    pub base_vertex: i32,
    /// The node's index in the plan.
    pub first_instance: u32,
}

impl DrawRecord {
    pub fn to_bytes(&self) -> [u8; 20] {
        let mut b = [0u8; 20];
        b[0..4].copy_from_slice(&self.index_count.to_le_bytes());
        b[4..8].copy_from_slice(&self.instance_count.to_le_bytes());
        b[8..12].copy_from_slice(&self.first_index.to_le_bytes());
        b[12..16].copy_from_slice(&self.base_vertex.to_le_bytes());
        b[16..20].copy_from_slice(&self.first_instance.to_le_bytes());
        b
    }
}

/// What a run of draws names: one storage-buffer block of the pool (ADR 0033 point 3, ADR 0054: a draw names its block by being in a
/// run of that block; the node's slot reaches the shader through the node's tile reference, the block through the run's bind group).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DrawBuffer(pub u32);

/// Consecutive records that draw from one block: records `first..first + count`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrawRun {
    pub buffer: DrawBuffer,
    pub first: u32,
    pub count: u32,
}

/// The records of a plan and the runs that group them by block.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct DrawList {
    pub records: Vec<DrawRecord>,
    pub runs: Vec<DrawRun>,
}

impl DrawList {
    /// The records as the argument buffer holds them.
    pub fn record_bytes(&self) -> Vec<u8> {
        self.records.iter().flat_map(DrawRecord::to_bytes).collect()
    }

    /// Bytes of the argument records.
    pub fn argument_bytes(&self) -> u64 {
        self.records.len() as u64 * DRAW_RECORD_BYTES
    }
}

/// The block of `slot`.
pub fn buffer_of(slot: SlotRef, block_slots: u32) -> DrawBuffer {
    match slot.pool {
        PoolKind::Main => DrawBuffer(slot.index / block_slots),
    }
}

/// Build the records for nodes whose tiles sit in `slots` (one per node, the plan's order); `index_count` the index count of the
/// tile mesh.
pub fn build_draw_list(slots: impl IntoIterator<Item = SlotRef>, block_slots: u32, index_count: u32) -> DrawList {
    build_draw_list_placed(slots.into_iter().map(|slot| buffer_of(slot, block_slots)), index_count)
}

/// [`build_draw_list`] for nodes whose block is already known: one block per node, in the plan's order.
pub fn build_draw_list_placed(nodes: impl IntoIterator<Item = DrawBuffer>, index_count: u32) -> DrawList {
    let mut list = DrawList::default();
    for (i, buffer) in nodes.into_iter().enumerate() {
        list.records.push(DrawRecord { index_count, instance_count: 1, first_index: 0, base_vertex: 0, first_instance: i as u32 });
        match list.runs.last_mut() {
            Some(run) if run.buffer == buffer => run.count += 1,
            _ => list.runs.push(DrawRun { buffer, first: i as u32, count: 1 }),
        }
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    fn main(index: u32) -> SlotRef {
        SlotRef { pool: PoolKind::Main, index }
    }

    // spec: REND-015
    #[test]
    fn records_follow_the_nodes_and_runs_follow_the_blocks() {
        let slots = [main(0), main(5), main(256), main(257), main(3), main(2)];
        let list = build_draw_list(slots, 256, 6_144);
        assert_eq!(list.records.len(), 6);
        for (i, r) in list.records.iter().enumerate() {
            assert_eq!((r.first_instance, r.instance_count, r.first_index, r.index_count, r.base_vertex), (i as u32, 1, 0, 6_144, 0));
        }
        let runs: Vec<(DrawBuffer, u32, u32)> = list.runs.iter().map(|r| (r.buffer, r.first, r.count)).collect();
        assert_eq!(runs, [(DrawBuffer(0), 0, 2), (DrawBuffer(1), 2, 2), (DrawBuffer(0), 4, 2)]);
        assert_eq!(list.record_bytes().len() as u64, list.argument_bytes());
    }

    // spec: REND-015
    #[test]
    fn a_record_is_five_little_endian_words() {
        let b = DrawRecord { index_count: 1, instance_count: 2, first_index: 3, base_vertex: -4, first_instance: 5 }.to_bytes();
        let words: Vec<u32> = b.chunks(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
        assert_eq!(words, [1, 2, 3, (-4i32) as u32, 5]);
        assert_eq!(b.len() as u64, DRAW_RECORD_BYTES);
    }
}
