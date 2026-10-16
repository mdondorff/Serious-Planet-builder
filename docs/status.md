# Status

Forward state only (owner, 2026-10-16): open owner items, the current queue's open items, the current unanswered stop report, tracked items and quarantines. History goes to `docs/log.md`. When a change is archived the coder deletes its queue item's text (the log line is the record); when the architect answers a stop report it deletes it. The `housekeeping` skill cleans the rest at milestone boundaries. Earlier states: `docs/archive/M4/status-at-m4-acceptance.md`.

## Waiting for the owner
- Nothing open. (M4 accepted by the owner 2026-10-16 with the `test fast` budget raised to 20 s; M5 started.)

Dormant (no near trigger; not asked now):
- **CON-08 reading** (ADR 0022): may a field read another field of its own stage? Trigger: a regional chain that needs same-stage reads. Until then one field per chain.
- **ADR 0045** (bodies, bake resolution and the storage budget; point 3 decided): proposed. Trigger: multi-body work, or the editor (M6) for point 7.
- **ADR 0036 point 3** (GPU noise floor 0.1 ms): proposed. Trigger: the first M5 perf window with GPU verdicts.

## Tracked
- **Perf windows run from a native-UI coder session** on the architect's authorization (owner, 2026-10-16; CLAUDE.md). A gated scene counts only with `load_calib_before_single_us` <= 50,000 and `load_calib_before_parallel_median_us` <= 100,000. Why the planner's VS Code-terminal session runs about 1.4x slower per thread is not known.
- **WARP parallel-device crash** (`gpu_skeleton`, `gpu_warmup`: STATUS_ACCESS_VIOLATION under plain parallel `cargo test` on WARP; nextest, xtask and CI isolate each test): sonnet-spikes explores it on the planner's go (briefed 2026-10-16).
- **M3 gate condition 2** (trigger: before the first regional field is stored): per-tile node set id (ADR 0025 point 3), the declared-versus-recorded reads contract test (ADR 0021 point 7), a tile-level GEN-017 test.
- **Cold start without the pre-fill:** dive-cold lag 1.77 levels and a 48 ms frame 2 (M4 report). The viewer window measures the real cold start once.

## M5 Atmosphere & ocean: design constraints (owner, 2026-10-15)
Scope as in the architecture report (Hillaire atmosphere, aerial perspective, HDR and exposure, ocean surface and shoreline; gate after M5). These constraints keep later work possible without rework:
1. **Water level is a field, not a constant.** A water-surface height W(dir) and a water-body kind sit next to `height(dir)`; in M5 the sea is the only producer (a constant level). Shoreline and water rendering read W and the kind, never a hard-coded sea level (lakes, rivers and deltas plug in later as producers).
2. **Ocean connectivity in the global bake.** A low area is sea only if it connects to the ocean; inland basins below sea level stay dry in M5 (future lakes).
3. **Liquid kinds are data.** The kind of liquid (water, lava, other) is a parameter table in the world definition (colour, emission, viscosity, waves or none), not a code enum.
4. **HDR and lighting accept emissive surfaces** (lava glow, lighting the atmosphere from below).
5. **A retrofit test:** one synthetic lake (a basin with a fixed level) renders through the same water path, proving a second producer works before hydrology exists.

Owner's decisions for an autonomous M5 (2026-10-15): new M5 goldens are blessed under policy and the look is reviewed once at the gate (ADR 0037); look parameters are data with physically based Earth defaults; the ocean is Gerstner waves only (FFT after the gate); GPU budgets: atmosphere <= 1 ms, ocean <= 2 ms, frame median <= 16.6 ms and p99 <= 20 ms unchanged; no egui in M5 (frame rate in the window title); per-experiment time boxes with a written fallback the coder may take without asking, reported afterwards. Discrete GPUs are the target (owner, 2026-10-16).

After the M5 gate (to be decided there): hydrology (lake levels, rivers, deltas, carving) together with surface materials (snow, sand, scree as climate- and slope-driven layers), sharing one material-transport engine written with a clean seam (what moves, what drives it). Not planned: runtime simulation of moving material; overhangs and caves (a heightfield cannot hold them).

