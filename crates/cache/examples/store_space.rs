//! G-STORE remainder (STRM-079; A3 section 10): the field bake on the new chain, the space a replacement version and a compaction take, and the authored edit items.
//!
//! Usage: store_space <k> [threads] [field_stride] [field_k]. `field_k` samples the field at 3,072 * field_k cells a face edge over the same tiles (12,288 is the regional grid: field_k 4). Needs `landmass_scale <k> 8 dump` and `store_measure <k>` first (`target/store_dump_k<k>`, `target/store_measure_k<k>`).
//! - Field bake: every `field_stride`-th tile of 208 km that touches the landmass, the four channels of the production sample (`global::class_sample`, the K3 field) on the block's (T+1)^2 vertices,
//!   encoded as the production block (byte planes, LZ4) and as a narrow-channel variant (16-bit and 8-bit channels with stated steps, brotli), bytes a vertex by the share of land in the tile.
//! - Space: the store of `store_measure` is the version 1; an edit writes new records (a new key, the size of the record it replaces) for the tiles of its footprint beside it; `pack::compact`
//!   then rewrites the live set. The disk of the directory is sampled during the compaction for its peak. The footprint of a cell is its tile, its eight neighbour tiles and the tiles of its downstream path.
//! - Edit items: the canonical TOML of authored brush items of 1, 10 and 64 strokes, raw and brotli.
//!
//! Measurement encodings, not a store format; the replacement records are copies of the size of the records they replace (an edit changes a record's content, not its size class).
#![allow(clippy::chunks_exact_to_as_chunks, clippy::needless_range_loop)]

use planet_cache::key::CacheKey;
use planet_cache::pack::{
    compact, encode_record, rank_by_pack_age, CompactPolicy, PackWriter, Record, StoreParams, TileStore, DEFAULT_MAX_PACK_BYTES,
};
use planet_core::{Face, TileId, Vec3};
use planet_generators::bake::face::FaceSpec;
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::global::{class_sample, GlobalSample};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::Path;

const R: f64 = 6_371_000.0;
const SEED: u64 = 14;
const TILES_PER_EDGE: usize = 48;
/// Earth at the regional grid: 6 faces of 12,288 cells, and its land cells (1.49e8 km2 over (0.814 km)^2).
const EARTH_CELLS: f64 = 6.0 * 12_288.0 * 12_288.0;
const EARTH_LAND_CELLS: f64 = 2.25e8;
const GB: f64 = 1e9;

type Tile = (u8, u32, u32);
type Row = (f64, usize, usize, usize, [f64; 4], usize);

fn read(dir: &str, name: &str) -> Vec<u8> {
    std::fs::read(format!("{dir}/{name}")).unwrap_or_else(|e| panic!("{dir}/{name}: {e}"))
}

fn brotli(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len() / 2 + 16);
    {
        let mut w = brotli::CompressorWriter::new(&mut out, 64 * 1024, 5, 22);
        w.write_all(raw).expect("writing to a vector");
    }
    out
}

/// Byte planes: plane b holds byte b of every value (values of `width` bytes).
fn planes(values: &[u64], width: usize) -> Vec<u8> {
    let mut raw = vec![0u8; values.len() * width];
    for (i, v) in values.iter().enumerate() {
        for b in 0..width {
            raw[b * values.len() + i] = (v >> (8 * b)) as u8;
        }
    }
    raw
}

fn dir_bytes(dir: &Path) -> u64 {
    std::fs::read_dir(dir).map(|rd| rd.flatten().filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum()).unwrap_or(0)
}

