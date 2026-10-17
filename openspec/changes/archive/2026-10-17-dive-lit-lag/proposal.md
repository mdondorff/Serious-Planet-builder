## Why
M5 gate item 9a: lit `dive` fails frame max (61 ms vs 40) and descent detail lag (1.70 vs 1). Diagnosis (window 7 logs plus non-measured smokes): the plan thread makes no water queries (0 queries, 0 tile misses, 0 band queries per frame); frame spikes are frame-thread preemption by the 8 helper threads each tile job's water fill starts at normal priority; the lag is the first wave of coarse tile jobs reading up to 25 store blocks each through a 31-block cache.

## What Changes
- `WaterBook::fill` helper threads run at the calling job's priority (STRM-024).
- Frames log: four recorded-only columns for the lit path's water plan (TEST-031); `--frames N` sets the length of a streaming smoke run.

## Impact
Specs: STRM-024, TEST-031 added. No digest, golden, MESH_VERSION or GLOBAL_VERSION change. Frames logs from before TEST-031 are refused as old-format.
Expected golden changes: none
