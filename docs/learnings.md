# Learnings

Append-only. Added with `/learning`. Each entry names the assumption it changes (spec ID, ADR number, constitution rule, or background report section) and the proposed follow-up.

## 2026-10-10 wgpu 30 headless API differs from older documentation
What happened: first compile of the hello triangle against wgpu 30.0.1 failed on API drift: `RequestAdapterOptions` needs `apply_limit_buckets`, `Instance::new` takes `InstanceDescriptor::new_without_display_handle()`, `RenderPipelineDescriptor` has `multiview_mask`, `get_mapped_range` returns a `Result`, error scopes are popped with `scope.pop()`. Fixed from compiler output in `crates/render/src/lib.rs`; the software adapter (`force_fallback_adapter`) selects WARP on Windows (`Dx12 Microsoft Basic Render Driver`, device type Cpu) and rendered correctly.
Assumption it changes: background report §9/§17 item 18 (wgpu API churn): confirmed, not refuted.
Proposed follow-up: none beyond ADR 0004's pin-and-upgrade-at-milestones rule; record each upgrade here.

## 2026-10-10 nextest rejects a filterset that matches no binary
What happened: `cargo nextest run -E 'not binary(~gpu_) and not binary(~oracle_)'` failed with "operator didn't match any binary names" while no `oracle_*` test binary existed. Tier selection by binary-name prefix therefore needs at least one binary per tier at all times.
Assumption it changes: ADR 0009 (tier naming convention): refined, not refuted.
Proposed follow-up: `crates/render/tests/oracle_shader_validation.rs` (REND-002) is the standing tier-B binary; if it is ever removed, another tier-B test must replace it (noted in the M0 design).

