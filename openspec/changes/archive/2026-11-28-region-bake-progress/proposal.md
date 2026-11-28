## Why
The owner's smoke of S-Region-Authoring (2026-11-28): `bake-region` is silent for about 100 s, and the printed `open:` line lacks `--regions`, so the reopened editor shows no boxes (lead's order 13:50, E1 follow-up).

## What Changes
- A progress line at most every 10 s: region, step, percent done (steps weighted by measured shares: base 16 %, solve and hydrology 10 %, L tiles 74 % of a 104 s 4x4 box), time left; a final line with each step's time.
- `bake_l_progress`, `bake_region_progress`, `bake_entry_progress`: the bakes report their steps; the old functions call them with no-op callbacks.
- One `open:` line per run: `editor <world> [--items DIR] --regions FILE`, without `--commit` (D2: the editor opens every ready region, E2; form agreed with crusader); paths with a space are quoted.

## Impact
- Specs: BUILD-078 modified.
- Code: `crates/cache/src/{l_writer,region_bake,regions_file}.rs`, `crates/cache/examples/bake_regions.rs`; tests in `crates/cache/tests/regions_file.rs`.
