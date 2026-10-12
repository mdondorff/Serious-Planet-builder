## Why

M3 task 3, first slice: the global-coarse stage needs a field (continents, elevation, climate) and a bake before any stage output can be keyed, stored or read by regional fields. Briefing: new implementation id, bit-exact across runs, thread counts and OSes (CON-14, libm), data products as PNG, 1 km bake under ten minutes.

## What Changes

- `generators::global`: `sample(seed, dir)` (continentalness from a domain-warped fBm, elevation from basins, shelves and ridged mountain ranges inside range zones, temperature from latitude and the lapse rate, precipitation from latitude bands), `bake` onto the six cube-face grids with row-chunked threads, `grid_hash`, and image products (`product` cube net, `equirect_product`). One field with four channels: under ADR 0022 option 1 a chain is one field, so the channels cannot be separate same-stage fields.
- GEN-013 added. Tests (tier A, `generators/tests/global.rs`): thread independence and equality with the pointwise field, face-edge agreement, climate consistency, resolution refusals, image products (written to `target/review/global-coarse/`), pinned known answers.
- Timing: a 1 km bake is 6 x 8193^2 = 403 M samples; at an estimated 4 us per sample per core (not yet measured on the reference machine; the architect measures) that is about 1600 core-seconds, 80 s on 20 threads. Not a test.
- Not here (next changes): 128-bit stage keys and storage of the bake in the tile store, the ADR 0021 point 7 recording contract test (no field reads cells yet), the `generate` command for global bakes and its timing, regional heights that read the bake.
