//! The channel graph as a stored kind (item 15 PR 7, STRM-084; architect D51): one record of [`KIND_CHANNEL_GRAPH`] (20) per L tile, holding the pieces of the segments inside the tile's block of H1 cells
//! and one H1 cell beyond it (clipped to the face), filed under the tile's quadtree id, in the key family and commit of the L tile (D17).
//!
//! A piece is a run of consecutive vertices of one segment inside the block: `(segment id, part)`, the first vertex as face H1 coordinates `(i, j)` and then, per further vertex, the planar step to it
//! ([`planet_generators::bake::NEIGHBOURS`]), and per vertex the `Q_w` code and the bed code of [`planet_generators::bake::channel_graph`]. Runs are cut where a segment leaves the block's margin or the face; a segment that
//! crosses a face edge is two runs on two faces (a channel point of a neighbouring face is not mapped into this face's plane: the known limit of this version).
//!
//! The payload is `codec (u8) | graph version (u32) | face (u8) | block column and row (u32 each) | H1 cells a block side (u32) | unit (u8: 1 discharge, 2 area) | pieces (u32) | key (16) |` the brotli stream of the pieces, each
//! `segment id (u64) | part (u32) | vertices (u32) | i, j (i32 each) | vertices - 1 steps (u8) | vertices Q_w codes (u16) | vertices bed codes (i16)`, all little-endian. A decoder reserves no more than the bytes it was given allow.

use std::collections::BTreeMap;
use std::io::{Read, Write};

use planet_core::{Face, TileId};
use planet_generators::bake::channel_graph::ChannelGraph;
use planet_generators::bake::hydro::QUnit;
use planet_generators::bake::{Grid, NEIGHBOURS};

use crate::basin_table::BasinSpec;
use crate::global_store::GlobalWorld;
use crate::key::{cache_key, CacheKey, Kind, PARAM_CHANNEL_GRAPH, PARAM_RESOLUTION};
use crate::pack::{Record, TileStore};

/// The kind byte of a graph block.
pub const KIND_CHANNEL_GRAPH: u8 = Kind::ChannelGraph as u8;
/// Version of the layout and of the meaning of the key. History: 1 = the layout of this file; 2 = the margin is a field of the spec and of the block (it was 1 H1 cell fixed): the H1 cells that hold the K7 halo, plus one (GEN-169).
pub const GRAPH_VERSION: u32 = 2;
const CODEC_VERSION: u8 = 1;
const IMPLEMENTATION_ID: &str = "channel-graph-block";
const BROTLI_QUALITY: u32 = 5;
const BROTLI_WINDOW_BITS: u32 = 22;
const BROTLI_BUFFER: usize = 64 * 1024;
const HEADER_LEN: usize = 1 + 4 + 1 + 4 + 4 + 4 + 1 + 4 + 16 + 4;
/// The fixed part of a piece.
const PIECE_FIXED: usize = 8 + 4 + 4 + 4 + 4 + 4 + 4 + 4;
/// The most bytes a decoded block may hold (a damaged length is an error, not a request to reserve memory).
const MAX_BODY: usize = 1 << 30;

/// Why a stored graph block could not be used. Every message names the record and what was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphStoreError(pub String);

impl std::fmt::Display for GraphStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What a graph is made of besides the bake it is beside: the identity of the hydrology (as the basin table's spec: its parameters, the L tile grid, the base and the other inputs) and the H1 cells a face edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GraphSpec {
    pub basin: BasinSpec,
    pub h1_cells: u32,
    /// How far beyond its block (H1 cells) a block's pieces reach: at least `required_margin` of the halo of the K7 tiles that read the graph.
    pub margin_h1: u32,
}

impl GraphSpec {
    /// H1 cells to a block side (the L tile's core in H1 cells).
    pub fn h1_per_block(&self) -> u32 {
        self.h1_cells / self.basin.tiles_per_edge
    }

    /// Local cells to an H1 cell.
    pub fn l_per_h1(&self) -> u32 {
        self.basin.tile_cells / self.h1_per_block()
    }

    /// The margin a window of `halo` local cells needs: the H1 cells that hold the halo, plus one for the next vertex of a line that leaves the window (GEN-169: the carry reads a halo upstream).
    pub fn required_margin(&self, halo: u32) -> u32 {
        halo.div_ceil(self.l_per_h1()) + 1
    }

