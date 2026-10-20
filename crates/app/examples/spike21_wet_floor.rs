//! SPIKE-21 (throwaway): time and size of the three passes of the wet floor (ADR 0071 point 3) on an in-memory bake or a stored one.
//! `cargo run --release -p planet --example spike21_wet_floor -- --mem 1024 [--cells 512] [--threads 8] [--verify]`
//! `cargo run --release -p planet --example spike21_wet_floor -- --store DIR --global-res 16384 [--threads 20]`

use std::time::Instant;

use planet_cache::global_store::{decode_block, decode_ocean_block, GlobalWorld, KIND_GLOBAL_BLOCK, KIND_GLOBAL_OCEAN};
use planet_core::cube::TangentWarp;
use planet_core::TileId;
use planet_generators::global::bake;
use planet_generators::ocean::{block_tiles, grid_block, parallel_map, tiled_ocean, CubeIndex, OceanBits};
use planet_generators::wet_floor::{block_codes, pass_a, pass_b, pass_c, reference, INF};
use planet_world_def::WorldDefinition;

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

fn has(name: &str) -> bool {
    std::env::args().any(|x| x == name)
}

fn main() -> Result<(), String> {
    let threads: usize = arg("--threads").map(|v| v.parse().unwrap()).unwrap_or(8);
    let (res, cells, tiles, codes_of): (u32, u32, Vec<TileId>, Box<dyn Fn(TileId) -> Vec<u16> + Sync>);
    let mut mem_grid = None;
    if let Some(r) = arg("--mem") {
        res = r.parse().unwrap();
        cells = arg("--cells").map(|v| v.parse().unwrap()).unwrap_or(res.min(1024));
        let t = Instant::now();
        let grid = bake(1, &TangentWarp, res, threads).map_err(|e| e.to_string())?;
        println!("bake {res} cells: {:.1} s", t.elapsed().as_secs_f64());
        let t = Instant::now();
        let (_, ocean) = tiled_ocean(&grid, &TangentWarp, cells, threads, 0.02).map_err(|e| e.to_string())?;
        println!("ocean channel (tiled, {cells} cells per block): {:.2} s, {} ocean vertices", t.elapsed().as_secs_f64(), ocean.count());
        tiles = block_tiles(res, cells).map_err(|e| e.to_string())?;
        let grid = std::sync::Arc::new(grid);
        let ocean = std::sync::Arc::new(ocean);
        let (g2, o2) = (grid.clone(), ocean.clone());
        codes_of = Box::new(move |t| {
            let samples = grid_block(&g2, t, cells);
            let mut bits = OceanBits::zeros(cells);
            for l in 0..=cells {
                for k in 0..=cells {
                    if o2.get(t.face(), t.x() * cells + k, t.y() * cells + l) {
                        bits.set(k, l);
                    }
                }
            }
            block_codes(cells, &samples, &bits)
        });
        mem_grid = Some((grid, ocean));
    } else {
        let dir = arg("--store").ok_or("--mem N or --store DIR needed")?;
        res = arg("--global-res").ok_or("--global-res N needed")?.parse().map_err(|e| format!("{e}"))?;
        let def = WorldDefinition::single_planet(6_371_000.0, 1);
        let world = GlobalWorld::from_definition(&def).map_err(|e| format!("{e}"))?;
        let store = planet_cache::pack::TileStore::open(std::path::Path::new(&dir), world.params()).map_err(|e| e.to_string())?;
        let (level, c) = world.layout(res).map_err(|e| format!("{e:?}"))?;
        cells = c;
        let total = 6usize << (2 * u32::from(level));
        let side = 1usize << level;
        tiles = (0..total)
            .map(|i| {
                let (face, rest) = (i / (side * side), i % (side * side));
                TileId::new(planet_core::Face(face as u8), level, (rest % side) as u32, (rest / side) as u32).unwrap()
            })
            .collect();
        codes_of = Box::new(move |t| {
            let key = world.block_key(t, cells);
            let rec = store.get(KIND_GLOBAL_BLOCK, t, key).expect("read").expect("block stored");
            let (_, samples) = decode_block(&rec.payload).expect("decode");
            let okey = world.ocean_key(t, cells);
            let orec = store.get(KIND_GLOBAL_OCEAN, t, okey).expect("read").expect("ocean block stored");
            let bits = decode_ocean_block(&orec.payload).expect("decode ocean");
            block_codes(cells, &samples, &bits)
        });
    }
    println!("{res} cells per face edge, blocks of {cells} cells: {} blocks, {threads} threads", tiles.len());
    let index = CubeIndex::new(res);
    let idx: Vec<usize> = (0..tiles.len()).collect();

    let t = Instant::now();
    let edges = parallel_map(&idx, threads, &|&i| pass_a(index, tiles[i], cells, &codes_of(tiles[i])));
    let a = t.elapsed().as_secs_f64();
    let total_edges: usize = edges.iter().map(|b| b.edges.len()).sum();
    let max_edges = edges.iter().map(|b| b.edges.len()).max().unwrap_or(0);
    println!(
        "pass (a): {a:.2} s; edges {total_edges} in all, {max_edges} at most per block (bound {}), {:.0} MB held",
        4 * u64::from(cells) + 1,
        (total_edges * 24) as f64 / 1e6
    );

    let t = Instant::now();
    let border = pass_b(&edges);
    let b = t.elapsed().as_secs_f64();
    println!("pass (b): {b:.2} s; {} border nodes with a finite floor", border.len());

    let t = Instant::now();
    let floors = parallel_map(&idx, threads, &|&i| {
        let codes = codes_of(tiles[i]);
        let f = pass_c(index, tiles[i], cells, &codes, &border);
        let finite = f.iter().filter(|v| **v != INF).count();
        let wet64 = f.iter().filter(|v| **v <= 24_128).count();
        // The size of the block as the store would hold it: the u16 floors little endian, LZ4 (every 32nd block, to bound the time).
        let packed = if i % 32 == 0 {
            let raw: Vec<u8> = f.iter().flat_map(|v| v.to_le_bytes()).collect();
            (raw.len(), lz4_flex::compress(&raw).len())
        } else {
            (0, 0)
        };
        (finite, wet64, if has("--verify") { f } else { Vec::new() }, packed)
    });
    let c = t.elapsed().as_secs_f64();
    let finite: usize = floors.iter().map(|f| f.0).sum();
    let wet: usize = floors.iter().map(|f| f.1).sum();
    let verts = tiles.len() * (cells as usize + 1) * (cells as usize + 1);
    println!(
        "pass (c): {c:.2} s; {finite} of {verts} vertices have a floor, {wet} are wet at 64 m ({:.1} %)",
        100.0 * wet as f64 / verts as f64
    );
    println!("total {:.2} s", a + b + c);
    let (raw, lz4): (usize, usize) = floors.iter().fold((0, 0), |s, f| (s.0 + f.3 .0, s.1 + f.3 .1));
    let blocks_sampled = floors.iter().filter(|f| f.3 .0 > 0).count();
    if raw > 0 {
        println!(
            "stored size: {blocks_sampled} blocks sampled, {raw} bytes raw, {lz4} bytes LZ4 ({:.1} %); all {} blocks about {:.2} GB raw, {:.2} GB LZ4",
            100.0 * lz4 as f64 / raw as f64,
            tiles.len(),
            raw as f64 / blocks_sampled as f64 * tiles.len() as f64 / 1e9,
            lz4 as f64 / blocks_sampled as f64 * tiles.len() as f64 / 1e9
        );
    }

    if has("--verify") {
        let (grid, ocean) = mem_grid.ok_or("--verify needs --mem")?;
        let t = Instant::now();
        let want = reference(&grid, &ocean);
        println!("serial reference: {:.2} s", t.elapsed().as_secs_f64());
        let mut bad = 0usize;
        for (i, tile) in tiles.iter().enumerate() {
            let n = cells as usize + 1;
            for l in 0..n {
                for k in 0..n {
                    let node = index.node_id(tile.face(), tile.x() * cells + k as u32, tile.y() * cells + l as u32);
                    if floors[i].2[l * n + k] != want[node as usize] {
                        bad += 1;
                    }
                }
            }
        }
        println!("mismatches against the serial reference: {bad}");
        if bad > 0 {
            return Err(format!("{bad} mismatches"));
        }
    }
    Ok(())
}
