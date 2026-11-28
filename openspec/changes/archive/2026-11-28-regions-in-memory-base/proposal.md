## Why
The owner's E3 smoke and an external review (S-Region-Authoring, 2026-11-28):
- `editor --seed 14 --regions FILE` exited: the viewer fell back to the stale default store `target/accept/global16384`.
- Without a store, `--world` and `--regions` ran on the legacy generator's in-memory bake, not the class-source base `bake-region` bakes against (review F1).
- The Sites tab needed Tab first (F2); Open's status said "copied" but nothing reached the clipboard (F3); the owner calls the title's region status noise.

## What Changes
- `--regions` or `--world` without `--store` (not under `--smoke`): the class-source base computed in memory (`WorldTerrain::open_class`) at 1,024 cells, else `--global-res N`; never a default store (`window::viewer_store`, `class_base_res`).
- A default store that cannot be read is refused naming `--commit DIR`, `--global-res N`, `cargo xtask store`.
- `--regions`, or a regions file beside `--world`, starts the editor mode on.
- Open on a ready box copies its command to the clipboard.
- The window title no longer shows the region status; the start line of the log keeps it.
- `--commit`'s coarse globe outside its exact rectangle is the class source's 1,024-cell bake (review F4; lead, 2026-11-28): the step at the rectangle's edge drops from p95 1,278 m to about 316 m (seed 14, grt-4x4), for about 12 s more at the start; the rest is a known limit.

## Impact
- Specs: VIEW-022, VIEW-052, VIEW-069, VIEW-077 modified.
- Code: `crates/app/src/window.rs`, `worldfield.rs`, `regionedit.rs`, `panels.rs`, `editwin.rs`; tier-A tests in `window.rs`, `regionedit.rs`, `panels.rs`.