    fn bytes(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(48);
        b.extend(GRAPH_VERSION.to_le_bytes());
        b.extend(planet_generators::bake::BAKE_KERNEL_VERSION.to_le_bytes());
        let h = &self.basin.hydro;
        b.extend(h.initiation_m2.to_bits().to_le_bytes());
        b.extend(h.pit_min_m.to_bits().to_le_bytes());
        b.extend(h.min_cells.to_le_bytes());
        b.extend(h.min_depth_m.to_bits().to_le_bytes());
        b.extend(h.body.to_le_bytes());
        b.extend(self.basin.tile_cells.to_le_bytes());
        b.extend(self.basin.tiles_per_edge.to_le_bytes());
        b.extend(self.h1_cells.to_le_bytes());
        b.extend(self.margin_h1.to_le_bytes());
        b.extend(self.basin.base_id.to_le_bytes());
        b.extend(self.basin.inputs_id.to_le_bytes());
        b
    }
}

/// The quadtree id a block is filed under (the L tile's).
pub fn block_tile(spec: &GraphSpec, face: u8, bi: u32, bj: u32) -> Result<TileId, GraphStoreError> {
    let n = spec.basin.tiles_per_edge;
    if !n.is_power_of_two() {
        return Err(GraphStoreError(format!("{n} blocks a face edge is not a power of two: a block cannot be filed under a quadtree id")));
    }
    TileId::new(Face(face), n.trailing_zeros() as u8, bi, bj)
        .ok_or_else(|| GraphStoreError(format!("block ({bi}, {bj}) of face {face} is outside the {n} blocks a face edge")))
}

/// The key of the block `(bi, bj)` of `face`.
pub fn graph_key(world: &GlobalWorld, res: u32, spec: &GraphSpec, face: u8, bi: u32, bj: u32) -> Result<CacheKey, GraphStoreError> {
    let tile = block_tile(spec, face, bi, bj)?;
    let mut inputs = world.base_key_inputs(Kind::ChannelGraph, tile, res, IMPLEMENTATION_ID);
    inputs.output_params.insert(PARAM_RESOLUTION, spec.basin.tile_cells.to_le_bytes().to_vec());
    inputs.output_params.insert(PARAM_CHANNEL_GRAPH, spec.bytes());
    Ok(cache_key(&inputs))
}

/// The block `(face, bi, bj)` a record of this kind belongs to, read from its payload header.
pub fn block_of(spec: &GraphSpec, r: &Record) -> Result<(u8, u32, u32), String> {
    let p = &r.payload;
    if r.kind != KIND_CHANNEL_GRAPH || p.len() < HEADER_LEN {
        return Err(format!("a record of kind {} with {} bytes is not a channel graph block", r.kind, p.len()));
    }
    let (face, bi, bj) = (p[5], u32_at(p, 6), u32_at(p, 10));
    block_tile(spec, face, bi, bj).map_err(|e| e.0)?;
    Ok((face, bi, bj))
}

/// A run of consecutive vertices of one segment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Piece {
    pub segment: u64,
    pub part: u32,
    /// The index of the piece's first vertex in its segment's chain, and the vertex ranges of the segment under a lake's water that meet the piece (D54: the lake entry is the vertex before a range, the spill cell the vertex at its end; when the end is the segment's last vertex and it is under water too, the next segment starts with a cut at vertex 0), in the segment's
    /// vertex numbering. The drawing derives the lake-cut pieces from these.
    pub v0: u32,
    /// The vertices of the whole segment and ALL its lake cuts (not only those that meet this piece): with them the producer's pieces of the segment (the runs between the cuts, numbered from 0) and their ends are known from
    /// any one of its pieces, whatever block it is in.
    pub seg_vertices: u32,
    pub cuts: Vec<(u32, u32)>,
    /// The first vertex as H1 coordinates on the face.
    pub i0: i32,
    pub j0: i32,
    /// The planar step into each vertex after the first (an index of [`NEIGHBOURS`]).
    pub steps: Vec<u8>,
    pub q: Vec<u16>,
    pub bed: Vec<i16>,
}

impl Piece {
    pub fn vertices(&self) -> usize {
        self.q.len()
    }

    /// The H1 coordinates of every vertex.
    pub fn coords(&self) -> Vec<(i64, i64)> {
        let mut out = Vec::with_capacity(self.q.len());
        let (mut i, mut j) = (i64::from(self.i0), i64::from(self.j0));
        out.push((i, j));
        for &s in &self.steps {
            let (di, dj) = NEIGHBOURS[usize::from(s)];
            i += di;
            j += dj;
            out.push((i, j));
        }
        out
    }
}

