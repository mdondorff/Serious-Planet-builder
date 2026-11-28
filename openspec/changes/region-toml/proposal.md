## Why
S-Region-Authoring's E1 (owner, 2026-11-28, D1): region boxes marked in the editor need a file format and an offline bake that reads the world's own definition and items; today the region bake runs only as the G-RT fixture example over the default world.

## What Changes
- `planet_cache::regions_file`: the `regions.toml` format (D1), `check_box` (the one box rule E1 and E2 share; every box it accepts passes `RegionSpec::check`), the commit folder per region, the derived state `unbaked | ready | stale`, `bake_entry` over the production base of a definition with items.
- `cargo xtask bake-region` over `--world` or `--seed` with `--regions`, `--items`, `--name`, `--force`; the example `bake_regions` of planet-cache does the work.
- API agreed with crusader (E2) on 2026-11-28.

## Impact
- Specs: WDEF-019, BUILD-078, TEST-165 added.
- Code: `crates/cache/src/regions_file.rs`, `crates/cache/examples/bake_regions.rs`, `xtask/src/bakecmd.rs`, `xtask/src/main.rs`; tests in `crates/cache/tests/regions_file.rs` and `crates/app/src/lib.rs`.