## 2026-10-10 GitHub CLI is not installed on Rasierklinge
What happened: `gh` is not on PATH in Git Bash or PowerShell, so `gh pr create`, CI status checks and the "merge only when CI is green" rule in CLAUDE.md cannot be followed. The remote `origin` is configured (https://github.com/mdondorff/Serious-Planet-builder.git).
Assumption it changes: starter doc §2 (install table) assumed `gh` present.
Proposed follow-up: owner installs `winget install GitHub.cli` and runs `gh auth login`; until then work stays on local branches and CI has not run (listed under "Waiting for the owner" in docs/status.md).

## 2026-10-10 Bit-identical directions do not give crack-free coarse tiles
What happened: numerics review of `core` (m1-coordinates). Face-edge directions are bit-identical from both faces (tested exhaustively to level 3, randomly to level 28 at res 32), but f32 tile-local offsets meet 1 mm only down to level 9 (measured, `coarse_tiles_need_more_than_f32_tile_local_offsets`); at level 0 offsets reach about 3.7e6 m and a shared border vertex quantises differently per tile (about 0.1 to 0.2 m).
Assumption it changes: ADR 0001 / CON-08 (seam work): identical directions are necessary, not sufficient.
Proposed follow-up: the M2 terrain change must rebuild border positions from shared data (or use skirts/morphing), and the lattice exactness needs `level + log2(res) <= 52` (now asserted in `sample_direction`). Float known answers now pin libm bits; libm is pinned to `=0.2.16`; hash has no domain separation yet (add a tag before HASH_VERSION 1 is used for real cache keys).

## 2026-10-10 Fragment `@builtin(position)` ignores the viewport origin assumption
What happened: the debug-view shader derives texel indices from `@builtin(position)` (framebuffer coordinates) minus the draw rectangle origin; this is correct because the viewport and scissor equal the rectangle, and passes on WARP with GPU == CPU twin. It is not verified on other adapters.
Assumption it changes: ADR 0004 (wgpu fit): confirmed on WARP only.
Proposed follow-up: run `cargo xtask test` on lavapipe (CI) and `--real` on Rasierklinge.

## 2026-10-10 Height debug view has low contrast at coarse levels
What happened: with a fixed +-4000 m range the value-noise generator shows about 40 to 75 grey levels per tile at level 3.
Assumption it changes: none; design note for the M2 views.
Proposed follow-up: a per-plan height range is a plan decision (frame), not a renderer one.

## 2026-10-10 A hole test can pass while the geometry is garbage
What happened: adding the `ref_pos` attribute without serialising it misaligned every vertex on the GPU. The tile-id hole test still passed in nine of ten views and only a 24-pixel hole at orbit exposed it; the orbit image showed ragged blobs instead of a disc. Fixed by single-sourcing the vertex layout and adding an oracle test that checks where nodes project.
Assumption it changes: ADR 0009 (tier C metrics): hole counts alone do not prove correct geometry; pair them with a CPU projection oracle.
Proposed follow-up: every GPU-fed buffer layout gets one serialisation function and a test; new visual features get a CPU-projection oracle, not only a coverage metric.

## 2026-10-10 Tile origins on the reference sphere cost precision
What happened: with origins on the reference sphere the f32 jitter test gave 0.22 px at 2.5 km terrain (budget 0.25) because offsets and camera-relative origins both carry the terrain height. Placing the origin at the tile centre height gives 0.0002 px.
Assumption it changes: ADR 0001 (TileLocal origin = tile centre): refined; the centre includes terrain height.
Proposed follow-up: none; M3 per-node height bounds will also fix LOD distance near mountains.

## 2026-10-10 First real-GPU runs: one rounding difference, and a perf log that proved nothing
What happened: on the RTX 3080 Ti Laptop (Vulkan) 15 of 16 tier-C tests passed immediately, including the vertex-placement, morph and depth oracles. The hello triangle failed: the shader returned 0.5 (127.5 in 8 bits), WARP rounds to 128 and NVIDIA to 127. The performance skeleton log (owner run) shows the Balanced power scheme and the GPU in P8 at 210 MHz with throttle reason 0x1 (idle): the 0.43 ms medians are a CPU round trip, not a frame time. Real-GPU debug-view images are byte-identical to the WARP goldens; three terrain views differ by at most 1 in one channel.
Assumption it changes: ADR 0009 (exact debug-view goldens, per adapter): confirmed and justified; report §8 (performance protocol): the harness now flags an unready machine.
Proposed follow-up: shaders must output values that are exact in 8 bits; a real frame workload (M2 terrain) is needed before any budget run; consider blessing the NVIDIA goldens next to the WARP ones.

## 2026-10-10: one-pixel rasteriser difference at a tile border (lavapipe)
terrain_aerial20km_level differs between lavapipe and WARP/NVIDIA in a single pixel (86,89): it sits on the diagonal border of two tiles of different levels and takes the neighbour's colour. Two independent CI runs produced byte-identical Linux images, so it is deterministic per adapter (CON-20), not flakiness; WARP and NVIDIA agree. Cause: edge-tie rasterisation of a shared border differs by implementation. Consequence: goldens stay per adapter, and tile-border pixels must never be compared across adapters.

## 2026-10-10 The tier-B oracles hold on the real NVIDIA GPU with the software-adapter tolerances
What happened: with the oracles following `PLANET_ADAPTER`, `cargo xtask test gpu --real` ran `oracle_debug_views` (face and tile-id exact, height within 1 at rounding boundaries) and `oracle_shader_validation` on the RTX 3080 Ti (Vulkan) and both passed unchanged; all NVIDIA goldens matched in the same run (27 tests).
Assumption it changes: ADR 0009 (tier B on the software adapter): the CPU twins and their tolerances also hold on the reference GPU, so tier B is not WARP-specific.
Proposed follow-up: none; keep running `test gpu --real` at milestone gates.

## 2026-10-11 Height-ignorant selection puts a low camera inside steep terrain, not just under coarse tiles
What happened: on the steep stress field (TEST-015) the ground-1m view (terrain 687.4 m, camera 688.4 m) selects a level-16 leaf under the camera (9.5 m cells; reference ground-1m gets level 19), and because morph is also measured to the height-free reference position the leaf is drawn fully morphed to the level-15 grid (19 m). On slopes up to 72 degrees the interpolated surface rises above the camera (nearest vertex 689.5 m): the frame shows one tile from inside the terrain. The no-holes oracle and morph checks still pass, because the frame is fully covered.
Assumption it changes: ADR 0014 ("a camera low over high terrain gets coarser near tiles than ideal"): the consequence on steep terrain is a camera below the drawn surface. Coverage metrics alone again proved insufficient (see the earlier hole-test learning).
Proposed follow-up: LOD-008 (planned) as the first M3 change: per-node height bounds in selection and morph distances, with a camera-above-surface check in every scripted view on both fields.

## 2026-10-11 Contour-line goldens are not comparable across adapters, and golden tests hid later mismatches
What happened: for PR #43 (LOD-008) `terrain_ground1m_depth` changed about 940 pixels on WARP and NVIDIA but 7 on llvmpipe; every difference sat on derivative-drawn (`fwidth`) contour lines, which lavapipe rasterises differently. Separately, the golden test stopped at its first mismatch, so llvmpipe produced one candidate per CI run (five rounds for five goldens).
Assumption it changes: ADR 0009 (debug views compare exactly and alike on every adapter): views with derivative-drawn lines are exact per adapter only; they are classed shaded in `tests/goldens/classes.txt` and skip the cross-adapter check of ADR 0017.
Proposed follow-up: none beyond ADR 0017 and `GoldenChecks`; new views with screen-space derivatives start as shaded.

## 2026-10-11 The flat-colour flight showed nothing in its last 20 %, so the M2-gate flight approval covered only the dive
What happened: the TEST-014 review sequence used the `tile-id` view, where each tile is one flat colour. At 1 m the camera sees one or two tiles, so the 5 cm glide frames (the ones built to show swimming) are solid; `height`, `normals` and `depth` are also uniform on the smooth reference terrain there. A first `relief` view with a per-tile anchored grid then drew fake seams at tile borders and jumped at LOD splits, and 1 cm contours on an absolute f32 height flicker (ulp 0.24 mm at 3 km).
Assumption it changes: "a debug view that is exact and flat-coloured is enough for a visual acceptance flight" (ADR 0009 / TEST-014). A review sequence needs its own legibility check, and a debug overlay must be world-anchored (per-node f64 phase) or it creates the artefacts it is meant to reveal.
Proposed follow-up: TEST-017 (edge-density check with a tile-id negative control) and the `relief` flight default landed in flight-demo-legibility; the owner re-reviews the relief flight (`target/review/flight-relief-nvidia/`). Related: running `gpu_flight` with `cargo test` (all tests in one process) crashed with STATUS_ACCESS_VIOLATION on WARP; nextest (one process per test, as `cargo xtask test gpu` runs it) is fine; not investigated on main.