/// A block as stored.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphBlock {
    pub face: u8,
    pub bi: u32,
    pub bj: u32,
    pub q_unit: QUnit,
    pub pieces: Vec<Piece>,
}

fn unit_byte(u: QUnit) -> u8 {
    match u {
        QUnit::Discharge => 1,
        QUnit::Area => 2,
    }
}

/// The records of the blocks of `graph`: every block that holds a piece, sorted by `(face, bi, bj)`. `ids` gives the segment id of each segment of the graph (the hydrology's `Segment::id`), `cell_xy` the
/// `(face, i, j)` of a cell of `grid` on the face's H1 grid. Panics on a graph whose steps are not planar neighbours within a face (a graph of a grid the caller mapped wrongly).
pub fn graph_block_records(
    world: &GlobalWorld,
    res: u32,
    spec: &GraphSpec,
    graph: &ChannelGraph,
    ids: &[u64],
    grid: &Grid,
    cell_xy: &dyn Fn(u32) -> (u8, i64, i64),
) -> Vec<Record> {
    assert_eq!(ids.len(), graph.segments(), "one id per segment");
    let h = i64::from(spec.h1_per_block());
    let margin = i64::from(spec.margin_h1);
    assert!(h >= margin && margin >= 1, "a block is at least {margin} H1 cells a side, as its margin (the margin reaches the neighbouring block only from there), it is {h}");
    let n = i64::from(spec.h1_cells);
    let mut blocks: BTreeMap<(u8, u32, u32), Vec<Piece>> = BTreeMap::new();
    graph.for_each(|s| {
        let coords: Vec<(u8, i64, i64)> = s.cells(grid).iter().map(|&c| cell_xy(c)).collect();
        // The blocks the segment touches: each vertex's block and its neighbours' within the margin.
        let mut touched: Vec<(u8, u32, u32)> = Vec::new();
        for &(f, i, j) in &coords {
            for (di, dj) in [(-margin, -margin), (margin, margin), (-margin, margin), (margin, -margin), (0, 0)] {
                let (a, b) = ((i + di).clamp(0, n - 1), (j + dj).clamp(0, n - 1));
                touched.push((f, (a / h) as u32, (b / h) as u32));
            }
        }
        touched.sort_unstable();
        touched.dedup();
        for (f, bi, bj) in touched {
            let (lo_i, hi_i) = (i64::from(bi) * h - margin, (i64::from(bi) + 1) * h + margin);
            let (lo_j, hi_j) = (i64::from(bj) * h - margin, (i64::from(bj) + 1) * h + margin);
            let inside = |&(vf, i, j): &(u8, i64, i64)| vf == f && i >= lo_i && i < hi_i && j >= lo_j && j < hi_j;
            let mut part = 0u32;
            let mut k = 0;
            while k < coords.len() {
                if !inside(&coords[k]) {
                    k += 1;
                    continue;
                }
                let start = k;
                while k < coords.len() && inside(&coords[k]) {
                    k += 1;
                }
                let run = &coords[start..k];
                let steps: Vec<u8> = run
                    .windows(2)
                    .map(|w| {
                        let d = (w[1].1 - w[0].1, w[1].2 - w[0].2);
                        NEIGHBOURS.iter().position(|&nb| nb == d).expect("consecutive vertices of a face are planar neighbours") as u8
                    })
                    .collect();
                let (v0, v1) = (start as u32, k as u32);
                let _ = v1;
                let cuts: Vec<(u32, u32)> = graph.cuts_of(s.index).iter().map(|c| (c.1, c.2)).collect();
                blocks.entry((f, bi, bj)).or_default().push(Piece {
                    segment: ids[s.index],
                    part,
                    v0,
                    seg_vertices: coords.len() as u32,
                    cuts,
                    i0: run[0].1 as i32,
                    j0: run[0].2 as i32,
                    steps,
                    q: s.q[start..k].to_vec(),
                    bed: s.bed[start..k].to_vec(),
                });
                part += 1;
            }
        }
    });
    blocks
        .into_iter()
        .map(|((face, bi, bj), mut pieces)| {
            pieces.sort_unstable_by_key(|p| (p.segment, p.part));
            block_record(world, res, spec, &GraphBlock { face, bi, bj, q_unit: graph.q_unit, pieces })
        })
        .collect()
}

