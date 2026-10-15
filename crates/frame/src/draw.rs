//! The argument records of an indirect multi-draw (REND-015, review item C): GPU-free and built from a plan's slots, so the
//! renderer only copies them into a buffer (CON-03: the CPU decides what is drawn, the GPU culls nothing).
//!
//! One record per plan node, in the plan's order: `first_instance` is the node's index (the shader reads its node record at
//! that index), `base_vertex` is the first vertex of the node's slot inside the vertex buffer the draw names. A draw names
//! its buffer by being in a *run*: consecutive nodes whose slots are in the same buffer (one block of the main pool, or the
//! small vertex pool) form one run, and the renderer binds that buffer once and issues one multi-draw per run. The order of
//! the draws is the plan's, so the picture is the per-draw loop's even where two tiles tie in depth.

use planet_streaming::{PoolKind, SlotRef};

/// Bytes of one `DrawIndexedIndirect` record (five 32-bit words, tightly packed, as the API reads them).
pub const DRAW_RECORD_BYTES: u64 = 20;

/// Bytes of one node record in the storage buffer the shader indexes with the instance index (origin, colour, morph interval
/// and nine grid phases, 16-byte aligned).
pub const NODE_RECORD_BYTES: u64 = 48 + 16 * 9;

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
    /// First vertex of the slot in the bound buffer, in vertices.
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

/// The vertex buffer a run of draws names: the small vertex pool, or one block of the main pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DrawBuffer {
    VertexPool,
    Block(u32),
}

/// Consecutive records that draw from one buffer: records `first..first + count`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrawRun {
    pub buffer: DrawBuffer,
    pub first: u32,
    pub count: u32,
}

/// The records of a plan and the runs that group them by buffer.
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

/// The buffer and the slot index within it of `slot`.
pub fn buffer_of(slot: SlotRef, block_slots: u32) -> (DrawBuffer, u32) {
    match slot.pool {
        PoolKind::Vertex => (DrawBuffer::VertexPool, slot.index),
        PoolKind::Main => (DrawBuffer::Block(slot.index / block_slots), slot.index % block_slots),
    }
}

/// Build the records for nodes whose tiles sit in `slots` (one per node, the plan's order). `vertices_per_slot` is the vertex
/// count of a slot (the same for every slot of both pools); `index_count` the index count of the tile mesh.
pub fn build_draw_list(slots: impl IntoIterator<Item = SlotRef>, block_slots: u32, vertices_per_slot: u32, index_count: u32) -> DrawList {
    let mut list = DrawList::default();
    for (i, slot) in slots.into_iter().enumerate() {
        let (buffer, within) = buffer_of(slot, block_slots);
        let base_vertex =
            i32::try_from(u64::from(within) * u64::from(vertices_per_slot)).expect("a block's vertices fit a 32-bit base vertex");
        list.records.push(DrawRecord { index_count, instance_count: 1, first_index: 0, base_vertex, first_instance: i as u32 });
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
    fn records_follow_the_nodes_and_runs_follow_the_buffers() {
        let slots = [main(0), main(5), main(256), main(257), SlotRef { pool: PoolKind::Vertex, index: 3 }, main(2)];
        let list = build_draw_list(slots, 256, 1_221, 6_144);
        assert_eq!(list.records.len(), 6);
        for (i, r) in list.records.iter().enumerate() {
            assert_eq!((r.first_instance, r.instance_count, r.first_index, r.index_count), (i as u32, 1, 0, 6_144));
        }
        let bases: Vec<i32> = list.records.iter().map(|r| r.base_vertex).collect();
        assert_eq!(bases, [0, 5 * 1_221, 0, 1_221, 3 * 1_221, 2 * 1_221]);
        let runs: Vec<(DrawBuffer, u32, u32)> = list.runs.iter().map(|r| (r.buffer, r.first, r.count)).collect();
        assert_eq!(
            runs,
            [(DrawBuffer::Block(0), 0, 2), (DrawBuffer::Block(1), 2, 2), (DrawBuffer::VertexPool, 4, 1), (DrawBuffer::Block(0), 5, 1)]
        );
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
