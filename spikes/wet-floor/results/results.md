# SPIKE-21: the wet floor of ADR 0071, cost and memory of the three passes

Code: branch `spike/wet-floor` (never merged as is): `crates/generators/src/wet_floor.rs`, `crates/generators/tests/wet_floor.rs`, `crates/app/examples/spike21_wet_floor.rs`. Machine: Rasierklinge, release build, 20 threads (12 for the in-memory runs), not a perf window (a timing measurement); main fa8e3f8 plus the spike. Date: 2026-10-20.

## Answer

The three passes of ADR 0071 point 3 fit by a wide margin. On the stored 16384-cell bake they take 30 to 32 s in all on 20 threads, hold 151 MB of border edges (peak working set of the whole run 654 MB) and equal the serial priority flood. The stored floors are 0.19 GB with LZ4 (3.2 GB raw). The ADR's estimates were pessimistic on time (3 to 5 minutes) and right on memory (125 MB). Option C stands; option B (the fallback) is not needed.

## Method

- Integer codes `q(e) = floor(2 e) + 24000` clamped to 1 .. 0xFFFE, NaN 0xFFFF, ocean vertex 0. 8-neighbour graph, blocks share their border row and column, cube-face edge and corner copies are one node (`CubeIndex::node_id`).
- Pass (a) per block: a union-find in increasing code, one virtual ocean node, an edge whenever two components that hold terminals (block border vertices, the ocean node) merge. Pass (b): one serial Kruskal-style pass over all edges. Pass (c) per block: a bucketed priority flood seeded with the border floors and the ocean vertices.
- Reference: a serial bucketed priority flood over the whole cube graph (`reference`).

## Equality (tier A in the spike branch)

- Random grids (res 4 to 32, 55 to 70 % low ground, 4 % ocean vertices, 0.2 % NaN elevations, a value per node so shared copies agree): the tiled floors equal the reference for every block size (res, res/2, res/4, 1) and 1 and 3 threads; more than 1,000 finite floors compared.
- Real bakes (seed 1 at 64 cells and seed 7 at 128 cells, ocean threshold 2 %): equal for block sizes 4, 16, 32 and the whole face, 1 and 3 threads.
- The timing example with `--verify`: 0 mismatches at 256 cells (blocks of 128) and 1024 cells (blocks of 512).

## Time and size

| Run | Blocks | Pass (a) | Pass (b) | Pass (c) | Total | Edges (all / max per block / bound) | Held |
|---|---|---|---|---|---|---|---|
| in memory, 256 cells, 8 threads | 24 | 0.01 s | 0.00 s | 0.01 s | 0.02 s | 12,288 / 512 / 513 | 0 MB |
| in memory, 1024 cells, 8 threads | 24 | 0.10 s | 0.01 s | 0.05 s | 0.15 s | 49,152 / 2,048 / 2,049 | 1 MB |
| in memory, 2048 cells, 12 threads | 24 | 0.37 s | 0.01 s | 0.17 s | 0.55 s | 98,304 / 4,096 / 4,097 | 2 MB |
| **stored, 16384 cells, 20 threads** | 1,536 | 17.3 to 17.8 s | 1.2 to 1.4 s | 11.2 to 12.8 s | **29.7 to 32.0 s** | 6,291,155 / 4,096 / 4,097 | 151 MB |

- Every block emits exactly 4 x cells edges (4,096 at 1024 cells): the bound `4 (N - 1) + 1` of the ADR holds with one to spare.
- Stored run: 1,613,760,000 vertices, all with a floor (the land is connected to the ocean through some path; the floor is the largest code on it), 77.7 % wet at the 64 m bound (the ocean included). 3,144,194 border nodes carry a finite floor.
- Peak working set of the stored run, polled every 300 ms: 654 MB (20 blocks of samples in flight, the edges twice in pass (b), the dense map).
- Passes (a) and (c) each decode the 1,536 stored sample blocks and the ocean blocks (11 GB store); the time is dominated by decoding and the union-find, not by the merge.
- Stored size: the u16 floors of 48 sampled blocks (every 32nd) are 5.9 % of their raw size with LZ4 (100.9 MB raw, 5.9 MB packed); all 1,536 blocks: 3.23 GB raw, 0.19 GB LZ4. The sample blocks are 11 GB.
- For comparison, the ocean channel on the same bake: 10.2 s beyond the bake, 55.6 s thread time inside it, the merger held 115 KB.

## Recommendation

Implement ADR 0071 as decided (option C). Notes for the implementing change: pass (a) is a candidate to run inside the bake while the block is in memory (saving one decode), the dense map of pass (b) can be sized from the edge count, and the sort of pass (a) can be a counting sort on the 16-bit codes. Not measured: the point-query side (`band_water` reading the stored floors through the store source), the summary's lowest floor per cell.
