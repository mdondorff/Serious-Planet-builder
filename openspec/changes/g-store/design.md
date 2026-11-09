## Decisions
- A tile is 208 km (64 cells at 3,072 cells a face edge, 128 at 6,144): the record count follows the Earth tiling of 256 regional cells.
- Encodings: f32 as four byte planes and brotli quality 5 (as kind 13); the receiver as a direction code 0 to 7; segments, basins and keys as byte columns; keys are 16-byte hashes (incompressible, their real size).
- Records are keyed by tile; segments and basins by the tile of their upstream and floor cell. Kinds 20 to 28 are measurement kinds.

## Measurements (prototype landmass, 4 rounds, 8 threads; examples, not gates; B = bytes on disk incl. 61 B record headers and 40 B index entries)
| Item | k = 1 (3,072 cells), B per land cell | k = 2 (6,144 cells), B per land cell | Earth at the regional grid, from k = 2 (GB) |
|---|---|---|---|
| Round records, 4 rounds, land, f32 | 12.47 | 12.07 | 2.72 |
| Routing (receiver direction + area) | 3.74 | 3.66 | 0.82 |
| Channel segments | 3.60 | 1.20 | 0.27 |
| Basins (curves) | 0.054 | 0.030 | 0.007 |
| Channel bits | 0.127 | 0.076 | 0.017 |
| Basin label | 0.049 | 0.023 | 0.005 |
| Catchment-round key table | 0.185 | 0.097 | 0.022 |
| Per-tile border summaries | 0.462 | 0.218 | 0.049 |
| Continuity maps (estimated, identity between builds) | 0.010 | 0.004 | 0.001 |
| **Total** | **20.69** | **17.38** | **3.91** |

- Landmass: 9.31e6 land cells at k = 1 (192.6 MB), 3.72e7 at k = 2 (647.0 MB); 2,687 (k = 1) and 2,685 (k = 2) tiles of 208 km; 80,794 and 177,410 catchments of 2 or more cells; 1,895 and 4,475 basins; 1.20e6 and 1.57e6 segments (initiation 75 km^2). The written packs and indexes match the sum of the records and index entries within the 64 B of file headers. No routing receiver fell outside the 8 neighbours (code 254: 0).
- Earth: 2.25e8 land cells at 814 m (1.49e8 km^2 of land over 0.663 km^2 a cell), a scale factor, not a measurement at that grid. The k = 1 to k = 2 trend is flat for the round records (12.5 to 12.1) and the routing (3.7), so the per-cell sizes at 12,288 are taken as the k = 2 values; k = 1 gives an upper bound of 4.66 GB.
- The new items come to about 3.9 GB at Earth (A3 estimated about 4 GB for the round records and 0.8 to 1.1 GB for the graph: the round records are 2.7 GB, the graph with basins 0.3 GB).
- The round records are 70 % of the new items (3 B a cell a round: f32 heights compress little). They are evictable once the final is built.

## What each row is
- Real bytes in a measurement encoding, with a content that may be smaller or larger than a store's: round records (lossless f32 heights of land cells, final 4 rounds; rounds 1 to 3 re-solved with `steps = 5 r`, assumed to equal the 4-round run's intermediate states, as in `sedround`; a store would probably quantise, so this is likely an over-count); routing (final round only); channel segments, basins, channel bits, basin label (almost all zeros, so 185 KB of 37 MB).
- Under-counts: the key table is built from the final routing only (A3's table covers 4 rounds, up to about 4 times, about +0.3 B a land cell), keys are a hash of (root, members) standing for the real key, and catchments of one cell are left out; the border summaries hold the filled level and the area only, without ids (a lower bound).
- Estimated: continuity maps (identity between two builds). Zero, no builder: edit overrides.
- Left out of the sum: the diffuse area byte (0.1 GB) and H0 (under 0.1 GB) of A3's table; the new-chain items may overlap legacy ones (A3's own total for today is about 36 GB).

## Direction of the Earth extrapolation
- The round records (12.5 to 12.1 B) and the routing (3.7 B) are flat from k = 1 to k = 2; their value at 12,288 is an extrapolation (the f32 compression at a finer grid is unmeasured).
- The segments (3.60 to 1.20 B: the count is nearly constant per km^2, so the cost a cell falls with 1/k^2), the border summaries (0.46 to 0.22) and the key table (0.19 to 0.10) fall with k; taking the k = 2 values at 12,288 over-counts those rows.
- The 4.66 GB bound assumes the k = 1 values bound k = 4; not shown. The prototype landmass is about two thirds of Earth's land; the land cell count is a scale factor.

## Where G-STORE stands
- Measured on the new chain: the 3.9 GB above (4.7 GB upper bound).
- Not measured on the new chain: the field bake (11.7 GB measured on the legacy bake, target 5 to 8 GB with narrow channels) and the local change L with channels (about 24 GB measured on the legacy bake, target 16 to 24 GB; the stored i16 L of the K7 chain is not built).
- With the legacy figures the sum is 11.7 + 24 + 3.9 = about 39.6 GB (about 39.8 with the diffuse byte and H0), at the 40 GB hard limit; with A3's targets 5 to 8 + 16 to 24 + 3.9 to 4.7 = 25 to 37 GB. The gate is decided by the two items not measured here.
- Not measured at all: temporary re-bake space, extra space while a replacement version is committed, retained versions for undo (no code; the pack is append-only, so these need two stores side by side), edit overrides (no serializer).

## Not done
- A reader for the records, and a round trip (the encodings are one way).
- Kind 13 and the field bake on the new chain; steady state and peak temporary separately.
- A measurement at 12,288 cells (the dump needs about 12 GB at H1).
- Earth-specific values (ADR 0108): the land cell count and the 208 km tile are examples of the Earth case.