/// The narrow variant of a block: continent i16 at 1/4096, elevation i16 at 0.25 m, temperature u8 at 0.5 degrees from -64, precipitation u16 at 1 mm; returns the bytes, the largest errors and the clamped count.
fn narrow(s: &[GlobalSample]) -> (Vec<u8>, [f64; 4], usize) {
    let mut err = [0.0f64; 4];
    let mut clamped = 0usize;
    let mut cols: [Vec<u64>; 4] = Default::default();
    for g in s {
        let vals = [
            (f64::from(g.continent), 1.0 / 4096.0, -32768.0, 32767.0),
            (f64::from(g.elevation_m), 0.25, -32768.0, 32767.0),
            (f64::from(g.temperature_c) + 64.0, 0.5, 0.0, 255.0),
            (f64::from(g.precipitation_mm), 1.0, 0.0, 65535.0),
        ];
        for (i, (v, step, lo, hi)) in vals.into_iter().enumerate() {
            let r = (v / step).round();
            let c = r.clamp(lo, hi);
            if r != c {
                clamped += 1;
            }
            err[i] = err[i].max((c * step - v).abs());
            cols[i].push(c as i64 as u64);
        }
    }
    let mut raw = planes(&cols[0], 2);
    raw.extend(planes(&cols[1], 2));
    raw.extend(planes(&cols[2], 1));
    raw.extend(planes(&cols[3], 2));
    (brotli(&raw), err, clamped)
}

