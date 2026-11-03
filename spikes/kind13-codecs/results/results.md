# SPIKE-97: pure-Rust codecs for the erosion layer's tile record (kind 13)

Question (owner, via opus-planner, 2026-11-03): which pure-Rust codec can hold a kind 13 record, and what does it cost in size, decode time and error. Analysis and measurement only; no dependency was added to the main crates.

## Finding first
- `ruzstd` 0.8.3 has a pure-Rust encoder, but it implements only level `Fastest`. `Default`, `Better` and `Best` panic with `not implemented` (`frame_compressor.rs:203`). A pure-Rust zstd therefore gives only 0.88 of lz4_flex's size; the zstd 3 and zstd 19 figures of the earlier study (`sizes.out`: 21.7 and 19.2 GB at 0.25 m with row-delta) need the C library (`zstd` crate).
- Speeds below are **indicative only, from a loaded machine** (4 to 7 other cargo and rustc processes of other sessions ran throughout; one codec's decode varied up to 4x between runs). The ratios (sizes) are exact and unaffected by load. Local CPU timings are not evidence (planner rule, 2026-11-03).

## Data and method
- 12 real kind 13 tiles from the seed-14 full-resolution `--region` bake (PR 5): each 3072 x 3072 cells of i16 counts, 18.87 MB raw; the planet is bounded as 2,742 land tiles x the 12-tile mean.
- Two quantisation steps: 0.25 m (as built) and 1 m. Each plain and with row-delta (first value of a row, then the wrapping difference to the left neighbour).
- Codecs: `lz4_flex` 0.11.6 (MIT, the baseline), `ruzstd` 0.8.3 (MIT), `brotli` 8.0.4 (rust-brotli; BSD-3-Clause AND MIT), `miniz_oxide` 0.8.9 (deflate; MIT OR Zlib OR Apache-2.0). Maintenance state was not checked beyond these versions being the newest in the local registry.
- Sizes: all 12 tiles for lz4, ruzstd, brotli 5 and deflate. Brotli 9 and 11 (37 to 108 s a tile to encode) ran on the first 3 tiles only; their figures below are scaled by their size relative to lz4_flex on the same 3 tiles (lz4 on those 3: 1.977 / 1.609 / 1.496 / 1.151 B per land cell in the order of the table), then applied to lz4's 12-tile figure (marked `~`).
- Timings: first 3 tiles, single thread, release build, decode min of 5 (encode min of 5 where a run takes under 3 s, else one run). Every decode was checked to round-trip.
- Reproduce: `proto/main.rs` and `proto/Cargo.toml.txt` (a scratch crate; run `codecs <dir of f32 tiles> 3`); raw output `raw/codecs-12-tiles.txt`. The proto source is the final one (it also holds the extra `lz4_flex 3 tiles` baseline row, which is not in the raw file).

## Size (planet, GB, bytes per land cell)
| Codec | 0.25 m plain | 0.25 m row-delta | 1 m plain | 1 m row-delta |
|---|---|---|---|---|
| lz4_flex (as built) | 39.4 (1.81) | 34.2 (1.57) | 30.8 (1.41) | 25.3 (1.16) |
| ruzstd Fastest | 34.7 (1.59) | 30.1 (1.38) | 26.8 (1.23) | 22.3 (1.02) |
| deflate 6 | 28.4 (1.30) | 22.1 (1.01) | 20.2 (0.93) | 15.5 (0.71) |
| deflate 9 | 28.4 (1.30) | 22.0 (1.01) | 20.2 (0.93) | 15.3 (0.70) |
| brotli 5 | 25.4 (1.17) | 20.1 (0.92) | 17.7 (0.81) | 14.3 (0.66) |
| brotli 9 | ~22.7 (~1.04) | ~19.5 (~0.90) | ~16.3 (~0.75) | ~14.4 (~0.66) |
| brotli 11 | ~20.0 (~0.92) | ~17.7 (~0.82) | ~14.3 (~0.66) | ~12.8 (~0.59) |

- Row-delta helps every codec by 13 to 22 %.
- Brotli 5 is within 12 % of brotli 11 at about 1/100 of its encode time (1.3 to 6.5 s a tile against 37 to 108 s); brotli 11 is not usable in a bake.
- For reference, with the C `zstd` crate (earlier study): row-delta + zstd 3 21.7 GB, zstd 19 19.2 GB at 0.25 m; 15.5 and 13.0 GB at 1 m.

## Speed (single thread, one tile = 18.87 MB raw; loaded, indicative only)
| Codec | Encode s a tile | Decode ms a tile | Decode vs lz4_flex (same run) |
|---|---|---|---|
| lz4_flex | 0.03 to 0.1 | 5 to 11 at 0.25 m; 18 to 36 at 1 m and under load | 1x |
| ruzstd Fastest | 0.4 to 1.0 | 76 to 270 | 10 to 17 times slower |
| brotli 5 | 1.3 to 6.5 | 107 to 270 | 9 to 15 times slower |
| deflate 6 / 9 | 0.9 to 15 | 84 to 360 | 6 to 20 times slower |
| brotli 11 | 37 to 108 | 134 to 490 | 10 to 40 times slower |

The row-delta undo (one pass of wrapping adds) is not in the decode figures; it was not measured.

## Decode budget
- Source of render-tile rate: `perf/logs/dive-lit.json` and `dive-lit.frames.txt` (committed 2026-11-01): scene `stream-lit/dive`, 2,400 frames = 40 s of script time, **13,727 tiles dispatched** (343 a second), 13,701 uploaded, 1,440 cancelled, 130 MB uploaded. The session is loaded (`valid_for_budgets: false`) and read the `global16384` bake, which has no kind 13 layer; it gives only how fast render tiles are requested.
- Kind 13 is per full-resolution bake tile: 3072 cells x 101.8 m = **312.7 km** an edge (98,304 cells a face edge, 32 tiles an edge). The flight needs a new kind 13 tile only when it crosses a tile edge: speed / 312.7 km.

| Flight | Speed (m/s) | Kind 13 tiles a second | Seconds between tiles |
|---|---|---|---|
| dive walk segment (TEST-018) | 1.5 | 0.000005 | 208,000 |
| dive-fast vehicle | 30 | 0.0001 | 10,400 |
| envelope, 100 m (M8-V chunk curve) | 280 | 0.0009 | 1,100 |
| envelope top, 300 m (M8-V criterion 7) | 920 | 0.0029 | 340 |

- Decode load against terrain's share (BUDG-003, B = 18: terrain has 12 threads): at 0.0029 tiles a second (a tile corner crossing may need up to 3 more: take 4x, 0.012 a second) the slowest option here (270 ms) costs 0.003 thread-seconds a second, **0.03 % of terrain's 12 threads**; lz4_flex (11 ms) costs 0.001 %. The flight alone does not decide the codec.
- Decode per render tile is a different matter and is decided by what a render tile reads. If every dispatched render tile of the dive rate (343 a second) decoded a whole kind 13 tile: brotli 5 at 107 ms is 37 thread-seconds a second (**3.1 times terrain's 12 threads**), lz4_flex at 11 ms is 3.8 (**31 %**). A decoded tile (or a window of one) must therefore be cached and shared by the render tiles of its 312.7 km square; most of the dive's render tiles are coarse and would not read kind 13 at all (not measured here).
- A decoded tile is 37.7 MB as f32 and 18.9 MB as i16 (a decoded tile cache sized in tiles is a separate question).
- Not measured: decode into the cache under real flight, the effect of whole-tile granularity on a first read at 1 m altitude (one tile decode is 0.1 to 0.3 s on one thread).

## Options for the owner
| Option | Planet size | Decode a tile (single thread, loaded) | Error |
|---|---|---|---|
| lz4_flex + row-delta, 0.25 m (as built plus row-delta) | 34.2 GB | 11 to 20 ms | at most 0.125 m against the true f32 (half a step) |
| brotli 5 + row-delta, 0.25 m | 20.1 GB | 107 ms (115 to 160 ms in other runs) | the same, 0.125 m |
| brotli 5 + row-delta, 1 m | 14.3 GB | about 270 ms | at most 0.5 m against the 0.25 m stored value (p99 0.5 m), 0.625 m against the true f32 |

Dependencies of the options: lz4_flex is already in `planet-cache`; brotli (rust-brotli, BSD-3-Clause AND MIT, plus `brotli-decompressor` 5.0.3) and any ruzstd or deflate crate are new dependencies of `planet-cache` and need the owner's approval. No recommendation here.
