## Why
S-Region-Authoring's E2 (owner, 2026-11-28): the owner marks region boxes in `planet editor`, bakes them with `cargo xtask bake-region` (E1, #1698), and reopens a baked one; today a box can only be typed into `regions.toml` by hand.

## What Changes
- `planet::regionedit`: the editor's region boxes over `planet_cache::regions_file`: the L tile under a direction, a box of 2 to 4 tiles a side snapped to whole tiles and clamped into the allowed band, place, move, resize and delete by `check_box` (the rule E1 bakes by), the derived state per box, the ground outlines of the boxes and of the face-edge margin, the command that opens a ready box.
- `planet editor --regions FILE`; without it the regions file of `--world F` (`<world-dir>/regions.toml`); a seed world without `--regions` has none and the panel says how to name one.
- The Sites tab gets a Regions section; the window draws the boxes, the placement box under the cursor and the margin on the ground.
- A ready box reopens by the printed `--commit` command (the sprint's park path: the world is built once at start, no in-place swap).

## Impact
- Specs: VIEW-077, TEST-166 added.
- Code: `crates/app/src/regionedit.rs` (new), `editwin.rs`, `panels.rs`, `window.rs`, `lib.rs`.
