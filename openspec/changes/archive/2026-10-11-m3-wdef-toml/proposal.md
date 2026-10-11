## Why

M3 plan (report row M3, ADR 0007, CON-10): the WorldDefinition needs a file format with its own schema version and tested migrations, and hashing from the parsed form, before anything can be edited by hand or by the editor. Until now definitions existed only in code.

## What Changes

- `planet_world_def::toml_io`: `parse_definition` / `to_toml`. TOML with a required top-level `schema_version` (now 1); a migration engine (`Migration { from, apply }`, `parse_with` for a longer chain than production's, which is empty); canonical writer (version first, sorted tables, shortest round-trip floats, `u64` beyond TOML's `i64` as a decimal string). Values: non-negative integers and finite floats only; every other case is an error naming the path.
- Typing: the paths the program reads have a fixed type (`world.seed` integer, `body.<n>.radius_m` float; an exact integer literal is accepted for the float); other paths keep the type of their literal, which the next change that adds string nodes must revisit (learning). A decimal string for a `u64` is accepted only above `i64::MAX` and in canonical form.
- `WorldDefinition::nodes()`.
- Dependency: `toml` 1.x (pinned `=1.1.8`, `std`, `parse`, `serde` features; MIT or Apache-2.0; adds serde_core, serde_spanned, toml_datetime, toml_parser, toml_writer, winnow, all within `deny.toml`; `cargo deny` clean). ADR 0007 already chose TOML.
- Modified: WDEF-002 (now active), WDEF-003 (hash from the parsed nodes). Fixture `crates/world-def/tests/fixtures/v1_single_planet.toml`.
- Not here: strings, booleans and arrays as node values, the authored-item files and the cell index (WDEF-005, next), a real older schema version (none exists; the engine is tested with a synthetic chain), wiring the loader into the `generate` CLI and `replay`.