/// The record of one block (pieces in the order given).
pub fn block_record(world: &GlobalWorld, res: u32, spec: &GraphSpec, b: &GraphBlock) -> Record {
    let key = graph_key(world, res, spec, b.face, b.bi, b.bj).expect("a block of the grid");
    let mut body: Vec<u8> = Vec::new();
    for p in &b.pieces {
        assert!(
            p.q.len() == p.bed.len() && p.steps.len() + 1 == p.q.len() && !p.q.is_empty(),
            "a piece holds one step fewer than its vertices"
        );
        body.extend(p.segment.to_le_bytes());
        body.extend(p.part.to_le_bytes());
        body.extend((p.q.len() as u32).to_le_bytes());
        body.extend(p.v0.to_le_bytes());
        body.extend(p.seg_vertices.to_le_bytes());
        body.extend((p.cuts.len() as u32).to_le_bytes());
        body.extend(p.i0.to_le_bytes());
        body.extend(p.j0.to_le_bytes());
        for &(a, b) in &p.cuts {
            body.extend(a.to_le_bytes());
            body.extend(b.to_le_bytes());
        }
        body.extend_from_slice(&p.steps);
        for v in &p.q {
            body.extend(v.to_le_bytes());
        }
        for v in &p.bed {
            body.extend(v.to_le_bytes());
        }
    }
    let mut payload = Vec::with_capacity(HEADER_LEN + body.len() / 2);
    payload.push(CODEC_VERSION);
    payload.extend(GRAPH_VERSION.to_le_bytes());
    payload.push(b.face);
    payload.extend(b.bi.to_le_bytes());
    payload.extend(b.bj.to_le_bytes());
    payload.extend(spec.h1_per_block().to_le_bytes());
    payload.push(unit_byte(b.q_unit));
    payload.extend((b.pieces.len() as u32).to_le_bytes());
    payload.extend(key.0);
    payload.extend(spec.margin_h1.to_le_bytes());
    {
        let mut w = brotli::CompressorWriter::new(&mut payload, BROTLI_BUFFER, BROTLI_QUALITY, BROTLI_WINDOW_BITS);
        w.write_all(&body).expect("writing to a vector");
    }
    Record {
        key,
        kind: KIND_CHANNEL_GRAPH,
        tile: block_tile(spec, b.face, b.bi, b.bj).expect("a block of the grid"),
        generator_version: planet_generators::global::GLOBAL_VERSION,
        hash_version: planet_core::hash::HASH_VERSION,
        node_set_id: world.node_set_id(),
        payload,
    }
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().expect("four bytes"))
}

