//! G-STORE measurement (STRM-078; A3 section 10): the raw state of a landmass build (`landmass_scale <k> <threads> dump`, in `target/store_dump_k<k>/`) encoded as real pack records,
//! with the pack's record headers and index entries counted, per item of A3's table, in tiles of 208 km (64 cells at k = 1, 128 at k = 2).
//!
//! Usage: store_measure <k>
//!
//! The encodings are measurement encodings (byte-shuffled f32 or columns, brotli quality 5, as the kind 13 records), not the store format: the layouts are the proposal the bytes are counted for.
//! Items with no builder (continuity maps, edit overrides) are estimated from counts and labelled so.

#![allow(clippy::chunks_exact_to_as_chunks)]

use planet_cache::key::CacheKey;
use planet_cache::pack::{encode_record, PackWriter, Record, StoreParams, DEFAULT_MAX_PACK_BYTES};
use planet_core::{Face, TileId};
use planet_generators::bake::Grid;
use std::collections::BTreeMap;
use std::io::Write;

const R: f64 = 6_371_000.0;
const INDEX_ENTRY: u64 = 40;
/// The tiles of a face edge at every k: 3,072 / 64 and 6,144 / 128 (a tile is 208 km).
const TILES_PER_EDGE: usize = 48;
/// The bytes of one basin in `basins.bin`: id 8, floor cell 4, floor 8, spill cell 4, spill 8, cells 4, two curves of 16 f64, inflow 8, class 1, overflows 1, level 8, outflow 8.
const BASIN_BYTES: usize = 8 + 4 + 8 + 4 + 8 + 4 + 32 * 8 + 8 + 1 + 1 + 8 + 8;
const SEGMENT_BYTES: usize = 40;

const K_ROUND: u8 = 20;
const K_ROUTING: u8 = 21;
const K_SEGMENTS: u8 = 22;
const K_BASINS: u8 = 23;
const K_CHANNEL: u8 = 24;
const K_LABEL: u8 = 25;
const K_KEYS: u8 = 26;
const K_BORDER: u8 = 27;
const K_CONTINUITY: u8 = 28;

type Tile = (u8, u32, u32);

fn read(dir: &str, name: &str) -> Vec<u8> {
    std::fs::read(format!("{dir}/{name}")).unwrap_or_else(|e| panic!("{dir}/{name}: {e} (run `landmass_scale <k> <threads> dump` first)"))
}

