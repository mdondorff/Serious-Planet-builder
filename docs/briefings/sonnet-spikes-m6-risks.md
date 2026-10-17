# Briefing for sonnet-spikes: SPIKE-13 edit-to-visible latency, SPIKE-14 egui in the frame (opus-planner, 2026-10-17)

The owner wants M6's two main risks measured now, while M5 finishes. Message `opus-planner` with results and questions.

## Read first
1. `docs/constitution.md`, `CLAUDE.md` (including the "Writing" section, ADR 0055).
2. The `spike` skill: the parallel-session rules, the worktree check, and the output rule (`spikes/<name>/results/results.md` with a Visuals section).
3. ADR 0007 (authored layer data model), ADR 0021 (authored-item index), ADR 0031 (residency and request priorities), ADR 0042 (frame structure and viewer window).
4. `docs/background/architecture-report.md`: the M6 row, and the risk row "Editor on a streamed world" (target: edit a stamp → regenerated tiles visible ≤ 1 s at nearby LODs).

## Constraints
- Your worktree only; one branch per spike (`spike/spike-13-edit-latency`, `spike/spike-14-egui`), never merged; no production code.
- sonnet-explorer is fixing the M5 water pass. Before any RTX run, ask it whether the GPU is free, and tell it when you're done. Never run `cargo xtask perf`. Stop all runs if opus-planner announces a perf window.
- Timings are indicative: record the time and the host load (the harness's sampler, or the system CPU share) next to every number.
- Cap your builds at half the cores (`CARGO_BUILD_JOBS=10`), because sonnet-explorer builds at the same time.
- Results: terse, numbers in tables (ADR 0055).

## SPIKE-13 Edit-to-visible latency (first; time box 1 day)
The owner thinks 1 s is already the far end of acceptable. Measure where we stand and what the floor is.
1. **Prototype:** a test-only authored item, for example a height stamp (a raised disc or a crater, a few hundred metres wide), entered through the existing in-memory cell index (`crates/world-def/src/cells.rs`). It invalidates the tiles whose read sets contain the changed cells, and they regenerate through the real streamer and pool on the RTX.
2. **Measure** the time from the edit to the first frame showing the new terrain at full resolution near the camera. Three altitudes (1 m, 200 m, 2 km above the stamp), three stamp sizes, and the camera both still and moving. Break it down per stage:
   - finding the affected tiles;
   - queueing;
   - generation per tile and in total;
   - mesh or compact build;
   - upload;
   - the frame;
   - the number of tiles per level.
3. **The floor:** what the same path costs when edited tiles jump the request queue (ADR 0031 classes), are generated in parallel, and are drawn coarse first.
4. **Immediate feedback:** prototype a preview that shows the edit within one or two frames, for example the stamp applied as a GPU-side displacement on the resident tiles until the exact tiles arrive. Measure its latency and how far it lands from the exact result.
5. **Answer:** a latency table (median and worst) per case, the stage that dominates, and a recommendation: which target is realistic (for example preview ≤ 50 ms, exact ≤ 300 ms), and what M6 must build to reach it.

## SPIKE-14 egui in the frame (second; time box 1 day)
1. **Fit:** do `egui` and `egui-wgpu` versions exist that match our wgpu 30.0.1 and winit 0.30.13? If not: the cost and risk of the nearest compatible pair, an upgrade or a fork, measured by build errors.
2. **Prototype:** an egui panel drawn after tonemapping, into the presented LDR frame, over the lit viewer. It is one extra node of the render graph (ADR 0042), and the HDR targets stay untouched.
3. **Checks:**
   - it works on the RTX and on WARP;
   - DPI scaling at 100 % and 150 %;
   - GPU and CPU cost per frame with a typical panel (a few sliders, a list of 100 items);
   - input routing: a click on the panel never moves the camera, and mouse look (SPIKE-11's draft) works when the pointer is outside the panel.
4. **Answer:** a recommendation for M6's editor UI toolkit (egui as the architecture report proposes, or something else), with the version pair, the graph integration and the input-routing rule.

## When you finish each spike
One docs-only PR (entry, results with Visuals, learnings), then a message to opus-planner: PR number, the key numbers, the recommendation, and anything the owner must decide. SPIKE-11 stays open, waiting for the owner's local mouse check.
