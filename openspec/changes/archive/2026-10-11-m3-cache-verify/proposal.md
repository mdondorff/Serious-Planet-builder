## Why

M3 exit criterion and risk register ("stale cache from incomplete cache keys"): `cache-verify` regenerates stored tiles from scratch and compares them with the cache, so a key that leaves an input out is found by a command, not by a wrong picture. It needs a payload format for height tiles and a way to fill a store.

## What Changes

- `planet_cache::codec`: the height tile payload (ADR 0019): codec version, resolution, raw length, an LZ4 block of the heights shuffled by byte index. Bit-exact for every `f32` pattern; hostile headers refused before any allocation. `lz4_flex` `=0.14.0` (`std`, `safe-encode`, `safe-decode`: no unsafe; MIT; `cargo deny` clean).
- `planet_cache::tiles`: `World` (the single planet of a definition: field through the reference generator, radius, store parameters, node set id), `region_tiles`, `build_tiles` (reuses what is stored under the current key), `verify_store` (sample, regenerate, compare decoded heights; stale vs mismatch vs unreadable vs other kind) and `VerifyReport`. `TileStore::sample_records` and `Stored`.
- `generators`: `generate_tile_field` (a tile of any height field; `generate_tile` delegates to it, output unchanged).
- CLI: `planet generate --cache-dir DIR --region FACE,LEVEL [--res N] [--seed N | --world file.toml]` and `planet generate --verify-cache DIR [--sample N] [--seed N | --world file.toml]` (the mismatch lines name tile, pack, offset, key and first differing sample and the command fails); `cargo xtask cache-verify <dir> [...]` runs it in release mode.
- Added: STRM-009 (payload codec). Modified: STRM-004 (now active). Tests: nine in `cache/tests/tiles.rs` (codec round trip, every bit pattern, hand-built layout, hostile payloads, build and reuse, stale, the missing-input scenario with a buggy tile, unreadable and foreign entries, world errors), five in the app and one in xtask.
- Not here: compaction and the size cap, mesh verification (only height tiles are regenerated; other kinds are counted), wiring the store into the streamer, a `--world` for `editor`/`test-render`.
