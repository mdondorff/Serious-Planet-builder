# Spike register

Time-boxed experiments that answer a question before a decision. Opened and closed with `/spike`. Spike code lives on its spike branch and is never merged into the main crates as-is. **Each spike's report, numbers and renders live in `spikes/<name>/results/`** (`results.md` with a Visuals section; the `spike` skill has the rule). `docs/spikes/` holds only the frozen images of SPIKE-03 to 07.

Planned spikes from the background report still to open: city HLOD with 10^5 buildings (M7).

Closed spikes of earlier milestones: `docs/archive/M0-M3/spikes.md` (SPIKE-01 to 04), `docs/archive/M4/spikes.md` (SPIKE-05 to 07). SPIKE-10 was never used.

## Index of M5 spikes and explorations
| Id | Title | Status | Results |
|---|---|---|---|
| SPIKE-08 | Ocean connectivity at 16384 | closed 2026-10-16 | `spikes/ocean-connectivity/results/` |
| SPIKE-09 | Atmosphere f32 forms | closed | `spikes/atmosphere-f32/results/` |
| SPIKE-21 | The wet floor of ADR 0071: cost and memory of the three passes | open | `spikes/wet-floor/results/` |
| SPIKE-17 | Two bugs of the owner's descent video: the atmosphere top, the sea edge (diagnosis) | closed 2026-10-20 (causes: far terrain hazed differently inside and outside the top, fixed in #404; the 100 km reach of the coastal band, see SPIKE-20 results) | `spikes/descent-video-bugs/results/`, `spikes/owner-captures-2026-10-20/results/` |
| SPIKE-18 | The drawn tiles go coarser for a moment during a slow approach (diagnosis) | closed 2026-10-20 (cause: the cover splits a tile only if every in-view child is resident) | `spikes/tile-coarsening/results/` |
| SPIKE-19 | The window aborts on a refused morph interval (diagnosis) | closed 2026-10-20 (cause: the coarse LOD factor sits at the validate limit, the plan adds the height extent) | `spikes/morph-refusal/results/` |
| SPIKE-20 | The atmosphere still banded after the AP range fix (diagnosis) | closed 2026-10-20 (cause: the 32 x 32 x 32 volume is 21 to 23 % off between texels at 60 to 97.5 km) | `spikes/owner-captures-2026-10-20/results/` |
| SPIKE-22 | Is the base LOD refused in the real eroded world? (diagnosis) | closed 2026-10-20 (no: 0 refusals in 45,800 poses; the finding holds for fields with only a global bound) | `spikes/base-lod-refusal/results/` |
| SPIKE-16 | Dark patches over the sea while the camera moves (water-motion-gap; diagnosis) | closed 2026-10-19 (cause: the shore ramp depends on the drawn node's level) | `spikes/water-motion-gap/results/` |
| SPIKE-11 | Mouse look plumbing | closed 2026-10-18 (recommendation: raw events, Locked then Confined) | `spikes/mouse-look/results/` |
| SPIKE-15 | Edit-to-test build time | closed 2026-10-18 | `spikes/build-speed/results/` |
| SPIKE-14 | egui in the frame | closed 2026-10-18 | `spikes/egui/results/` |
| SPIKE-13 | Edit-to-visible latency | closed 2026-10-18 | `spikes/edit-latency/results/` |
| SPIKE-12 | Retiring the vertex tile format | closed 2026-10-17 (recommendation: retire, early M6; owner decides at the M5 gate) | `spikes/vertex-retirement/results/` |
| exploration | WARP parallel-device crash: inside WARP, test-only (PRs #195, #196) | closed | `spikes/warp-crash/results/` |
| exploration | WARP device removal with compact tiles and the atmosphere: test-only (PRs #212, #213) | closed | `spikes/warp-compact-atmosphere/results/` |
| exploration | Tile-format duality: cost of retiring the vertex format (PR #218) | closed; SPIKE-12 follows | `spikes/tile-format-duality/results/` |
| exploration | Compact tiles in storage buffers (PR #225; ADR 0054) | closed | `spikes/compact-storage-buffer/results/` |

The explorations before 2026-10-17 had no register entry; from now on every exploration gets a `SPIKE-NN` entry.

## SPIKE-21 The wet floor of ADR 0071: cost and memory of the three passes
Status: open
Opened: 2026-10-20
Question: Do the three passes of ADR 0071 point 3 (per-block union-find with border minimum-spanning edges, one global pass, per-block bucketed priority flood) fit the budget on the 256-cell and the 16384-cell bake: the time on 20 threads, the merger memory (edges per block against the 4,097 bound), the stored size with LZ4, and the equality with a serial priority flood?
Why it matters: ADR 0071 point 3 is provisional on it; if the memory or the time is out of reach the fallback is option B (one bit per vertex, keyed by the bound).
Time box: 1 day of agent work.
Success criteria: the three passes bit-equal to the serial flood on the 256-cell bake and on a 1024-cell bake at several block sizes and thread counts; measured wall time and peak memory of each pass at 16384 cells (a timing measurement, not `cargo xtask perf`); the edges per block at most 4,097; the stored kind-6 size.
Fallback if it fails: option B of ADR 0071 (a bit per vertex keyed by the bound).
Code location: branch `spike/wet-floor` (results in spikes/wet-floor/results/); the production code is the change that implements ADR 0071.
Result: (open)

## SPIKE-16 Dark patches over the sea while the camera moves (water-motion-gap)
Status: closed
Opened: 2026-10-19
Closed: 2026-10-19
Question: Why do dark patches with triangle edges appear on the sea near the camera at about 80 m over a shallow coast while the camera moves or rotates (owner, 2026-10-18)? Diagnosis only.
Why it matters: the M5 gate's look review; the bug sits in the lit path (water pass or terrain shading) and blocked #304.
Time box: about 4 hours of agent work.
Success criteria: a scripted repro without the owner, the root cause with plan and camera facts and node counts, a proposed fix and the test that would catch it.
Fallback if it fails: report what is known.
Code location: spikes/water-motion-gap/ (results only; the diagnostic code is on branch `spike/water-motion-gap`, never merged).
Result: reproduced with an 80 m coast path at 200 m/s in the pipelined lit loop (2,532 frames): the water is not missing (0 gap nodes, the water class view is complete, the drawn plan's camera equals the request's in every frame); the dark patches are terrain albedo: `shore_ramp_min_m(level)` makes a seabed point 2.5 m under the sea 4.9 times brighter under L17 tiles than under L19 and finer; still frames draw L16 to L19 near tiles, cover plans (moving, rotating) L19 to L20. Clamping the ramp's level at 18 removes the patch. See `spikes/water-motion-gap/results/results.md`.
Recommendation: make the albedo ramp a function of the fragment's camera distance and the screen-space tolerance (a world-space quantity), not of the drawn node's level; test: the same seabed point gives the same albedo under adjacent levels; goldens through `golden-review`.

## SPIKE-15 Edit-to-test build time
Status: closed
Opened: 2026-10-18
Closed: 2026-10-18
Question: Where do the 50-100 s of build after a one-line edit go, and which of a faster linker, fewer test binaries and test-profile tuning bring the edit to the first test result to 20-30 s?
Why it matters: the owner's development speed; BUILD-003 measures only the test half of `test fast`.
Time box: 1 day.
Success criteria: breakdown per edit; median of 3 with the load for each step; recommendation with the expected time.
Fallback if it fails: a development-only dynamic library of wgpu and winit.
Code location: branch `spike/spike-15-build-speed` (never merged); results `spikes/build-speed/results/`.
Result (Rasierklinge, median of 3, build only, cumulative steps):
| Configuration | frame edit | generators edit | app edit |
|---|---|---|---|
| Baseline | 88.8 s | 120.6 s | 31.4 s |
| `rust-lld` | 78.6 s | 96.9 s | 15.8 s |
| plus one test binary per tier (frame, render) | 41.6 s | 60.5 s | 18.2 s |
| plus `debug = "line-tables-only"`, dependencies without debug info | 23.2 s | 41.7 s | 15.2 s |
| (instead) workspace `opt-level = 0` | 32.1 s | 49.0 s | 33.0 s; tests 28.4 s against 21.2 s |
Recommendation: apply the debug-info and `rust-lld` changes, then per-tier test binaries for every crate with more than 5 test files (needs the nextest group and xtask guard rework); skip `opt-level = 0` and the dynamic library.

## SPIKE-14 egui in the frame
Status: closed
Opened: 2026-10-18
Closed: 2026-10-18
Question: Do egui and egui-wgpu fit wgpu 30.0.1 and winit 0.30.13, does a panel after the tone map work on the RTX and WARP at two DPI scales at a carryable cost, and which input rule separates panel and camera?
Why it matters: M6 editor UI toolkit (architecture report proposes egui); ADR 0015 (painter in render), ADR 0042 (UI node after the tone map).
Time box: 1 day.
Success criteria: version pair and build result; panel as one graph node over the lit viewer; RTX and WARP; DPI 100 % and 150 %; GPU and CPU cost per frame; input routing checks; recommendation.
Fallback if it fails: nearest compatible pair, an upgrade or a fork.
Code location: branch `spike/spike-14-egui` (never merged); results `spikes/egui/results/`.
Result (RTX, indicative): all numbers in `results.md`.
| Item | Result |
|---|---|
| Version pair | egui, egui-wgpu, egui-winit 0.36.2 and 0.37.0 build against wgpu 30.0.1 and winit 0.30.13; 0.35 and older need wgpu 29; no upgrade or fork |
| New crates | 33 (0.36.2), 36 (0.37.0); `cargo xtask layering` ok |
| Licences | `default_fonts` brings OFL-1.1 and Ubuntu-font-1.0, not on the `deny.toml` allow-list; without it labels are blank |
| Graph | node `ui` after `tonemap`; no frame-target byte or lifetime changes (test) |
| Cost, 1080p, typical panel, RTX | egui 0.175 ms CPU, UI pass 0.12 ms; window 48 of 48 frames presented, 0.53 ms CPU per frame |
| WARP | UI pass 17-23 ms (software rasteriser), egui 0.16 ms CPU |
| Outside the panel | 0 pixels differ from the blit, both adapters, both DPI scales |
| Routing | press on the panel goes to the UI; a drag started in the world stays with the camera across the panel; keys go to a focused text field |
Recommendation: egui `=0.36.2` for the M6 editor UI; painter in `render`, `ui` node after `tonemap`, editor builds the GPU-free frame; pointer latched at press. Owner decides the font licences (add OFL-1.1 and Ubuntu-font-1.0 to the allow-list, or ship a font under an allowed licence).

## SPIKE-13 Edit-to-visible latency
Status: closed
Opened: 2026-10-18
Closed: 2026-10-18
Question: How long from editing an authored item to the first frame showing the new terrain at full resolution near the camera, per stage, and what must M6 build to reach a realistic target?
Why it matters: M6 risk "editor on a streamed world" (target 1 s at nearby LODs, architecture report); ADR 0021 points 2, 5, 8.
Time box: 1 day.
Success criteria: latency table (median, worst) per altitude, stamp size and camera; stage breakdown; the floor; a preview latency and fidelity; targets and the M6 build list.
Fallback if it fails: none.
Code location: branch `spike/spike-13-edit-latency` (never merged); results `spikes/edit-latency/results/`.
Result (RTX, indicative, host load per row): all numbers in `results.md`.
| Case | Today's path | Floor measured |
|---|---|---|
| Exact, 200 m altitude, 50 m stamp, still | 1,790 ms | 144 ms |
| Exact, median over 3 radii: 2 km / 200 m / 1 m | 404-492 / 919-949 / 1,435-1,478 ms (reach, find by key) | 63-72 / 443 / 773-831 ms (reach, find by footprint, undisturbed) |
| Preview (vertex-stage displacement) | - | 4-14 ms |
| Find stage | 260-820 ms by key; 34-91 ms by cell grid | 0.6-3.7 ms by footprint |
| Tiles re-keyed by one 150 m stamp (cell invalidation) | 3,788 of 6,445 (2 km); 12,110 of 14,543 (1 m) | 36-2,812 touch the stamp |
Recommendation: preview at most 16 ms, exact at most 300 ms for stamps up to 150 m and at most 1 s up to 500 m; M6 builds item reach, key-aware refresh with stale-while-revalidate, an edit event (plan keys, cached volumes, forced plan), the preview displacement and a request class for edited tiles; the owner decides nothing now (ADR 0021 points 2, 5, 8 before the first authored stage).

## SPIKE-12 Retiring the vertex tile format
Status: closed
Opened: 2026-10-17
Closed: 2026-10-17
Question: Can the compact tile format draw levels 0-2 and the CON-13 cliff tiles, so that the vertex format retires from rendering, and what does that cost in GPU time, bytes, code, tests and goldens?
Why it matters: the M5 gate decision "retire the vertex tile format from rendering, or keep both" (`docs/status.md`, item 7c and the gate item); the duality exploration (PR #218) estimated 5-7 coder days with two unprototyped parts.
Time box: 1.5 days of agent work.
Success criteria: for levels 0-2 a 5x5 base surface (error per level in pixels) and a level-3 render floor (draws and slots at the ten standard views, indicative RTX time at orbit and aerial-200km), each with image differences against the current vertex path on WARP and the RTX; for the cliff tiles a 32-bit height with 15-bit normals against CON-13 and CON-14 on the steep field, GPU time against the 8 B texel, bytes per slot; a concrete slot layout against ADR 0033's shares; line, test and golden counts; an ordered plan with a firmer estimate and a recommendation.
Fallback if it fails: keep both formats (the status quo).
Code location: spikes/vertex-retirement/ (results only); prototype on branch `spike/spike-12-vertex-retirement`, cut from `explore/compact-storage-buffer`, never merged (switches in `generators::compact`, a level floor in `frame::lod`, `render/src/compact_v2.rs`, experiments in `crates/render/tests/spike12.rs`).
Result (2026-10-17, sonnet-spikes; report `spikes/vertex-retirement/results/results.md`, tables and images next to it). Levels 0-2: the 3x3 base errs 0.46 px (worst, 0.25 mean) on the 52 level-2 nodes of the orbit view at 2560 x 1440, a 5x5 base 0.0018 px (level 0: 0.35 px, level 1: 0.04 px, SPIKE-06's 190-pixel tile), but the 5x5 base costs vertex-stage time on every tile: +9.8 % at aerial-200km, +47.6 % at ground-1m (2.74 against 1.86 ms), +35.6 % on the steep field, +192 B per record, and it moves every terrain view by rounding noise (0.3 to 6 % of pixels). A render floor at level 3 changes only the orbit view (52 level-2 nodes become 181 level-3 nodes; 11,713 to 11,842 nodes over the ten views; the other nine plans are identical, 0 pixels differ): GPU time 0.174 to 0.321 ms at orbit, unchanged at aerial-200km, no bytes per slot; a floor leaf must be root-like (no morph, exempt from the empty-interval check). Cliff tiles: a 32-bit height with two 15-bit normal components keeps the 8 B texel, holds CON-13 (step 0.0004 mm against 28.6 mm for 16 bits at 1.9 km relief) and CON-14 (worst 0.0072 degrees against 0.01) on the steep field, and costs the same GPU time as the 16-bit texel (ratios 1.001 to 1.005 at ground-1m, aerial-200km and the steep field; per-slot dual decoders 1.003 to 1.015); one layout for all tiles. Slot: the climate leaves the spare texel channel for a 324 B block; slot 9,360 B (+5.9 %); with the record still in ADR 0033's 128 MiB reserve the mid share would fall from 230,640 to 123,008 slots, charged per slot it is 215,085 (-6.7 %) and 329,797 at the high share. Counts: about 950 production lines removed and 200 added (net about -750), about 40 of 805 tests touched, a test-only vertex renderer should stay as the independent image reference, all terrain goldens at rounding noise (the two orbit goldens by declaration), `MESH_VERSION` 7 to 8. Plan: four small changes (slot v2 and pool arithmetic 3.5 days, floor 1.5, removal 2) plus goldens and one perf window 1 day: about 8 coder days (range 7 to 9). GPU numbers are indicative (RTX 3080 Ti Laptop, 2026-10-17 about 11:31, sonnet-explorer confirmed the GPU free, no perf window).
Recommendation: retire the vertex format from rendering with the level-3 floor and the wide texel for all tiles, in early M6 (not before item 8's perf window); do not adopt the 5x5 base. The owner decides retire or keep, the visible orbit change and the 5.9 % larger slot at the gate; the architect writes the ADR 0033 amendment.

## SPIKE-11 Mouse look plumbing
Status: closed
Opened: 2026-10-16
Question: Which mouse input path gives usable mouse look on Windows with `winit` (raw `DeviceEvent::MouseMotion` against `CursorMoved` with recentring), which cursor grab works (`Confined` plus hidden cursor plus recentring, since `Locked` is unsupported there), what happens at focus loss and alt-tab, with DPI scaling and high-poll-rate mice, and can sensitivity be independent of the frame rate?
Why it matters: M5 item 1b (free-camera-mouse) builds mouse look on the pose function as this spike recommends; an earlier prototype in another app failed at mouse look (owner).
Time box: one day of agent work, plus about ten minutes of the owner's manual check.
Success criteria: a recommendation (event source, grab mode, fallback) from the owner's local check; a recorded input-trace format with traces that replay to known final poses.
Fallback if it fails: keyboard look only (item 1b's fallback), reported.
Code location: spikes/mouse-look/ (throwaway; branch `spike/spike-11-mouse-look`, never merged). Run from that folder: `target/release/mouse-look.exe`; checklist `CHECKLIST.md`; traces land in `target/traces/`; `mouse-look replay FILE` replays a trace.
Interim result (2026-10-17): built and unit-tested (13 tests): the draft pose function (yaw and pitch relative to the local vertical, heading by parallel transport along the great circle, pitch clamped at 89 degrees; no roll after 20,000 random inputs, reversible, split-invariant, continuous across the pole), the input adapter (raw, cursor and auto sources, lazy recentring with echo handling, release on focus loss, mouse deltas never multiplied by dt: turn is counts x sensitivity at 1, 7, 60 and 500 frames), and the trace format `spike11-trace-v1` (JSON Lines, documented in `src/trace.rs`) with a headless replay that shares the live window's code and reproduces a recorded final pose bit for bit. The owner's remote session (Android touch screen over Chrome Remote Desktop, no mouse) did not capture the mouse: no button event reached the window; 402 raw motion events did arrive. Out of scope by the owner's decision. The same trace was checked for keyboard movement against heading and pitch: 4,542 movement frames, 0 sign disagreements; what can look wrong is documented in `spikes/mouse-look/results/remote-session-analysis.md` (W follows the horizontal heading, no ground clamp so Q goes below the surface, speed scales with altitude on a 1 km planet). Still open: the local check (raw against cursor against auto, grab modes, focus loss, DPI, high-rate mice) and the recommendation.
Closed: 2026-10-18
Result: the owner's local check (RTX, Windows 11, scale 1.0): raw mouse motion works and feels right, independent of frame rate, no problem with fast spins, focus loss or poles; cursor mode with recentring does not work (86 % of recentres failed in one session, 14 cursor events against 938 raw while captured); `Locked` was accepted, so the Confined fallback was not needed here; 1000 Hz+ mice and DPI scaling not tested. See `spikes/mouse-look/results/results.md`.
Recommendation: raw `DeviceEvent::MouseMotion` as counts x sensitivity (never times dt); request `Locked`, fall back to `Confined`, cursor hidden, release on Esc and focus loss; no recentring; the spike's pose function plus the altitude clamp and speed scaling of the remote-session analysis; arrow-key look as fallback. For M5 item 1b / the editor camera.


## SPIKE-08 Ocean connectivity at 16384
Status: closed
Opened: 2026-10-16
Closed: 2026-10-16
Question: What do the ocean components of the stored 16384-cell bake cost to compute (time, memory), do a serial explicit-queue fill and a tiled union-find with min-id roots give bit-identical results, how does a dry basin one cell from the ocean classify, and what area threshold separates ocean from dry basin?
Why it matters: ADR 0041 point 2 (acceptance of the water ADR; the new bake channel, `GLOBAL_VERSION` bump, known answers); owner's M5 constraint 2 (a low area is sea only if it connects to the ocean).
Time box: 1 day of agent work.
Success criteria: components of the sea (`continent <= SEA_LEVEL_F32`, 4-connectivity, shared face-edge vertices as single nodes) over the stored 16384 bake; time and peak memory per method; serial and tiled results equal by hash for several tile sizes and thread counts; a dry basin one land vertex from the ocean tested; a proposed area threshold with the data behind it.
Fallback if it fails: label connectivity at a coarser resolution (the water field only needs the ocean flag), not needed.
Code location: spikes/ocean-connectivity/ (throwaway; branch `spike/spike-08-ocean-connectivity`, never merged; reads the store's bake blocks only, never the tile records).
Result (2026-10-16, sonnet-spikes; tables and logs in `spikes/ocean-connectivity/results/`: `run-16384.txt`, `seeds-2048.txt`, `selftest.txt`). Input: the 16384 bake of `cargo xtask accept` (`sonnet-explorer/target/accept/global16384`, read-only, world seed 1), 1,536 blocks of 1025^2 samples. The sea mask (one bit per raw vertex, 6 x 16385^2 = 201 MB) was extracted once into the spike's own `target/`: 98 s on 8 threads, peak heap 467 MB, 70.71 % of the 1.61 G raw vertices are sea; it is decode-bound (about 0.5 s of thread time per block). Graph: raw vertex id = face x 16385^2 + j x 16385 + i; the 196,604 face-edge groups (12 edges x 16383 plus 8 corner triples) are found from exact integer cube points and are single nodes (all copies agree on sea: 0 disagreements); a component's label is its smallest raw vertex id; "nodes" counts a shared node once; area is the dual cell in fixed point (1e-5 km^2, the six faces sum to 5.100645e8 km^2 = 4 pi R^2, exact to the printed digits), so serial and tiled sums are integers and compare exactly.
- Components. The sea has only 5 connected components: the ocean (1,137,983,017 nodes, 3.5725e8 km^2, 99.93 % of the sea) and four enclosed basins below sea level of 144,702, 60,841, 27,065 and 11,816 km^2 (527,042 / 164,653 / 78,190 / 32,840 nodes). There are no small components at all: the continentalness is smooth at 0.6 km, so no single-vertex ponds appear. The table hash is `a94f3bddfce28693`.
- Serial explicit-queue fill (frontier vectors, a visited bitset): 39.6 s (100 s on a first, cold-cache run), extra peak heap 202 MB (the visited bitset) on top of the 201 MB mask.
- Tiled union-find (local scanline union-find per tile with min-index roots, a global union-find over the tile components that touch tile borders and over the shared nodes, min-id roots): bit-identical table hash to the serial fill in every configuration: tile 1024 with 20 / 7 / 1 threads (4.1 s / 7.1 s / 31.9 s; extra peak heap 195 / 86 / 36 MB), tile 256 with 20 threads (6.1 s, 119 MB), tile 1000 with 3 threads (35.4 s, 51 MB, the last tile row narrower), tile 4096 with 20 threads (5.7 s, 2,691 MB: tile arrays scale with the tile and the thread count). The merge itself takes under 0.1 s. A tile of 1024 matches the bake block, so the tile pass can run while a block is in memory: its cost is about 0.02 s of thread time per block (the 1-thread run, 31.9 s over 1,536 blocks), the same as a block's decode (about 20 ms, `docs/learnings.md`).
- Ocean bitmap (a second tile pass that labels again, looks up the global component and sets the bit; 1 bit per vertex, 201 MB): 10.8 s on 20 threads, extra peak heap 369 MB; hash `af4fddc92724bd3f` at the proposed threshold, identical to a serial flood of the components at or above the threshold (18.7 s, 2,349 MB for its explicit stack).
- Correctness beyond the one bake: a self-test of random site-percolation masks (res 37 to 128, 0.5 to 0.7 density, shared nodes forced equal) gives serial == tiled for five tile/thread combinations each, components and ocean bitmap.
- Dry basin one cell from the ocean. The stored bake has none: the nearest enclosed basin is 55 land vertices from the ocean (about 33 km) along an axis walk; two have no ocean within 256 vertices. A hand-drawn fixture reproduces the case (11-vertex-tall basin behind a wall of land one vertex wide, plus a far pond): the basin is a separate component, falls below the threshold, is not ocean in the bitmap; serial and tiled agree; the scan finds the 11 one-vertex-wall pairs. Expected look: the basin stays dry, its floor below sea level visible as low land, separated from the sea by a ridge about 0.6 km wide; the water pass must read the ocean flag and never the continentalness alone, or it floods the basin.
- Area threshold. Ocean and enclosed-basin areas by seed at 2048 cells (in-memory bakes, 10 seeds; the stored world's seed at 2048 reproduces the 16384 areas within 0.4 %: 144,702 / 60,872 / 27,049 / 11,767 km^2, so the area is resolution-stable): enclosed sea areas range from 4.9e3 to 4.1e6 km^2; the smallest large water body that is plausibly an ocean is 5.0e6 km^2 (the 5th component of a 29 %-sea seed, with ocean basins of 5.5e7, 3.4e7, 3.1e7, 2.3e7 and 5.0e6); no component is under 100 km^2. There is no clean gap (4.07e6 km^2 against 5.03e6 across seeds), so the threshold is a design choice: 1e6 km^2 (0.2 % of the surface; Caspian and Black Sea sized basins, 3.7e5 and 4.3e5 km^2, stay dry; Mediterranean-sized and larger count as sea). On the stored bake, any threshold from 1.5e5 up to 3.5e8 km^2 gives the same ocean (the main component alone).
Recommendation: adopt ADR 0041 point 2 as written with these decisions: components by tiled union-find with min-id roots, tile = bake block (1024 vertices), shared face-edge vertices as single nodes (integer cube points), exact integer areas in fixed point so the table is independent of tile size and thread count (proved above by hash); the ocean channel is a bitmap of the same shape as the bake (1 bit per vertex; 201 MB raw, runs of equal bits compress to far less, not measured) computed in the bake after the blocks, at a cost of about 5 s on 20 threads plus the block reads (or about 0.02 s of thread time per block when done while the block is in memory) and 200 to 370 MB of extra memory; the area threshold is body data, Earth default 1e6 km^2 (stated as 0.2 % of the surface area so other radii scale), and is part of `GLOBAL_VERSION`'s contract. Known answers for the bump: table hash `a94f3bddfce28693` (5 components, labels 0, 11243, 387166641, 1482240544, 1490126660), ocean bitmap hash `af4fddc92724bd3f` at 1e6 km^2, valid for the stored seed-1 bake. Limits: one world seed at 16384; coarser seeds at 2048 only count components; the gap scan walks axes inside tiles only; the planner's perf windows were not involved (CPU only, 20 threads, while sonnet-explorer was working: timings carry that noise, the serial fill took 40 to 100 s). Needs nothing from the owner (the threshold is a data value he can change).

## SPIKE-09 Atmosphere f32 forms
Status: closed
Opened: 2026-10-16
Closed: 2026-10-16
Question: Does the naive f32 form of Hillaire's ray geometry (r, r^2 - R^2, ray-sphere roots, sample altitude) meet the bound of ADR 0040 point 4 (1 % relative plus 1e-4 absolute) including temporal stability in a slow walk at 1 m altitude, or do the altitude-relative primitives of ADR 0040 point 5 have to be used?
Why it matters: ADR 0040 point 6 (acceptance of the atmosphere ADR), ADR 0043 (CON-11 v1.4), the M5 atmosphere pass design.
Time box: 1 day of agent work.
Success criteria: per form (naive, primitives) and adapter (WARP, RTX, llvmpipe if available): worst error against an f64 oracle of a single-scattering ray march and of its geometry at h = 1 m, 2 km, 20 km, 20,000 km; mu = -1, the horizon +-1e-6, +1; a slow walk at 1 m altitude with the count of frame-to-frame steps beyond the bound; a verdict naming the simplest passing form.
Fallback if it fails: the primitives of ADR 0040 point 5 are the form (they are the fallback by design).
Code location: spikes/atmosphere-f32/ (throwaway; branch `spike/spike-09-atmosphere-f32`, never merged).
Result (2026-10-16, sonnet-spikes; full tables in `spikes/atmosphere-f32/results/{rtx-vulkan,rtx-dx12,warp-dx12}.md`). Method: a compute shader marches a compact single-scattering sky ray (24 view samples, 6 sun samples, Rayleigh and Mie, ground albedo 0.3, no ozone, no multiple scattering; sun illuminance 1) in two geometry forms: naive f32 (r, r^2 - R^2, ray-sphere roots, `r_s = sqrt(...)`, `h_s = r_s - R`) and the primitives of ADR 0040 point 5 (`c = h(2R + h)`, `t = c/(-r mu + sqrt(D))`, `c_top`, `h_s = (c + s(s + 2 r mu))/(r_s + R)`, with the entry from outside by the same stable root). An f64 oracle runs the same algorithm and sample placement at the exact altitude and the f32-rounded mu, sun cosines. Static grid: h = 1 m, 2 km, 20 km, 20,000 km, each at 16 altitudes up to 1.94 h (so r rounds differently), 13 to 17 mu values (-1, -0.5, -0.1, -0.01, horizon +-1e-4 and +-1e-6, +-1e-6 around 0, 0.01, 0.5, +1, and at orbit the top limb +-1e-4, +-1e-6), 3 suns (noon-ish 0.8, low 0.05, twilight -0.08), 3 azimuths: 8,064 rays. Bound: |L32 - L64| <= 1 % of L64 + 1e-4 per channel (ADR 0040 point 4). Walk: the camera rises from 1.000 to 2.000 m in 1 mm steps (1,001 frames) for 26 series (13 mu at fixed view direction, 2 suns); a step is a frame-to-frame change whose difference from the oracle's change exceeds the bound. Adapters: RTX 3080 Ti Laptop on Vulkan and on DX12, and WARP (Microsoft Basic Render Driver, DX12); llvmpipe does not exist on this Windows machine (not run; CI Linux was not used); the Intel Iris Xe (DX12) adapter crashed the harness with a segmentation fault at device or pipeline creation (not investigated). The three adapters agree on the conclusion; WARP is worse than the RTX for the naive form at 2 km (below).
- Naive f32 fails at 1 m. RTX: 196 of 1,872 rays (10.5 %) exceed the bound, worst 1,286 times the bound, 153 rays on the wrong side of the ground/sky decision (WARP: 177 and 144). The failures are in a band around the geometric horizon, which at 1 m is mu = -5.6e-4: it reaches 5.6e-3 rad below the horizon (0.3 degrees, about 8 pixels at 1440p and 60 degrees) and 3.2e-5 above it; at 10 m 1.8e-3 below and 1e-5 above; at 100 m 1e-5 and 3e-6. Away from the horizon the naive radiance passes (worst 0.07 of the bound for |mu| >= 0.01 and for horizon +1e-4), but only because of the 1e-4 absolute term: the distance t to the ground is 20 % to 32 % wrong at every downward angle at 1 m (mu = -1 to -0.01), which the aerial-perspective volume (distance slices) would show.
- 2 km: RTX 0 failures (worst 0.30 of the bound), WARP 56 of 1,872 (3.0 %), worst 26 times the bound and 72 ground/sky misclassifications within 1e-6 rad above the horizon (the "harmless from 2 km up" of ADR 0040 does not hold on WARP, which tier B runs on). 20 km: no failures (worst 0.23 RTX, 0.35 WARP). 20,000 km: none (0.15 to 0.23).
- Walk at 1 m, naive: six of 26 series (horizon -1e-4, -1e-6 and +1e-6, each at 2 suns) have 247 to 495 of 1,001 frames beyond the bound and 1 to 3 frame-to-frame steps beyond it, the largest 58 to 78 times the bound (the fixed view direction crosses the moving horizon as the camera rises: the horizon moves from mu = -5.6e-4 to -7.9e-4 between 1 and 2 m). The other 20 series stay under 0.07 of the bound per frame step.
- Primitives: no ray beyond the bound on any adapter: worst error 0.000 of the bound at 1 m (t_end relative error 9e-7), 0.001 at 2 km, 0.004 at 20 km, 0.83 (RTX Vulkan), 0.84 (RTX DX12) and 0.19 (WARP) at 20,000 km; no ground/sky misclassification; no frame beyond the bound and no step in any of the 26 walk series (max step 0.000 of the bound). The 20,000 km figure is the planet limb +-1e-6 rad seen from orbit, where the discriminant `(r mu)^2 - c` cancels in f32 (t_end relative error 5.3e-5, the same in both forms); it passes but with less margin than everything else.
- Limits of this evidence: a compact single-scattering march stands in for the sky-view and aerial-perspective passes (no multiple-scattering LUT, no ozone, no transmittance LUT lookups); the oracle shares the algorithm and sample placement, so the numbers measure geometry and arithmetic, not discretisation; the walk moves in altitude only at fixed view directions.
Recommendation: use the altitude-relative primitives of ADR 0040 point 5, at every altitude, as the one form (a handful of extra multiplications; no altitude gating, since the naive form also fails at 100 m and, on WARP, at 2 km). The CPU passes h, c, c_top and r computed in f64. For the orbit path (outside the atmosphere) take the same primitives for the entry root and put the 20,000 km limb +-1e-6 case into the tier-B bound test: it passes at 0.83 of the bound on the RTX, so a limb-stable discriminant `r^2 (mu - mu_h)(mu + mu_h)` with `mu_h` from f64 is the first thing to try if that case drifts in the real passes. ADR 0040 can be accepted with point 5 as the chosen form and the "harmless from 2 km up" remark corrected (sub-pixel but real at the horizon at 2 km, and 3 % of 2 km rays on WARP). Needs nothing from the owner.