fn u32s(b: &[u8]) -> Vec<u32> {
    b.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

fn f32s(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

fn brotli(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len() / 2 + 16);
    {
        let mut w = brotli::CompressorWriter::new(&mut out, 64 * 1024, 5, 22);
        w.write_all(raw).expect("writing to a vector");
    }
    out
}

/// 4-byte values as four byte planes (the field bake's shuffle), brotli.
fn shuffled(words: &[[u8; 4]]) -> Vec<u8> {
    let mut raw = vec![0u8; words.len() * 4];
    for (i, w) in words.iter().enumerate() {
        for (b, byte) in w.iter().enumerate() {
            raw[b * words.len() + i] = *byte;
        }
    }
    brotli(&raw)
}

fn shuffled_f32(v: &[f32]) -> Vec<u8> {
    shuffled(&v.iter().map(|x| x.to_le_bytes()).collect::<Vec<_>>())
}

fn shuffled_u32(v: &[u32]) -> Vec<u8> {
    shuffled(&v.iter().map(|x| x.to_le_bytes()).collect::<Vec<_>>())
}

/// Fixed-size rows as byte columns, brotli.
fn columns(rows: &[&[u8]], size: usize) -> Vec<u8> {
    let mut raw = Vec::with_capacity(rows.len() * size);
    for off in 0..size {
        for r in rows {
            raw.push(r[off]);
        }
    }
    brotli(&raw)
}

#[derive(Default)]
struct Item {
    records: u64,
    payload: u64,
    on_disk: u64,
    raw: u64,
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let k: usize = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let dir = format!("target/store_dump_k{k}");
    let meta = String::from_utf8(read(&dir, "meta.txt")).expect("meta");
    let n: usize = meta.lines().find_map(|l| l.strip_prefix("face_cells ")).and_then(|v| v.parse().ok()).expect("face_cells");
    let nn = n * n;
    let tile = n / TILES_PER_EDGE;
    let cube_id = u32s(&read(&dir, "cube_id.u32"));
    let cells = cube_id.len();
    let land: Vec<u8> = read(&dir, "land.u8");
    let land_cells = land.iter().filter(|&&l| l == 1).count();
    let mut bits = vec![0u64; (6 * nn).div_ceil(64)];
    for &g in &cube_id {
        bits[g as usize >> 6] |= 1 << (g as usize & 63);
    }
    let grid = Grid::compact_cube(n, R, bits);
    println!("k {k}: {n} cells a face edge, tiles of {tile} cells, {cells} cells, {land_cells} land cells");
    let xy = |p: usize| -> (usize, usize, usize) {
        let g = cube_id[p] as usize;
        (g / nn, (g % nn) % n, (g % nn) / n)
    };
    let tile_of = |p: usize| -> Tile {
        let (f, x, y) = xy(p);
        (f as u8, (x / tile) as u32, (y / tile) as u32)
    };
    let tile_id = |t: Tile| TileId::new(Face(t.0), 6, t.1, t.2).expect("a tile id");
    let sub_of = |t: Tile| u64::from(t.1) << 32 | u64::from(t.2) << 8 | u64::from(t.0);
    let mut tiles: BTreeMap<Tile, Vec<usize>> = BTreeMap::new();
    for p in 0..cells {
        tiles.entry(tile_of(p)).or_default().push(p);
    }
    println!("{} tiles of 208 km touch the landmass", tiles.len());

    let params = StoreParams { hash_version: 1, body: 1 };
    let out = format!("target/store_measure_k{k}");
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).expect("the output directory");
    let mut writer = PackWriter::open(std::path::Path::new(&out), params, DEFAULT_MAX_PACK_BYTES).expect("a pack");
    let mut items: BTreeMap<u8, Item> = BTreeMap::new();
    let mut put = |kind: u8, t: Tile, seed: u64, payload: Vec<u8>, raw: u64| {
        let mut key = [0u8; 16];
        key.copy_from_slice(&blake3::hash(&[&seed.to_le_bytes()[..], &sub_of(t).to_le_bytes()[..], &[kind]].concat()).as_bytes()[..16]);
        let rec =
            Record { key: CacheKey(key), kind, tile: tile_id(t), generator_version: 1, hash_version: 1, node_set_id: [0; 16], payload };
        let len = encode_record(&rec).expect("a record").len() as u64;
        writer.append(&rec).expect("appending");
        let it = items.entry(kind).or_default();
        it.records += 1;
        it.payload += rec.payload.len() as u64;
        it.on_disk += len + INDEX_ENTRY;
        it.raw += raw;
    };
    let land_of = |ps: &Vec<usize>| -> Vec<usize> { ps.iter().copied().filter(|&p| land[p] == 1).collect() };

    // Round records: the heights of the land cells after each of 4 rounds, f32.
    for r in 1..=4usize {
        let h = f32s(&read(&dir, &format!("heights_r{r}.f32")));
        for (&t, ps) in &tiles {
            let v: Vec<f32> = land_of(ps).iter().map(|&p| h[p]).collect();
            if !v.is_empty() {
                put(K_ROUND, t, r as u64, shuffled_f32(&v), v.len() as u64 * 4);
            }
        }
    }
    let mut off_graph = 0u64;
    // The routing record: the receiver as a direction (0..7, 255 for none) and the drainage area, land cells.
    let receiver = u32s(&read(&dir, "receiver.u32"));
    let area = f32s(&read(&dir, "area.f32"));
    for (&t, ps) in &tiles {
        let lp = land_of(ps);
        if lp.is_empty() {
            continue;
        }
        let dirs: Vec<u8> = lp
            .iter()
            .map(|&p| {
                let r = receiver[p];
                if r == u32::MAX {
                    255
                } else {
                    (0..8).find(|&d| grid.neighbour(p, d) == Some(r as usize)).map_or_else(
                        || {
                            off_graph += 1;
                            254
                        },
                        |d| d as u8,
                    )
                }
            })
            .collect();
        let ar: Vec<f32> = lp.iter().map(|&p| area[p]).collect();
        let mut payload = brotli(&dirs);
        payload.extend(shuffled_f32(&ar));
        put(K_ROUTING, t, 0, payload, lp.len() as u64 * 5);
    }
    // The channel graph: segments and basins by the tile of their upstream / floor cell.
    let seg = read(&dir, "segments.bin");
    let mut st: BTreeMap<Tile, Vec<&[u8]>> = BTreeMap::new();
    for s in seg.chunks_exact(SEGMENT_BYTES) {
        st.entry(tile_of(u32::from_le_bytes([s[8], s[9], s[10], s[11]]) as usize)).or_default().push(s);
    }
    for (t, ss) in &st {
        put(K_SEGMENTS, *t, 0, columns(ss, SEGMENT_BYTES), (ss.len() * SEGMENT_BYTES) as u64);
    }
    let bas = read(&dir, "basins.bin");
    assert!(bas.len().is_multiple_of(BASIN_BYTES), "a basins file is a whole number of basins");
    let mut bt: BTreeMap<Tile, Vec<&[u8]>> = BTreeMap::new();
    for b in bas.chunks_exact(BASIN_BYTES) {
        bt.entry(tile_of(u32::from_le_bytes([b[8], b[9], b[10], b[11]]) as usize)).or_default().push(b);
    }
    for (t, bs) in &bt {
        put(K_BASINS, *t, 0, columns(bs, BASIN_BYTES), (bs.len() * BASIN_BYTES) as u64);
    }
    // The channel bits and the basin label per tile (land cells).
    let chan = read(&dir, "channel.bits");
    let label = u32s(&read(&dir, "label.u32"));
    for (&t, ps) in &tiles {
        let lp = land_of(ps);
        if lp.is_empty() {
            continue;
        }
        let mut bitsv = vec![0u8; lp.len().div_ceil(8)];
        for (i, &p) in lp.iter().enumerate() {
            bitsv[i >> 3] |= ((chan[p >> 3] >> (p & 7)) & 1) << (i & 7);
        }
        put(K_CHANNEL, t, 0, brotli(&bitsv), bitsv.len() as u64);
        let lab: Vec<u32> = lp.iter().map(|&p| label[p]).collect();
        put(K_LABEL, t, 1, shuffled_u32(&lab), lab.len() as u64 * 4);
    }
    // The catchment-round key table of the final routing: per catchment its key (16 B), root and member count, by the tile of its root.
    let mut root = vec![u32::MAX; cells];
    for p in 0..cells {
        let mut c = p;
        let mut path = Vec::new();
        while root[c] == u32::MAX && receiver[c] != u32::MAX {
            path.push(c);
            c = receiver[c] as usize;
        }
        let r = if root[c] == u32::MAX { c as u32 } else { root[c] };
        root[c] = r;
        for q in path {
            root[q] = r;
        }
    }
    let mut members: BTreeMap<u32, u32> = BTreeMap::new();
    for &r in &root {
        *members.entry(r).or_insert(0) += 1;
    }
    let mut kt: BTreeMap<Tile, Vec<(u32, u32)>> = BTreeMap::new();
    for (&r, &m) in &members {
        if m > 1 {
            kt.entry(tile_of(r as usize)).or_default().push((r, m));
        }
    }
    let catchments: usize = kt.values().map(Vec::len).sum();
    for (t, cs) in &kt {
        let mut raw = Vec::new();
        for (r, m) in cs {
            raw.extend(&blake3::hash(&[&r.to_le_bytes()[..], &m.to_le_bytes()[..]].concat()).as_bytes()[..16]);
            raw.extend(r.to_le_bytes());
            raw.extend(m.to_le_bytes());
        }
        put(K_KEYS, *t, 0, brotli(&raw), raw.len() as u64);
    }
    // Per-tile border summaries: the filled level and the drainage area of the border cells of each tile, f32.
    let filled = f32s(&read(&dir, "filled.f32"));
    for (&t, ps) in &tiles {
        let mut v: Vec<f32> = Vec::new();
        for &p in ps {
            let (_, x, y) = xy(p);
            let (x, y) = (x - t.1 as usize * tile, y - t.2 as usize * tile);
            if x == 0 || y == 0 || x == tile - 1 || y == tile - 1 {
                v.push(filled[p]);
                v.push(area[p]);
            }
        }
        if !v.is_empty() {
            put(K_BORDER, t, 0, shuffled_f32(&v), v.len() as u64 * 4);
        }
    }
    // Continuity maps (estimated, identity): per basin (old id, new id, shared cells).
    for (t, bs) in &bt {
        let mut raw = Vec::new();
        for b in bs {
            raw.extend(&b[0..8]);
            raw.extend(&b[0..8]);
            raw.extend(&b[32..36]);
        }
        put(K_CONTINUITY, *t, 0, brotli(&raw), raw.len() as u64);
    }
    writer.finish().expect("the index");
    let (mut packs, mut idx) = (0u64, 0u64);
    for e in std::fs::read_dir(&out).expect("the directory").flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let len = e.metadata().map(|m| m.len()).unwrap_or(0);
        if name.ends_with("tidx") {
            idx += len;
        } else {
            packs += len;
        }
    }
    println!("routing receivers that are not one of the 8 neighbours (code 254): {off_graph}");
    println!("{catchments} catchments of 2 or more cells, {} basins, {} segments", bas.len() / BASIN_BYTES, seg.len() / SEGMENT_BYTES);
    let names = [
        (K_ROUND, "round records (4 rounds, land, f32)"),
        (K_ROUTING, "routing (receiver direction + area)"),
        (K_SEGMENTS, "channel segments"),
        (K_BASINS, "basins (curves)"),
        (K_CHANNEL, "channel bits"),
        (K_LABEL, "basin label"),
        (K_KEYS, "catchment-round key table"),
        (K_BORDER, "per-tile border summaries"),
        (K_CONTINUITY, "continuity maps (estimated, identity)"),
    ];
    println!("{:<42} {:>8} {:>13} {:>13} {:>13} {:>10}", "item", "records", "raw (B)", "payload (B)", "on disk (B)", "B / land");
    let mut total = 0u64;
    for (kind, name) in names {
        if let Some(it) = items.get(&kind) {
            total += it.on_disk;
            println!(
                "{name:<42} {:>8} {:>13} {:>13} {:>13} {:>10.3}",
                it.records,
                it.raw,
                it.payload,
                it.on_disk,
                it.on_disk as f64 / land_cells as f64
            );
        }
    }
    println!(
        "{:<42} {:>8} {:>13} {:>13} {:>13} {:>10.3}",
        "total (records with headers + index entries)",
        "",
        "",
        "",
        total,
        total as f64 / land_cells as f64
    );
    println!("files written: packs {packs} B, indexes {idx} B ({out})");
}