## Queued for M5 (sonnet-explorer; started 2026-10-16, owner)
ADRs 0038-0042 are accepted. Briefing: `docs/briefings/sonnet-explorer-m5.md`. Every change opens its spec deltas with each attribute's class and bound (ADR 0035), and new goldens are blessed under policy (ADR 0037). Budgets: atmosphere <= 1 ms, water <= 2 ms, frame median <= 16.6 ms and p99 <= 20 ms at 1440p in a valid window. Time boxes and fallbacks as in the ADRs, taken without asking and reported. Perf windows: ask the planner, who authorizes them.
1. **viewer-window** (owner's first M5 item; ADR 0042 point 4, ADR 0015): `planet editor` opens a `winit` window. `render` owns the surface. The window runs the pipelined loop with the scripted orbit-to-1 m flight on the injected clock, on today's debug path. Keys: pause, restart, debug view, vsync on/off. The title shows fps, CPU frame time, GPU time and the readiness report (ADR 0045 point 5). No egui. The `--smoke` test opens and closes the window path, runs on Rasierklinge with `test gpu --real`, and is skipped with a reason where CI has no display. Remote desktop works. Time box two days. **Adapter gate (owner, 2026-10-16):** the app picks a discrete GPU and refuses an integrated one, or any adapter missing a required feature or limit, before creating a device, with a message naming the reason, never a crash. The test adapters (WARP, lavapipe) stay allowed in test and CI modes only. A tier-A test covers the selection logic on adapter descriptions.
2. **render-graph-hdr** (ADR 0042 points 1-3 and 5, ADR 0038): `render::graph` as product code with compile tests; the lit path with terrain into `Rgba16Float` with pre-exposure; the histogram exposure (dt from the `FramePlan`; fixed exposure for goldens); the ACES fit and the sRGB encode; the frame-target share asserted from the compiled graph (an amendment to ADR 0033's remainder). The debug path and its goldens are unchanged.
3. **look-file** (ADR 0042 point 6): a schema-versioned look file beside the world definition, outside its hash and every cache key, with tested migrations (CON-10), Earth defaults, and loading per body.
4. **sun-and-base-shading** (ADR 0039): the sun direction in the plan, the climate per tile as a field (9x9 f16, ADR 0027 reader, MESH_VERSION bump, ADR 0033 headroom re-pinned), and albedo from look data.
5. **atmosphere** (ADR 0040, accepted after SPIKE-09: the altitude-relative primitives at every altitude): static LUTs on the CPU in f64, sky-view and aerial-perspective on the GPU per frame, the orbit raymarch, the tier-B oracle, and a temporal-stability scenario at 1 m (ADR 0043). Time box four days; fallback Bruneton 2008, reported.
6. **ocean-bake-and-water-level** (ADR 0041, accepted after SPIKE-08: tiled union-find per bake block during the bake; area threshold as body data, Earth default 1e6 km²): the ocean-connectivity bake channel (`GLOBAL_VERSION` bump, known answers), the field W with body id and kind, liquid kinds as world data, and the synthetic-lake retrofit test.
7. **water-pass** (ADR 0041 points 4-6, 8 and 9): water on the terrain grid, Gerstner waves with phases reduced on the CPU, the shoreline from W - h_morph, and the coastal-flight flicker metric. Time box four days; fallback flat water, reported.
8. **m5-perf-scenes**: per-pass GPU timestamps and the M5 views at 1440p; the harness also logs system-wide CPU load and the top competing processes during each case (M4 window lessons). The M4 budgets are re-measured on the lit path, and the compact-versus-vertex GPU ratio is measured.
9. **tier-b-perf-smoke** (owner, 2026-10-16): the real streamer and pool on the software adapter with order-of-magnitude thresholds (tiles/s, scheduler and upload ms), run by coders, not a verdict; it catches an M4-style configuration cap without a perf window.
10. **M5 acceptance and gate**: `cargo xtask accept M5`, `docs/milestones/M5-report.md`, and contact sheets and flight videos for the owner's single look review (ADR 0037). Stop and report to the owner (milestone gate).
