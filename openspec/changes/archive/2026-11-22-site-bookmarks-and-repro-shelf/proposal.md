# Site Bookmarks & F9 Repro Shelf Data Model (`VIEW-073`, `VIEW-074`)

## Why
Flying back to interesting terrain locations or re-inspecting saved F9 repro bundles inside `planet editor` requires a pure GPU-free model for geographic camera readouts, coordinate jumps (`LAT,LON[,ALT]`), persistent site bookmarks stored outside `WorldDefinition`, and scanning captured `.bundle` / `-path.txt` / `.png` artifacts in `PLANET_REPRO_DIR`.

## What Changes
- Add `crates/editor/src/bookmarks.rs` (`CameraReadout`, `Bookmark`, `BookmarkShelf`, `JumpTarget`, `parse_jump_target`) using `libm` and `tangent_frame` for pole-safe `f64` round-trips and atomic `version = 1` TOML persistence outside `WorldDefinition` (`VIEW-073`).
- Add `crates/editor/src/repro_shelf.rs` (`ReproShelf`, `ReproEntry`, `ReproCompatibility`, `BrokenBundle`, `BundleKind`) scanning `.bundle`, `-path.txt`, and `.png` captures newest-first with corrupt-bundle isolation and `cargo xtask repro` command synthesis (`VIEW-074`).
- Expose `bookmarks` and `repro_shelf` in `crates/editor/src/lib.rs`, add Tier-A tests in `crates/editor/tests/bookmarks_and_repros.rs`, and specify `VIEW-073` and `VIEW-074` in `openspec/specs/viewer/spec.md`.
