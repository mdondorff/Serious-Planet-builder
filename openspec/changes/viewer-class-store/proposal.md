## Why
- Explorer's #1162 and #1164 (GEN-115, STRM-073, STRM-074): a store can be baked from the class source and records it, and the erosion layer carries a manifest (shape, versions, field source, change range). The viewer must show the world the store records and read the layer by its manifest; the preview store is `global16384-m9c14` (seed 14, class source).
- Inbox #59 (planner, 2026-11-04), and the planner's follow-up: `StoreLayer::open` and the viewer's key check read `LayerManifest::read(dir)`.

## What Changes
- The viewer, `generate --plan-check` and `generate --coarse-horizon` open the world the store records through `GlobalWorld::for_store` (the viewer terrain already did, #1162): a store of both bakes asks for `--class-source` or `--legacy-source` (the viewer takes them now), naming both.
- `--erosion on` uses the terrain's own world for the layer's keys, so a class store works without a marker flag; a store holding the legacy generator's bake is refused unless `--erosion-unchecked`.
- `StoreLayer::open_recorded` reads the layer's manifest: its versions (`check_current`), its field source against the world's (refused naming both), its shape (the options `--erosion-face-res` and `--erosion-res` become replacements) and its change range for the bounds (instead of the representable +-8.2 km); a layer without a manifest takes the production shape and the representable range. `GlobalWorld` is `Clone`.
- The title names the world's field source (class or legacy) and, with the layer on, "erosion layer"; the log prints the description.

## Impact
- Code: `app` (`worldfield`, `window`, `plancheck`, `horizonworld`, `skeleton`, `lib`), `cache` (`layer_tiles`, `global_store`). Requirement VIEW-065 added (VIEW-062 and VIEW-063 edited in viewer-erosion's delta). No key, record or golden moves.
