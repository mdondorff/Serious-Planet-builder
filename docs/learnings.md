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
