//! lead #195 (R13, spike only): the resident size of a river graph as the chain holds it, measured on a canonical `graph.txt`: `size_of` of every struct plus
//! each `Vec`'s capacity after `Graph::parse`, per L vertex, per reach and per node; and the portal-only wave 2 (one record per L reach crossing a K7 tile
//! edge, plus the landmass-owned trunks whole).
//!
//! Usage: `graph_mem GRAPH.txt [--tile CELLS] [--min-km2 A]` (tile 768 L cells, GEN-182; A: the vertices counted as channel cells, default 1 km2).
use planet_generators::bake::river_graph::{Attrs, Basin, Domain, Exception, Graph, Level, Node, Reach};
use std::collections::HashSet;
use std::mem::size_of;

/// The record a tile-boundary portal would hold: the crossing's cell, its reach and down node, and the reach's attributes at that vertex.
#[allow(dead_code)]
struct Portal {
    cell: u64,
    reach: u64,
    down: u64,
    a: [f64; 6],
    strahler: u8,
    bed: i32,
}

fn attrs_heap(a: &Attrs) -> usize {
    8 * (a.a_geo.capacity() + a.a_ext.capacity() + a.a_run.capacity() + a.a_run_no_ext.capacity() + a.q.capacity() + a.q_w.capacity())
        + a.strahler.capacity()
        + 4 * a.bed.capacity()
}

fn reach_bytes(r: &Reach) -> usize {
    size_of::<Reach>() + 8 * r.vertices.capacity() + attrs_heap(&r.attrs) + size_of::<Exception>() * r.exceptions.capacity()
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).ok_or("graph_mem GRAPH.txt [--tile CELLS] [--min-km2 A]")?;
    let opt = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let tile: u64 = opt("--tile").map_or(Ok(768), |s| s.parse()).map_err(|_| "--tile CELLS")?;
    let min_m2: f64 = opt("--min-km2").map_or(Ok(1.0), |s| s.parse::<f64>()).map_err(|_| "--min-km2 A")? * 1.0e6;
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let g = Graph::parse(&text).map_err(|e| format!("{path}: {e}"))?;
    drop(text);
    println!(
        "GRAPH-MEM struct sizes: Node {} B, Reach {} B, Attrs {} B, Exception {} B, Basin {} B, Portal {} B",
        size_of::<Node>(),
        size_of::<Reach>(),
        size_of::<Attrs>(),
        size_of::<Exception>(),
        size_of::<Basin>(),
        size_of::<Portal>()
    );
    let graph_vecs =
        size_of::<Node>() * g.nodes.capacity() + size_of::<Reach>() * g.reaches.capacity() + size_of::<Basin>() * g.basins.capacity();
    for lev in [Level::L, Level::H1] {
        let rs: Vec<&Reach> = g.reaches.iter().filter(|r| r.level == lev).collect();
        let verts: usize = rs.iter().map(|r| r.vertices.len()).sum();
        let bytes: usize = rs.iter().map(|r| reach_bytes(r)).sum();
        let nodes = g.nodes.iter().filter(|n| n.level == lev).count();
        let basins = g.basins.iter().filter(|b| b.level == lev).count();
        println!(
            "GRAPH-MEM {lev:?}: {} reaches, {verts} vertices, {nodes} nodes, {basins} basins; reaches {bytes} B ({:.1} B a vertex, {:.0} B a reach), nodes {} B, basins {} B",
            rs.len(),
            bytes as f64 / verts.max(1) as f64,
            bytes as f64 / rs.len().max(1) as f64,
            nodes * size_of::<Node>(),
            basins * size_of::<Basin>()
        );
    }
    let total: usize = size_of::<Graph>() + g.reaches.iter().map(|r| reach_bytes(r) - size_of::<Reach>()).sum::<usize>() + graph_vecs;
    println!("GRAPH-MEM total resident (structs, Vec capacities, the graph's own Vecs): {total} B");
    // the channel cells at the threshold: distinct L vertex cells with a_geo >= min, and the smallest a_geo (one cell's area)
    let mut cells: HashSet<u64> = HashSet::new();
    let mut min_a = f64::INFINITY;
    for r in g.reaches.iter().filter(|r| r.level == Level::L) {
        for (v, &id) in r.vertices.iter().enumerate() {
            let a = r.attrs.a_geo[v];
            if a > 0.0 {
                min_a = min_a.min(a);
            }
            if a >= min_m2 {
                cells.insert(id);
            }
        }
    }
    let l_verts: usize = g.reaches.iter().filter(|r| r.level == Level::L).map(|r| r.vertices.len()).sum();
    let l_bytes: usize = g.reaches.iter().filter(|r| r.level == Level::L).map(reach_bytes).sum();
    let l_ge: usize =
        g.reaches.iter().filter(|r| r.level == Level::L).map(|r| r.attrs.a_geo.iter().filter(|&&a| a >= min_m2).count()).sum();
    println!("GRAPH-MEM L channel cells at >= {} km2: {} distinct ({l_ge} vertices of {l_verts}); smallest a_geo {min_a:.1} m2 (one L cell); L reaches' bytes a vertex {:.1}", min_m2 / 1e6, cells.len(), l_bytes as f64 / l_verts.max(1) as f64);
    // wave 2, portal-only: one record per consecutive L vertex pair in different K7 tiles, plus the landmass-owned reaches whole
    let tile_of = |id: u64| ((id & 0xffff_ffff) / tile, (id >> 32) / tile);
    let mut portals = 0usize;
    for r in g.reaches.iter().filter(|r| r.level == Level::L) {
        for w in r.vertices.windows(2) {
            if tile_of(w[0]) != tile_of(w[1]) {
                portals += 1;
            }
        }
    }
    let trunks: Vec<&Reach> = g.reaches.iter().filter(|r| r.owner == Domain::Landmass).collect();
    let trunk_bytes: usize = trunks.iter().map(|r| reach_bytes(r)).sum();
    let trunk_verts: usize = trunks.iter().map(|r| r.vertices.len()).sum();
    let lm_nodes = g.nodes.iter().filter(|n| n.owner == Domain::Landmass).count();
    println!(
        "GRAPH-MEM wave 2: {portals} portals ({} B), {} landmass reaches ({trunk_verts} vertices, {trunk_bytes} B), {lm_nodes} landmass nodes ({} B); total {} B",
        portals * size_of::<Portal>(),
        trunks.len(),
        lm_nodes * size_of::<Node>(),
        portals * size_of::<Portal>() + trunk_bytes + lm_nodes * size_of::<Node>()
    );
    Ok(())
}
