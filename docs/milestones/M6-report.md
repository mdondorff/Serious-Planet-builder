# M6 Editor MVP: report (DRAFT, 2026-10-21)

**DRAFT pending the owner's retest of the P1 fixes.** Drafted by sonnet-explorer for opus-planner from `docs/briefings/m6-gate-evidence.md` (sonnet-crusader, 2026-10-20), `docs/briefings/m6-owner-test-plan.md`, `docs/status.md`, the log fragments of 2026-10-19 to 2026-10-21 and the perf windows 18 to 21. Nothing is archived. "Not checked" marks a figure this draft did not re-derive. The verdict, the owner's decisions and the process review are the planner's.

## Verdict (opus-planner): pending one condition

**Owner, 2026-10-21:** M6 closes without the owner's retest once the ADR 0074 plan-job cost is back within +10 % (window 22 measured +125 %). The exit criteria are met (windows 20 and 21, the undo and merge tests), the owner's hands-on review took place on 2026-10-20, and its P1 findings are fixed and verified by tests and the item sweep (SPIKE-26, 0 refused stops with #505).

State at drafting:

- Every automated exit-criterion row has evidence (section 1); the edit-latency budgets are met: the stamp cases in windows 19 and 20, the brush cases in window 20 only (window 19: the 150 m brush at 200 m was over, 383.8 ms against 300 ms) (section 2).
- The owner tested the editor on 2026-10-20 and filed P1 (stability) and P2 items (section 4). P1-1 (plan refusals near stamps), P1-2 (smooth sharpens) and P1-3 (editor toggle and capture) are merged; P1-4 (tiles flash to a coarser level) is not reproduced in scripted flights (SPIKE-18, SPIKE-25) and its diagnosis is open; P1-5 (frame rate while a coder was building) waits for a retest on a quiet machine.
- Three owner decisions are open (section 5): keep-inside regions, the look review of the editor (tier D), the regional-scale budget.

## 1. Exit criteria

Architecture report M6 row (`docs/briefings/m6-editor-plan.md`).

| Criterion | Result | Evidence |
|---|---|---|
| Edit visible: preview <= 16 ms | PASS | window 18: 2.7 to 6.7 ms in 9 stamp cases; window 19: 2.2 to 7.1 ms in every previewed case; window 20: brush rows 8.5 and 10.3 ms at 200 m altitude. Tests: `frame/tests/preview.rs`, `frame/tests/managed.rs`, `render/tests/gpu_preview.rs` (REND-033) |
| Edit visible: drawn exact <= 300 ms (stamp up to 150 m) and <= 1 s (up to 500 m, or at 1 m altitude) | PASS in window 19 (stamps) and window 20 (brush) | the former failing case (200 m altitude, 150 m stamp) is 125.3 ms (window 18: 508 to 544 ms); 1 m altitude, 50 m: 239 ms (was 953 to 996 ms); the 150 m brush at 200 m is 286.1 ms (window 19: 383.8 ms, before the brush footprint named only the tiles along its strokes, #474); the 1 km brush at 200 m is 402.8 ms against 1,000 ms. "Exact" means the drawn tiles (ADR 0072). Tests: STRM-031, REND-032 |
| Undo and redo over 1,000 operations | PASS | `editor/tests/commands.rs` (AUTH-004): 1,000 commands undo to the exact starting hash and redo, also through written-through files |
| Item files merge cleanly in git | PASS (tier A) | `editor/tests/commands.rs::two_additions_made_on_two_branches_merge_in_git_without_a_conflict_and_load_as_one_definition` (AUTH-001); fixed key order and a bit-exact round trip (`world-def/tests/item_file.rs`). Not tested: two sessions adding strokes to one brush item |

### Editor MVP rows (`docs/status.md`)

| Row | Result | Evidence |
|---|---|---|
| Stamps (crater, hill, plateau), handles, typed values | PASS | WDEF-010, GEN-026, VIEW-017, VIEW-019; `generators/tests/stamp_stage.rs`, `render/tests/gpu_stamps.rs`, `editor/tests/model.rs`, `interaction.rs` |
| Lines (ridge, channel), one item per line, split into linked items | PASS | WDEF-011, GEN-028, VIEW-027 to VIEW-033, AUTH-005 schema 2 |
| Regions: mask, surface type, name | PASS with a wording fix | WDEF-012, GEN-029, VIEW-030, VIEW-033; the mask mode 2 cuts the layer's edits inside the outline (ADR 0069). The owner read "clip inside" as "edits only inside": relabelled (P2-1, section 4); keep-inside (mode 1) is deferred and waits for the owner |
| Brush: raise, lower, smooth, flatten, after stamps, metric radius and strength | PASS | WDEF-013, GEN-030, VIEW-035, VIEW-036; a brush over a stamp: `generators/tests/brush_stage.rs`. Smooth is not previewed (known limit) |
| Scale: local (1 to 500 m) | PASS | windows 19 and 20 (above) |
| Scale: regional (1 to 50 km) | recorded, not gated | TEST-044: regional stamps (5 km, 40 km, the largest an item reaches), a 5 km ridge and a 2 km region are in the edit-latency scene; window 19: 65.7 to 249.8 ms drawn exact. The budget is for the owner (section 5) |
| Managing edits: map first, search, nearby list, layer list, graph | PASS (tier A); look is tier D | VIEW-015, VIEW-016, VIEW-020, VIEW-021; rows for split lines and brush drags (#465) |
| Relevance (the camera decides what is shown) | PASS | VIEW-015, `editor/tests/model.rs` |
| Views: 3D free camera, globe overview | PASS with an open bug | VIEW-010, VIEW-013; the movement keys dead after G: root cause not found (`keys-after-globe`, VIEW-034, closes one path); P1-3 (editor-toggle-capture, VIEW-039) fixes Tab, focus and capture |
| Surface type stored on the region, tinted in the debug overlay | PASS | WDEF-012, VIEW-030 (the materials are drawn later, out of the MVP) |
| History: plain undo and redo, 1,000 operations | PASS | see above |
| Input: mouse and keyboard | PASS (tier A and C) | VIEW-022; the real window on the software adapter: `app/tests/gpu_window.rs`. 1000 Hz mice and DPI scaling: a manual check at the laptop, not done |

## 2. Performance (Rasierklinge, release builds, RTX 3080 Ti Laptop)

All four windows: preflight ok, the readiness gate ok, no `--machine-ready`. Raw: `docs/perf/window-18.md` to `window-21.md`. Windows 19 and 20 state 1440p, cells 32 and tau 6 px; window 21 states 1440p (its lit scene sets no cells or tau); window 18 states none of the three.

| Window | Main | Subject | Result |
|---|---|---|---|
| 18 (2026-10-20) | fa8e3f8 | edit latency after the pre-emption change (ADR 0072, STRM-031) | 8 of 9 stamp cases within budget at settle 0 and 40; 200 m altitude with a 150 m stamp 508 / 544 ms against 300 ms |
| 19 (2026-10-20) | ab23639 | 21 cases: brush, regional stamps, ridge, region; preview cost; lattice normals (ADR 0073) in main, settle 25 ms | the stamp case above 125.3 ms; all 9 stamp cases within budget; 150 m brush at 200 m 383.8 ms (over 300 ms); the harness printed `no result` for the gated brush rows (no preview median) |
| 20 (2026-10-20) | 80e1418 | after the brush footprint (#474) | 150 m brush 286.1 ms; 1 km brush 402.8 ms; tiles refreshed 601 and 930 (were 1,816 and 3,857); every gated row has a verdict `ok` |
| 21 (2026-10-20) | branch `change/atmosphere-camera-march` at 29982b2 plus the blend band, against main 80e1418 | the atmosphere camera march (SPIKE-20, #479) | atmosphere 0.60 to 0.76 ms on the 20 km view against 0.25 ms with the volume; the per-pixel march was 1.12 to 1.65 ms and was replaced by a half-resolution haze; the runs held 1,252 to 1,335 MHz (under the 1,400 floor), so the times are overstated |

Not measured: the preview cost at 0, 8 and 32 active stamps is printed by the scene (TEST-043); its figures are in window 19 (not re-read for this draft: not checked). Lines and regions are in the scene as recorded rows only.

## 3. What was built

Counts: 208 commits to main from 2026-10-18 to 2026-10-21 (squash merges, docs and specs included; `git log origin/main --since=2026-10-18`, by author date). ADR numbers are in `docs/adr/`.

### The edit path (main lane)

- **Authored items** (ADR 0066, 0067, 0068): the footprint query, the item reach, region summaries, the per-tile node set id, the stamp key hash and bounds by delta range, a stage with nothing to add passes its source's key through (GEN-027, STRM-029). Item ids and hashes are 128 bits (WDEF-009).
- **Stamp stage** (GEN-026, #365 to #380): crater, hill, plateau as deltas on the running height; the plan cache keys tiles by their reads (STRM-027); the window reads a replaceable definition (STRM-028).
- **Edit visible** (ADR 0072; #345 to #418, #450, #452): refresh with stale-while-revalidate and an edit event that forces a plan (STRM-026, REND-032), the preview displacement (REND-033), pre-emption of stale refreshes with the drawn tiles as the verdict (STRM-031), the editor settle 25 ms, the editor's find skipping tiles no edit can touch.
- **Lines, regions, brush** (ADR 0069, 0070): item file schemas 2 and 3 (AUTH-005), the ridge and channel stamp kinds (GEN-028), the region mask (GEN-029), the brush stage (GEN-030) with strokes inline in the TOML item file, the brush footprint that names the tiles along its strokes (#474).
- **Coastal water** (ADR 0071; SPIKE-17 bug 2): water by connectivity through low ground, the wet floor (GEN-031, kind-6 store blocks, STRM-032) and the consumers (#456, #463).
- **Lattice normals** (ADR 0073, SPIKE-23 and SPIKE-24): the canonical normal is the lattice normal (GEN-019, mesh version 9); it brought the stamp cases within budget.
- **Atmosphere** (ATMO-021, SPIKE-17 bug 1, SPIKE-20): the aerial-perspective range follows the horizon (#404); geometry above 8 km is hazed by a march from the camera at half resolution with a depth-aware upsample (#479).
- **Render floor and slot format** (M5 carry-over: ADR 0062 to 0064; render-floor-3, #303): the vertex tile format retired, compact slot v2, the floor at level 3.

### The editor (support lane and main)

- **Model and panels** (VIEW-015 to VIEW-022, #370, #374): what the camera shows, lists, handles on the planet, typed values, layout as data, the egui shell (SPIKE-14), globe overview with item markers (VIEW-013, #351, #382).
- **Command layer** (AUTH-001, 002, 004, 005; #364): items as TOML files, put, change and delete with undo and redo, group commands, rollback that names its failure, an item of an older stage version re-keyed on load (AUTH-002, #425).
- **Tools** (VIEW-027 to VIEW-036): lines and regions drawn in the window, the inspector of a line and a region, the brush tools and their live preview, layer rows for groups (#465), the key list (#481, #482).
- **Viewer** (VIEW-023 to VIEW-040): a rolling 60 s camera path and F9 with a repro bundle, `cargo xtask repro` replays it (VIEW-024, VIEW-026), the wireframe overlay (VIEW-014), mouse look on the keyboard camera's pose function (VIEW-010), keys after the globe (VIEW-034), a plan refusal never ends the window (VIEW-032, VIEW-037; crash report; F9 frame PNG), a refused plan makes a measured run invalid (VIEW-038), the toggle and capture of the editor mode (VIEW-039), refusals logged once per cause per 5 s (VIEW-040).

### Tooling

- `cargo xtask prepush` (BUILD-016, 017, 021, 022), tier C on Windows in two halves (BUILD-018), the series of `test fast` (BUILD-019), CI history per job (BUILD-020), one test binary per tier (BUILD-015), the perf preflight (TEST-040), a bless that applies by name (BUILD-013).

### Spikes of the milestone

SPIKE-13 (edit latency), 14 (egui), 15 (build speed), 16 (water motion), 17 (the two descent bugs), 18 (tiles coarser for a moment), 19 (the window aborts on a refused coarse LOD), 20 (the aerial-perspective volume), 21 (the wet floor), 22 (base LOD refusal in the real world: no), 23 (why drawn-exact took 508 ms), 24 (lattice normals), 25 (the drawn tiles do not go coarser in scripted flights: not reproduced). The register is `docs/spikes.md`.

## 4. After the owner's test (2026-10-20): the iteration

The owner's notes are in `docs/briefings/m6-owner-test-plan.md`; the queue is the table "M6 iteration" in `docs/status.md`.

| Item | State |
|---|---|
| P1-1 Plan refusals near stamps (CRSH-0001 to 0004) | merged (#489): a plateau's bound was its weight times its declared 5 km bound on both sides, so a fine tile had a 10 km extent; the bounds of a stamp stage are now the range of the running height through the items; ADR 0067 point 4 kept. Checked at the bound level (tier A); not flown at Plateau 7 in the editor |
| P1-2 Smooth sharpens after several strokes | merged (#489): the moved-so-far counter was per item and one drag is one item; counted over the layer now; stage version 4 |
| P1-3 Editor toggle and mouse capture | merged (#486, VIEW-039): Tab toggles the editor unless a text field is edited, a focus gain presses nothing, a panel's focus takes no keys |
| P1-4 Tiles flash to a coarser level while moving (video) | open: SPIKE-18 and SPIKE-25 did not reproduce it in scripted flights; diagnosis with the spikes lane |
| P1-5 Editing frame rate degraded (a coder was building) | open: the owner's retest on a quiet machine, then the planner |
| P2-1 Region clip "inverted" | requirement wording mismatch, relabelled (#492): the stage cuts the layer's edits inside the outline as ADR 0069 says; the editor now says "Removes the edits of a layer inside the outline"; keep-inside (mode 1, `openspec/changes/region-mode1`) waits for the owner |
| P2-2 to P2-9 | later: inspector focus after Enter and arrow stepping; wheel for radius and strength; picking an edit under another; an editing light fixed to the camera; a faint colour shift after fast moves to orbit (adaptive exposure, to confirm); a colour step between 95 and 100 km; missing tiles during an edit's refresh (with P1-4); globe click-to-fly clarity |
| Post-M6: the generator editor, global-scale authoring, AUTH-003 | M7 planning |

## 5. Deviations from the plan and decisions

- **AUTH-003 (version pinning, freeze and bake) deferred out of M6** (owner, 2026-10-20): no crate pins a definition to a generator version or freezes or bakes a region; it belongs with global-scale authoring. No test cites it.
- **Geo-anchored objects deferred** (named by the planner; ADR 0007 and the authored-layers spec describe the concept; whether any M6 change builds it, and the owner's decision text, were not checked for this draft).
- **Clip outside parked** (owner, 2026-10-20): mode 1 is refused at put. The owner's reading of "clip inside" in the test is this mode (P2-1). Decision: build it now or in M7 (about a day: a global item of the layer in every footprint query, a fifth read-record kind, one region per layer and body, a full refresh of the layer per edit of the region).
- **"Edit visible exact" read as the drawn tiles** (ADR 0072, owner, 2026-10-20): seconds are acceptable for tiles not in view.
- **Coarse-first refinement of an edit parked** (owner, 2026-10-20): not needed after lattice normals; the trigger is a brush case in a window that stays over budget.
- **The M3 gate condition 2** (per-tile node set id built; the declared-versus-recorded reads contract test, ADR 0021 point 7, and the tile-level GEN-017 test approved but not written, owner quota 2026-10-20): an open gap (G6), trigger "before the first regional field is stored".
- **Regional scale (1 to 50 km)**: recorded, not gated; the budget is set with the owner from window 19's figures.

Owner decisions open: (1) keep-inside regions now or in M7; (2) the look review of the editor, which is tier D: panels, overlays, the brush ring and the live stroke, the layer rows; (3) the regional-scale budget; (4) the gate itself, GO or NO-GO for M7.

## 6. Tracked risks and open items

| Item | State | Next |
|---|---|---|
| P1-4 tile flashing, P2 list | section 4 | spikes lane; later queue |
| The movement keys dead after G | root cause open (`keys-after-globe` closes one path; P1-3 another) | the owner reads the title readout when it recurs |
| Smooth is not previewed; the brush preview chain is approximate (worst 6 m of a 40 m strength on a bent stroke) | known limits | none planned |
| A merge conflict inside one brush item (two sessions adding strokes) | not tested | ADR 0070 follow-up |
| `Session::set_points` can leave the hover on a part it drops; `Session::set_hover` does not check that the id exists (verifier minor gaps, G9) | left, not fixed | the support lane |
| 1000 Hz mouse and DPI scaling of the raw mouse look | manual check at the laptop | owner |
| Item store revisit trigger (ADR 0066 point 6): more than 20,000 items or a load over 2 s | nothing counts items or times a load yet | measure with the first real world of authored items |
| `test fast` over the 20 s budget on a loaded machine: 39.8 to 58.5 s whole-workspace in sonnet-explorer's and the verifier's runs of 2026-10-20 and 2026-10-21 with "BUDGET EXCEEDED: edit-to-first-test" printed (not recorded in the repo's series: not checked there) | reported; the slowest tests in those runs were `planet-cache` `concurrent_source` (25 to 33 s each under load) | a Tracked item under BUILD-014 when the planner asks |
| `windows (WARP)` tier C over the 16 min CI budget before the two-halves split; the editor smoke `gpu_edit_perf` times out under load | tracked; the smoke was trimmed (#477, #478) | watch |
| Intermittent device loss on the RTX under parallel GPU load | tracked since M5 | the owner's driver update is optional |
| The hash of `gpu_stamps` was re-recorded after the stamp bounds change (#489) | declared in its proposal | none |

## 7. Where the material is

| What | Where |
|---|---|
| Gate evidence per criterion and row, gaps G1 to G10 | `docs/briefings/m6-gate-evidence.md` |
| The owner's test notes and the iteration plan | `docs/briefings/m6-owner-test-plan.md`, `docs/status.md` ("M6 iteration") |
| Perf windows 18 to 21 | `docs/perf/window-18.md` to `window-21.md` (raw output under `docs/perf/window-18/` to `window-21/`) |
| The decisions | ADR 0066 to 0073 |
| The owner's videos and screenshots | the owner's upload folder (over 1 MB, not committed); `docs/dev_screenshots/` |

## Process review

Not drafted (housekeeping step 9 is the planner's, as for M5).
