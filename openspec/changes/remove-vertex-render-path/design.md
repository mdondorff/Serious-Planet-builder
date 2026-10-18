## Design: remove the vertex render path (#304)

Brief from opus-planner (2026-10-18), written here so it survives a session restart. Start code only after `render-floor-3` (#303) has merged; until then this file and the proposal are the only edits.

## Sources
- ADR 0062 (retire the vertex tile format). Point 1: `tile_mesh_field`, `TileMesh` and `MeshVertex` stay as the CPU reference, plus a test-only reference renderer.
- ADR 0063 (slot v2 is 9,168 B; 231,090 slots become 219,585).
- ADR 0064 (the floor's roots are exempt from the per-frame upload cap).
- Draft proposal: PR #304, branch `change/remove-vertex-render-path` (draft worktree `C:/DEV/wt-spec/remove`). Work happens in `sonnet-m4` on a branch, not a detached HEAD.

## Order
1. `compact-slot-v2` (#302) and `render-floor-3` (#303, crusader; waiting on the owner's `/bless`) merge first.
2. This change builds on top of #303. Its STRM-021 delta sits on #303's; both touch TEST-024 and TEST-011.

## What is deleted
- In `render`: the vertex pipelines, shaders and draw loops.
- `Place::{Vertex, Fallback}`, the vertex pool and the 256-slot fallback region (`FALLBACK_SLOTS`). The 512-slot pool returns about 30 MB. Use ADR 0063's 231,090 to 219,585 slots in the arithmetic, not ADR 0062's figures.
- `PoolKind::Vertex`, `RenderForm::Vertex` and the vertex payload codec.
- `VertexReason`, `FALLBACK_RELIEF_M`, `finest_class_key`, `--tile-format`, `GPU_STRIDE`, `gpu_climate`.
- The `prefill_present` Vertex/Compact double-check (`crates/app/src/worldfield.rs`, `store_check`, about line 471) goes in this change, not in #303.
- Cover the flags and constants the removal touches: `--prefill-levels`, `--floor-level` (arrives with #303) and the readiness constants.

## What is replaced, not deleted
CLAUDE.md: never weaken or delete a test.
- The 5 vertex-only renderer tests, and the level 0-2 and fallback tests, are replaced by tests on the compact path or on the test-only reference renderer (`crates/render/tests/support`, about 150 lines), so the differential image tests keep an independent reference.
- The software-adapter smoke: WARP loses the device with compact tiles plus the atmosphere (ADR 0062 consequences). The remedy order is in the proposal (re-test, then three waited submissions on the software adapter in `--smoke` only, then the smoke without the atmosphere). The reference renderer is not the remedy.

## Spec deltas
The list is in the proposal (REND-009, 013 to 018, 027; GEN-010, 019, 021; STRM-006, 007, 016, 021; TEST-011, 024, 025; the atmosphere-ocean water minimum; VIEW-005). Real deltas, not an audit.

## Verification
- Tier A/B/C per requirement as the specs declare; the vertex-only tests move to the reference renderer.
- `golden-review` over the goldens this change moves.
- Removal perf window (after merge, on opus-planner's GO only; coders never start perf runs): compact terrain and water GPU per view, before and after removal. Attribute window 11's +17 to 26 % terrain against the vertex format (wide texel fetch, slot v2 climate block, or clock state). If it stays above 10 %, tell opus-planner before #304 merges.
- RTX hash re-record: only if anything is still missing after #303.
- Perf windows run from the Claude desktop app, not the VS Code panel; say so in the window report and compare the readiness gate's `load_calib` values with window 11's.

## To verify against main when #303 and #302 have landed
Checked against `origin/main` 5e06102 on 2026-10-18: `Place::{Vertex, Fallback}`, `FALLBACK_SLOTS = 256`, `VertexReason`, `FALLBACK_RELIEF_M`, `finest_class_key`, `GPU_STRIDE`, `gpu_climate`, `--tile-format` and the `prefill_present` double-check exist. Not yet on main: ADR 0063's 219,585 (pool.rs, GEN-025 and STRM-006 still say 231,090) and `--floor-level`. Re-grep these after #302 and #303 merge before writing tasks.
