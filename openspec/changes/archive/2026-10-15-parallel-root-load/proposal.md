## Why

`Streamer::load_roots_blocking` loaded the six level-0 tiles one after another. Over a cold 16384-cell store each root takes about 40 s (it touches most of its face's blocks), so `Rig::new` spent about 243 s before the first frame of every perf scene (#150 notes). The block cache has one decode per block and a lock-free hit path (STRM-014), so concurrent roots are safe and share decodes.

## What Changes

- `Streamer::load_roots_blocking` hands the six root jobs to the spawner together (with cancel tokens) and waits for all of them; the roots become resident in face order after all have arrived. One failing root cancels the others and the error names the tile; nothing is left half-loaded.
- `Spawner::help` (default false; `ManualSpawner` runs one queued job): the wait uses it so deterministic spawners cannot deadlock the caller.
- Spec: STRM-001 gains the start-up root load.
- Measurement example: `store_blocks --part roots` (distinct blocks and decoded bytes per root, load time at N threads).

## Not in this change

- Sampling within one tile on the pool: skipped (see design).
- A shared warm cache across perf scenes (owner/planner decision: every scene stays cold).
- The persisted coarse summary / lattice pyramid (needs an ADR). The cold start stays a tracked finding for the M5 viewer.
