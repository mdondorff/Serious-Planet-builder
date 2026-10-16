# Learnings, M0 to M3 (milestone scope)

## 2026-10-10 Height debug view has low contrast at coarse levels
What happened: with a fixed +-4000 m range the value-noise generator shows about 40 to 75 grey levels per tile at level 3.
Assumption it changes: none; design note for the M2 views.
Proposed follow-up: a per-plan height range is a plan decision (frame), not a renderer one.

## 2026-10-10 First real-GPU runs: one rounding difference, and a perf log that proved nothing
What happened: on the RTX 3080 Ti Laptop (Vulkan) 15 of 16 tier-C tests passed immediately, including the vertex-placement, morph and depth oracles. The hello triangle failed: the shader returned 0.5 (127.5 in 8 bits), WARP rounds to 128 and NVIDIA to 127. The performance skeleton log (owner run) shows the Balanced power scheme and the GPU in P8 at 210 MHz with throttle reason 0x1 (idle): the 0.43 ms medians are a CPU round trip, not a frame time. Real-GPU debug-view images are byte-identical to the WARP goldens; three terrain views differ by at most 1 in one channel.
Assumption it changes: ADR 0009 (exact debug-view goldens, per adapter): confirmed and justified; report §8 (performance protocol): the harness now flags an unready machine.
Proposed follow-up: shaders must output values that are exact in 8 bits; a real frame workload (M2 terrain) is needed before any budget run; consider blessing the NVIDIA goldens next to the WARP ones.

## 2026-10-11 Height-ignorant selection puts a low camera inside steep terrain, not just under coarse tiles
What happened: on the steep stress field (TEST-015) the ground-1m view (terrain 687.4 m, camera 688.4 m) selects a level-16 leaf under the camera (9.5 m cells; reference ground-1m gets level 19), and because morph is also measured to the height-free reference position the leaf is drawn fully morphed to the level-15 grid (19 m). On slopes up to 72 degrees the interpolated surface rises above the camera (nearest vertex 689.5 m): the frame shows one tile from inside the terrain. The no-holes oracle and morph checks still pass, because the frame is fully covered.
Assumption it changes: ADR 0014 ("a camera low over high terrain gets coarser near tiles than ideal"): the consequence on steep terrain is a camera below the drawn surface. Coverage metrics alone again proved insufficient (see the earlier hole-test learning).
Proposed follow-up: LOD-008 (planned) as the first M3 change: per-node height bounds in selection and morph distances, with a camera-above-surface check in every scripted view on both fields.

## 2026-10-11 Honest selection at 1 m costs 70 % more nodes, and steep terrain has no morph interval at production parameters
What happened: with height-aware volumes (m3-lod-height-aware) the reference ground-1m view plans 8,687 nodes (about 575 per level down to level 26) against 5,108 before; the old count was only possible because the terrain height (about 700 m there) was read as altitude, so the camera was never close to anything fine. Plan time at that view is about 19 ms against 6 ms. Separately, the steep field's per-octave interval bounds are 3 to 6 times wider than the true relief at tile scale (they add as if every octave peaked together), and the morph margin 2 rho then exceeds the room between `split(level)` and `split(parent)`: cells 32 at `viewport_h_px / tau_px` 240 has no crack-free interval on it, cells 4 at 77 does. The plan now rejects such parameters with the tile and numbers.
Assumption it changes: ADR 0013's draw-call budget (6,000, "worst case 5,108") was measured on a selection that ignored height; ADR 0014's claim that a fixed 8 % skirt plus morph handles any terrain the generator makes.
Proposed follow-up: per-tile stored min/max from the M3 field pipeline (exact, hierarchical) behind `height_bounds`; volume caching or incremental selection if the owner's perf run shows plan time over 8 ms; check M3 erosion output against the plan error early.

## 2026-10-11 Two ids for the reference surface, and a radius that bypasses the definition
What happened: the spec-guardian review of m3-multi-body found `planet_world_def::SPHERE_SURFACE_ID` (`sphere`, now what keys use unless a field overrides it) and `planet_core::Sphere::id()` (`sphere:<radius>`) as two different ids for the same surface, and that meshes and plans take the radius from the request (`lod.radius_m`) rather than from `body.N.radius_m` through a `GenContext`; the body parameter hash therefore covers whatever radius the caller passes.
Assumption it changes: "the definition is the only source of body parameters" (CON-05, ADR 0006) holds for seeds but not yet for radius.
Proposed follow-up: when plans take a body (multi-body plans), read radius and surface through the context and use one surface id source; align ADR 0018's body parameter hash (radius, seed, surface) with the code's (which also includes the id) before accepting it.

## 2026-10-11 A numeric scope hides the root tile; stage keys tag the scope kind
What happened: the reviews of m3-stage-model noted that a bare `u64` scope with `0` for "global" aliases the tile with raw id 0 (face 0, level 0), that `all_keys` keyed global fields per tile (a global bake repeated per tile), that the key left out the body, face mapping and resolution that ADR 0006 lists, and that `-0.0` as a halo gave a second key to the same output.
Assumption it changes: "a number is enough to say what an output is for": the scope is a typed value whose kind is hashed, the stage decides which kinds are legal, and every output parameter is a named word of the key.
Proposed follow-up: proposed ADR 0022 lists what stays open (same-stage reads for hydrology chains and local composition, the unit of the halo, several upstream keys for one input); decide CON-08's reading before SPIKE-03.

## 2026-10-11 The flat-colour flight showed nothing in its last 20 %, so the M2-gate flight approval covered only the dive
What happened: the TEST-014 review sequence used the `tile-id` view, where each tile is one flat colour. At 1 m the camera sees one or two tiles, so the 5 cm glide frames (the ones built to show swimming) are solid; `height`, `normals` and `depth` are also uniform on the smooth reference terrain there. A first `relief` view with a per-tile anchored grid then drew fake seams at tile borders and jumped at LOD splits, and 1 cm contours on an absolute f32 height flicker (ulp 0.24 mm at 3 km).
Assumption it changes: "a debug view that is exact and flat-coloured is enough for a visual acceptance flight" (ADR 0009 / TEST-014). A review sequence needs its own legibility check, and a debug overlay must be world-anchored (per-node f64 phase) or it creates the artefacts it is meant to reveal.
Proposed follow-up: TEST-017 (edge-density check with a tile-id negative control) and the `relief` flight default landed in flight-demo-legibility; the owner re-reviews the relief flight (`target/review/flight-relief-nvidia/`). Related: running `gpu_flight` with `cargo test` (all tests in one process) crashed with STATUS_ACCESS_VIOLATION on WARP; nextest (one process per test, as `cargo xtask test gpu` runs it) is fine; not investigated on main.

## 2026-10-13 A tier A test that bakes and plans twenty views is not a fast test
What happened: `generate --plan-check` was first tested inside the app crate's unit tests (a 256-cell bake, both fields, ten views each): 4 s, one of the slowest tests in `test fast`, which then failed its 10 s budget once inside `cargo xtask accept M3` (11.0 s; 8.4 s on a quiet machine). The spec-guardian review found it; it moved to `crates/app/tests/oracle_plan_check.rs` (tier B).
Assumption it changes: "a small bake makes a CLI test cheap": the planning of every view is the cost, not the bake. The frame test already plans the same flight on the same bake size; the CLI test only needs the refusal paths in tier A.
Proposed follow-up: none; new app-level tests that plan frames start in a tier B binary.