/// The block stored under `key` for `(face, bi, bj)`: `Ok(None)` when the store holds no record under it, an error naming the record and what was found when it is damaged, of another layout or not the block
/// the key names. The key comes from a commit (it names the block of the commit); `spec` gives the grid the block must be on.
pub fn load_graph_block(
    store: &TileStore,
    spec: &GraphSpec,
    face: u8,
    bi: u32,
    bj: u32,
    key: CacheKey,
) -> Result<Option<GraphBlock>, GraphStoreError> {
    let tile = block_tile(spec, face, bi, bj)?;
    let what =
        |why: String| GraphStoreError(format!("channel graph block {face}/{bi}/{bj} (record kind {KIND_CHANNEL_GRAPH}) key {key}: {why}"));
    let Some(r) = store.get(KIND_CHANNEL_GRAPH, tile, key).map_err(|e| what(e.to_string()))? else { return Ok(None) };
    let p = &r.payload;
    if p.len() < HEADER_LEN || p[0] != CODEC_VERSION {
        return Err(what(format!("a payload of {} bytes that is not codec version {CODEC_VERSION}", p.len())));
    }
    let version = u32_at(p, 1);
    if version != GRAPH_VERSION {
        return Err(what(format!("it holds graph version {version}, the code has {GRAPH_VERSION}")));
    }
    if p[5] != face || u32_at(p, 6) != bi || u32_at(p, 10) != bj {
        return Err(what(format!("it is the block {}/{}/{}", p[5], u32_at(p, 6), u32_at(p, 10))));
    }
    if u32_at(p, 14) != spec.h1_per_block() {
        return Err(what(format!("it is on blocks of {} H1 cells a side, the layout says {}", u32_at(p, 14), spec.h1_per_block())));
    }
    let q_unit = match p[18] {
        1 => QUnit::Discharge,
        2 => QUnit::Area,
        u => return Err(what(format!("its unit byte is {u}, not 1 (discharge) or 2 (area)"))),
    };
    let count = u32_at(p, 19) as usize;
    let margin = u32_at(p, 39);
    if margin < spec.margin_h1 {
        return Err(what(format!(
            "its pieces reach {margin} H1 cells beyond the block, the K7 halo needs {} (rewrite the graph with a margin of at least that)",
            spec.margin_h1
        )));
    }
    if p[23..39] != key.0 {
        return Err(what("the key inside the payload is not the record key".into()));
    }
    // A block holds at most one vertex of any segment in each of its (h + 2)^2 cells (a cell is on one segment), each with at most 5 bytes, a piece header of 36 and a lake cut of 8: the bound of a body.
    let side = u64::from(spec.h1_per_block()) + 2 * u64::from(margin);
    let max_body = ((side * side * (PIECE_FIXED as u64 + 5 + 8) + 4096) as usize).min(MAX_BODY);
    let mut body = Vec::new();
    brotli::Decompressor::new(&p[HEADER_LEN..], BROTLI_BUFFER)
        .take(max_body as u64 + 1)
        .read_to_end(&mut body)
        .map_err(|e| what(format!("its pieces do not decompress: {e}")))?;
    if body.len() > max_body {
        return Err(what(format!(
            "its pieces decode to more than the {max_body} bytes a block of {} H1 cells a side can hold",
            spec.h1_per_block()
        )));
    }
    if count > body.len() / PIECE_FIXED {
        return Err(what(format!("{count} pieces cannot fit in {} bytes", body.len())));
    }
    let mut pieces: Vec<Piece> = Vec::with_capacity(count);
    let mut at = 0usize;
    for k in 0..count {
        if at + PIECE_FIXED > body.len() {
            return Err(what(format!("piece {k} is cut off")));
        }
        let segment = u64::from_le_bytes(body[at..at + 8].try_into().expect("eight bytes"));
        let (part, n) = (u32_at(&body, at + 8), u32_at(&body, at + 12) as usize);
        let (v0, seg_vertices) = (u32_at(&body, at + 16), u32_at(&body, at + 20));
        let ncuts = u32_at(&body, at + 24) as usize;
        let (i0, j0) = (u32_at(&body, at + 28) as i32, u32_at(&body, at + 32) as i32);
        at += PIECE_FIXED;
        if n == 0 {
            return Err(what(format!("piece {k} has no vertex")));
        }
        if ncuts > (body.len() - at) / 8 {
            return Err(what(format!("piece {k} claims {ncuts} lake cuts, {} bytes are left", body.len() - at)));
        }
        let mut cuts = Vec::with_capacity(ncuts);
        for _ in 0..ncuts {
            let (a, b) = (u32_at(&body, at), u32_at(&body, at + 4));
            if a >= b || b >= seg_vertices || cuts.last().is_some_and(|&(_, pb): &(u32, u32)| a <= pb) {
                return Err(what(format!("piece {k} has a lake cut ({a}, {b}) that is empty, unsorted, overlapping or past the segment's {seg_vertices} vertices")));
            }
            cuts.push((a, b));
            at += 8;
        }
        let need = (n - 1) + n * 2 + n * 2;
        if n > body.len() || need > body.len() - at {
            return Err(what(format!("piece {k} claims {n} vertices, {} bytes are left", body.len() - at)));
        }
        let steps = body[at..at + n - 1].to_vec();
        if steps.iter().any(|&s| usize::from(s) >= NEIGHBOURS.len()) {
            return Err(what(format!("piece {k} has a step code outside the {} directions", NEIGHBOURS.len())));
        }
        at += n - 1;
        let q: Vec<u16> = body[at..at + 2 * n].as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
        at += 2 * n;
        let bed: Vec<i16> = body[at..at + 2 * n].as_chunks::<2>().0.iter().map(|c| i16::from_le_bytes(*c)).collect();
        at += 2 * n;
        if k > 0 && (pieces[k - 1].segment, pieces[k - 1].part) >= (segment, part) {
            return Err(what(format!("piece {k} is out of order or repeated")));
        }
        if u64::from(v0) + n as u64 > u64::from(seg_vertices) {
            return Err(what(format!("piece {k} starts at vertex {v0} of a segment of {seg_vertices} and has {n} vertices")));
        }
        pieces.push(Piece { segment, part, v0, seg_vertices, cuts, i0, j0, steps, q, bed });
    }
    if at != body.len() {
        return Err(what(format!("{} bytes follow the last piece", body.len() - at)));
    }
    Ok(Some(GraphBlock { face, bi, bj, q_unit, pieces }))
}