fn percentile(v: &mut [f64], q: f64) -> f64 {
    v.sort_by(f64::total_cmp);
    v[((v.len() - 1) as f64 * q) as usize]
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let k: usize = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let threads: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(4);
    let stride: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(8);
    let field_k: usize = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(k);
    let nf = 3072 * field_k;
    let tilef = nf / TILES_PER_EDGE;
    let dir = format!("target/store_dump_k{k}");
    let meta = String::from_utf8(read(&dir, "meta.txt")).expect("meta");
    let n: usize = meta.lines().find_map(|l| l.strip_prefix("face_cells ")).and_then(|v| v.parse().ok()).expect("face_cells");
    let nn = n * n;
    let tile = n / TILES_PER_EDGE;
    let cube_id: Vec<u32> = read(&dir, "cube_id.u32").chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
    let receiver: Vec<u32> = read(&dir, "receiver.u32").chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
    let land: Vec<u8> = read(&dir, "land.u8");
    let land_cells = land.iter().filter(|&&l| l == 1).count();
    let cells = cube_id.len();
    let xy = |p: usize| -> (usize, usize, usize) {
        let g = cube_id[p] as usize;
        (g / nn, (g % nn) % n, (g % nn) / n)
    };
    let tile_of = |p: usize| -> Tile {
        let (f, x, y) = xy(p);
        (f as u8, (x / tile) as u32, (y / tile) as u32)
    };
    let tile_id = |t: Tile| TileId::new(Face(t.0), 6, t.1, t.2).expect("a tile id");
    let mut tiles: BTreeMap<Tile, Vec<usize>> = BTreeMap::new();
    for p in 0..cells {
        tiles.entry(tile_of(p)).or_default().push(p);
    }
    println!("k {k}: {n} cells a face edge, tiles of {tile} cells, {cells} cells, {land_cells} land cells, {} tiles", tiles.len());

    // ---------------------------------------------------------------- the field bake on the new chain
    let inputs = BakeInputs::new(SEED, threads);
    let picked: Vec<(Tile, usize)> = tiles
        .iter()
        .enumerate()
        .filter(|(i, _)| i % stride == 0)
        .map(|(_, (t, ps))| (*t, ps.iter().filter(|&&p| land[p] == 1).count()))
        .collect();
    println!(
        "field bake at {nf} cells a face edge: {} of {} tiles sampled (every {stride}th), {} vertices a block",
        picked.len(),
        tiles.len(),
        (tilef + 1) * (tilef + 1)
    );
    let t0 = std::time::Instant::now();
    let rows: Vec<Row> = std::thread::scope(|s| {
        let per = picked.len().div_ceil(threads.max(1));
        let handles: Vec<_> = picked
            .chunks(per.max(1))
            .map(|chunk| {
                let inputs = &inputs;
                s.spawn(move || {
                    chunk
                        .iter()
                        .map(|&(t, landn)| {
                            let spec = FaceSpec { face: Face(t.0), cells: nf, radius_m: R, sea_level: 0.0, seed: SEED, halo: 0 };
                            let mut samples = Vec::with_capacity((tilef + 1) * (tilef + 1));
                            for y in 0..=tilef {
                                for x in 0..=tilef {
                                    let (i, j) = ((t.1 as usize * tilef + x).min(nf - 1), (t.2 as usize * tilef + y).min(nf - 1));
                                    let d: Vec3 = spec.halo_direction(i as i64, j as i64);
                                    samples.push(class_sample(SEED, d, inputs));
                                }
                            }
                            let prod = planet_cache::global_store::encode_block(tilef as u32, &samples).expect("a block").len();
                            let mut wide = Vec::new();
                            let chans: [fn(&GlobalSample) -> f32; 4] =
                                [|g| g.continent, |g| g.elevation_m, |g| g.temperature_c, |g| g.precipitation_mm];
                            for f in chans {
                                let vals: Vec<u64> = samples.iter().map(|g| u64::from(f(g).to_bits())).collect();
                                wide.extend(planes(&vals, 4));
                            }
                            let wide = brotli(&wide).len();
                            let (nb, err, clamped) = narrow(&samples);
                            (landn as f64 / (tile * tile) as f64, prod, wide, nb.len(), err, clamped)
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().expect("a field worker")).collect()
    });
    println!("field bake sampled in {:.0} s", t0.elapsed().as_secs_f64());
    let samples_per_block = ((tilef + 1) * (tilef + 1)) as f64;
    let class = |lo: f64, hi: f64| -> Option<[f64; 4]> {
        let sel: Vec<&Row> = rows.iter().filter(|r| r.0 >= lo && r.0 <= hi).collect();
        if sel.is_empty() {
            return None;
        }
        let m = |f: fn(&Row) -> usize| sel.iter().map(|r| f(r) as f64).sum::<f64>() / (sel.len() as f64 * samples_per_block);
        Some([sel.len() as f64, m(|r| r.1), m(|r| r.2), m(|r| r.3)])
    };
    println!("{:<26} {:>6} {:>14} {:>14} {:>14}", "tiles by land share", "tiles", "production B/v", "brotli f32 B/v", "narrow B/v");
    let mut per_class: Vec<Option<[f64; 4]>> = Vec::new();
    for (name, lo, hi) in
        [("land (>= 75 %)", 0.75, 1.0), ("coast (25 to 75 %)", 0.25, 0.7499), ("sea (< 25 %)", 0.0, 0.2499), ("all sampled", 0.0, 1.0)]
    {
        let c = class(lo, hi);
        match c {
            Some(c) => println!("{name:<26} {:>6.0} {:>14.3} {:>14.3} {:>14.3}", c[0], c[1], c[2], c[3]),
            None => println!("{name:<26} {:>6} (none)", 0),
        }
        per_class.push(c);
    }
    let worst = rows.iter().fold([0.0f64; 4], |m, r| [m[0].max(r.4[0]), m[1].max(r.4[1]), m[2].max(r.4[2]), m[3].max(r.4[3])]);
    let clamped: usize = rows.iter().map(|r| r.5).sum();
    println!(
        "narrow variant: largest error continent {:.2e}, elevation {:.3} m, temperature {:.3} C, precipitation {:.3} mm; {clamped} values clamped",
        worst[0], worst[1], worst[2], worst[3]
    );
    // Earth: the land cells at the land class rate and the rest of the sphere at the sea class rate (the land rate where no sea tile was sampled: an upper bound).
    let land_rate = per_class[0].or(per_class[3]).expect("a sampled tile");
    let sea_rate = per_class[2].unwrap_or(land_rate);
    let earth = |col: usize| (EARTH_LAND_CELLS * land_rate[col] + (EARTH_CELLS - EARTH_LAND_CELLS) * sea_rate[col]) / GB;
    println!(
        "Earth (12,288 cells a face edge, {:.2e} vertices, land {:.2e}): field bake production {:.2} GB, brotli f32 {:.2} GB, narrow {:.2} GB (sea at the {} rate)",
        EARTH_CELLS,
        EARTH_LAND_CELLS,
        earth(1),
        earth(2),
        earth(3),
        if per_class[2].is_some() { "sea-tile" } else { "land-tile" }
    );

    // ---------------------------------------------------------------- the space of versions and compaction
    let sdir = format!("target/store_measure_k{k}");
    let params = StoreParams { hash_version: 1, body: 1 };
    let base = TileStore::open(Path::new(&sdir), params).unwrap_or_else(|e| panic!("{sdir}: {e:?} (run store_measure {k} first)"));
    let records: Vec<Record> = base.sample_records(0).into_iter().map(|r| r.expect("a readable record").record).collect();
    drop(base);
    let steady_records: u64 = records.iter().map(|r| encode_record(r).expect("record").len() as u64).sum();
    let mut by_tile: BTreeMap<TileId, Vec<usize>> = BTreeMap::new();
    for (i, r) in records.iter().enumerate() {
        by_tile.entry(r.tile).or_default().push(i);
    }
    let tile_bytes = |t: &TileId| -> u64 {
        by_tile.get(t).map_or(0, |v| v.iter().map(|&i| encode_record(&records[i]).expect("record").len() as u64).sum())
    };
    let step = (land_cells / 400).max(1);
    let mut foots: Vec<(BTreeSet<TileId>, u64)> = Vec::new();
    let mut seen = 0usize;
    for p in 0..cells {
        if land[p] != 1 {
            continue;
        }
        seen += 1;
        if !seen.is_multiple_of(step) {
            continue;
        }
        let mut set: BTreeSet<TileId> = BTreeSet::new();
        let (f, x, y) = xy(p);
        for dy in [-1i64, 0, 1] {
            for dx in [-1i64, 0, 1] {
                let (tx, ty) =
                    ((x as i64 + dx * tile as i64).div_euclid(tile as i64), (y as i64 + dy * tile as i64).div_euclid(tile as i64));
                if tx >= 0 && ty >= 0 && (tx as usize) < TILES_PER_EDGE && (ty as usize) < TILES_PER_EDGE {
                    set.insert(tile_id((f as u8, tx as u32, ty as u32)));
                }
            }
        }
        let mut c = p;
        let mut guard = 0;
        while receiver[c] != u32::MAX && guard < 10_000_000 {
            c = receiver[c] as usize;
            set.insert(tile_id(tile_of(c)));
            guard += 1;
        }
        let bytes: u64 = set.iter().map(&tile_bytes).sum();
        foots.push((set, bytes));
    }
    let mut tcount: Vec<f64> = foots.iter().map(|f| f.0.len() as f64).collect();
    let mut bcount: Vec<f64> = foots.iter().map(|f| f.1 as f64).collect();
    let b50 = percentile(&mut bcount, 0.5);
    println!(
        "edit footprints (cell, 8 neighbour tiles, downstream path) of {} sampled land cells: tiles p50 {:.0} p90 {:.0} max {:.0}; record bytes p50 {:.0} p90 {:.0} max {:.0}",
        foots.len(),
        percentile(&mut tcount, 0.5),
        percentile(&mut tcount, 0.9),
        percentile(&mut tcount, 1.0),
        b50,
        percentile(&mut bcount, 0.9),
        percentile(&mut bcount, 1.0)
    );
    let median = foots.iter().min_by_key(|f| (f.1 as i64 - b50 as i64).abs()).expect("a footprint").clone();
    let largest = foots.iter().max_by_key(|f| f.1).expect("a footprint").clone();
    let all: BTreeSet<TileId> = by_tile.keys().copied().collect();
    println!(
        "{:<28} {:>6} {:>13} {:>13} {:>15} {:>13} {:>12}",
        "scenario", "tiles", "steady B", "beside B", "compact peak B", "after B", "added B"
    );
    let scenarios: Vec<(&str, BTreeSet<TileId>)> =
        vec![("median edit", median.0), ("largest sampled edit", largest.0), ("full re-bake (every tile)", all)];
    for (name, set) in scenarios {
        let work = format!("target/store_space_k{k}");
        let _ = std::fs::remove_dir_all(&work);
        std::fs::create_dir_all(&work).expect("work dir");
        {
            let mut w = PackWriter::open(Path::new(&work), params, DEFAULT_MAX_PACK_BYTES).expect("a pack");
            for r in &records {
                w.append(r).expect("append");
            }
            w.finish().expect("index");
        }
        let steady = dir_bytes(Path::new(&work));
        {
            let mut w = PackWriter::open(Path::new(&work), params, DEFAULT_MAX_PACK_BYTES).expect("a pack");
            for t in &set {
                for &i in by_tile.get(t).map_or(&[][..], Vec::as_slice) {
                    let r = &records[i];
                    let mut key = [0u8; 16];
                    key.copy_from_slice(&blake3::hash(&[&r.key.0[..], b"edit"].concat()).as_bytes()[..16]);
                    w.append(&Record { key: CacheKey(key), payload: r.payload.clone(), ..r.clone() }).expect("append");
                }
            }
            w.finish().expect("index");
        }
        let beside = dir_bytes(Path::new(&work));
        // the live set: the new records and every record the edit did not replace
        let replaced: BTreeSet<(u8, CacheKey)> =
            set.iter().flat_map(|t| by_tile.get(t).into_iter().flatten().map(|&i| (records[i].kind, records[i].key))).collect();
        let live = |r: &Record| !replaced.contains(&(r.kind, r.key));
        let policy = CompactPolicy { live: &live, cap_bytes: None, rank: &rank_by_pack_age, max_pack_bytes: DEFAULT_MAX_PACK_BYTES };
        let peak = std::sync::atomic::AtomicU64::new(0);
        let stop = std::sync::atomic::AtomicBool::new(false);
        std::thread::scope(|s| {
            let (peak, stop, w) = (&peak, &stop, work.as_str());
            let h = s.spawn(move || {
                while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                    peak.fetch_max(dir_bytes(Path::new(w)), std::sync::atomic::Ordering::Relaxed);
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            });
            let r = compact(Path::new(&work), params, &policy);
            stop.store(true, std::sync::atomic::Ordering::Relaxed);
            h.join().expect("sampler");
            r.expect("compaction")
        });
        let after = dir_bytes(Path::new(&work));
        println!(
            "{name:<28} {:>6} {steady:>13} {beside:>13} {:>15} {after:>13} {:>12}",
            set.len(),
            peak.load(std::sync::atomic::Ordering::Relaxed).max(beside),
            beside - steady
        );
    }
    println!(
        "record bytes of the version 1 (headers, no indexes): {steady_records}; land cells {land_cells}; Earth is {:.2} times the landmass",
        EARTH_LAND_CELLS / land_cells as f64
    );

    // ---------------------------------------------------------------- the authored edit items
    use planet_world_def::item::{Anchor, Item, ItemStroke};
    use planet_world_def::stamp::BrushKind;
    use planet_world_def::BodyId;
    for strokes in [1usize, 10, 64] {
        let item = Item {
            id: 0x0123_4567_89ab_cdef_0123_4567_89ab_cdef,
            body: BodyId::PLANET,
            surface: "sphere".into(),
            layer: "brush".into(),
            priority: 0,
            kind: "brush".into(),
            anchor: Anchor { latitude_deg: 20.0, longitude_deg: 30.0, height_m: 0.0, heading_deg: 0.0 },
            reach_m: 5_000.0,
            max_delta_m: 1_000.0,
            params: BTreeMap::new(),
            base_hash: 7,
            generator_version: 1,
            stage_version: 1,
            name: None,
            notes: None,
            points: Vec::new(),
            group: None,
            part: None,
            mask_layer: None,
            strokes: (0..strokes)
                .map(|s| ItemStroke {
                    kind: BrushKind::Raise,
                    radius_m: 50.0,
                    strength_m: 10.0,
                    target_m: None,
                    points: (0..8).map(|p| (20.0 + 1e-4 * (s * 8 + p) as f64 * 1.37, 30.0 + 1e-4 * (s * 8 + p) as f64 * 0.91)).collect(),
                })
                .collect(),
        };
        let text = item.to_toml().expect("a valid item");
        println!(
            "authored brush item, {strokes} strokes of 8 points: {} B of TOML, {} B brotli",
            text.len(),
            brotli(text.as_bytes()).len()
        );
    }
}
