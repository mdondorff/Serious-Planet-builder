# Owner live test 2026-10-27: findings and plan

Source: the owner's session on Rasierklinge, editor and viewer, build `a7e739b5` (main after #821), store `C:/DEV/SeriousPlanetData/global16384`, site hills-b and others. Collected by opus-planner. Screenshots are in this folder (`frame-<n>.png`). The F9 bundles and camera paths (`frame-<n>.bundle`, `frame-<n>-path.txt`) and the 37 stall reports are outside the repository in `C:/DEV/SeriousPlanetData/owner-test-2026-10-27/` (`crashes/`).

## Evidence from the stall reports
| Item | Value |
|---|---|
| Stall reports (frame over 1 s) | 37 in about 1 h |
| Longest | 14.8 s, 14.5 s, 12.8 s, 9.9 s, 9.7 s |
| Phase | "chunk frame, refusals and editor update" in 34 of 37; "camera, ground heights and the globe overview" in 2 |
| Locks the main thread waited on | none |
| Horizon jobs | `baked: 0, landed: 0` in 36 of 37 reports: no horizon map was baked in the whole session |
| Chunk jobs | e.g. computed 651, disk hits 1057, landed 1708, evicted 1630 |

## Findings, grouped
| Group | # | Finding | Screenshot | Read | Kind |
|---|---|---|---|---|---|
| D0 Coarse levels uneroded | 17 | Erosion reaches only the chunk's levels; coarser levels show uneroded ground: a harsh disconnect between levels (owner, after the test) | frame-41356, frame-50119 | the chunk change stops at the pyramid top; coarse levels cannot read chunk solves (LOD-013) | **M8 (owner, 2026-10-27)**: ADR 0084 (proposed), a coarse valley carve in the base |
| A Editor stalls | 11 | Main-thread stalls of 1 to 15 s while editing | stall reports | the editor's chunk frame / refusals / editor update phase; the edit-latency scene does not reproduce it | blocker (CON-22, criterion 3) |
| A | 1 | Editor mode at full-planet view: frames up to 1.3 s on the CPU; viewer mode cheap | | editor per-frame work grows with the visible area | blocker |
| B Shadows | 16 | No shadow of a plateau on a crater next to it after edits | frame-197238 | no horizon map baked all session (stall reports); also STRM-057 re-bakes only touched chunks, not neighbours whose horizon reads the edit | blocker (M8 delivers shadows) |
| B | 6 | A washy dark spot at 30-84 km with no terrain to cast it | frame-102263 | coarse horizon map source or 19.6 km smear; hard black spots are gone (REND-043 confirmed) | M8 |
| C Debug view | 4 | K views height and drainage area: pool workers panic (`cache/src/key.rs:354`: a field that reads cells must report them in read_records_for) | | the views' fields read stored cells without reporting them; no test renders K views on a store | bug |
| D Chunk edges and tile cuts | 2 | Rivers pop up when moving closer; no continuity beyond | frame-31358, frame-32034 | rivers exist only in landed chunks; the base field has no river imprint | M8 look |
| D | 3 | Harsh cuts and different geometry near the coast, following one LOD level | frame-41356, frame-41595, frame-50119 | the chunk change's apply level (entry step) or the chunk-range edge | M8 look |
| D | 7 | Cuts between tiles; console: 78-116 tiles held back as too steep to morph (ADR 0074) | frame-116591 | coarse tiles beside fine ones; the +-300 m chunk widening eats the morph headroom (measured change -207..+1.2 m) | M8 look |
| D | 8 | Harsh river artefacts at the same place | frame-116591 | tile cuts along steep channel walls | M8 look |
| E Editing behaviour | 12 | Edits show at once, then change after the re-solve: no WYSIWYG; a 400 m hill eroded to half | | D3 option 2 at work; slumping to the angle of repose (GEN-058) removes mass | owner decision |
| E | 12b | The erosion on top makes the hill look good | | the detail is wanted, the mass loss and the surprise are not | owner decision |
| E | 15 | A river over a plateau edit leaves a raised seam instead of a cut | frame-187750 | the coarse route ignores the edit; the cut cap (GEN-069) turns the canyon into a dam | owner decision / M9 |
| F River form | 5 | Flat river beds with a thin pixelated channel | frame-72408, frame-78773 | flat corridor floor; 1-2 cell channel on the 8 m grid (SPIKE-40) | M9 |
| F | 9 | Tributary junctions: harsh steps and boundaries | frame-134050 | depth by area makes discordant junctions; overlapping corridor bands | M9 (junction rule maybe earlier) |
| F | 10 | A "border" keeps a river from running downhill | frame-139490 | the coarse network fixes the path and border crossings | M9 |
| F | 14 | Mostly straight rivers, one natural winding stretch | frame-181555 | local routing (ADR 0081) where the ground has shape; straight coarse segments elsewhere | M9 |
| G Resolution | 13 | Walking height looks low-res; high altitudes look dense | | refinement ends at the data's 8 m; ground detail is shading only | M9/M10 |
| H Look | look review | default vs sample driver look; brightness | look-review-2b | default has ground detail 0; adaptive exposure | owner decision |

## Decisions (owner, 2026-10-28)
1. **No degradation at every change.** A gate on the real store runs before each merge that touches generators, streaming, render or the editor (editor with chunks, one large edit, a short flight); CI runs the same counters on a small world. Known failures at its landing are quarantined with Tracked entries, never weakened.
2. **Gate checks:** no frame over 100 ms; 0 held-back tiles (ADR 0074) on procedural ground without edits (any held-back tile there is a generator or bounds bug; the hold-back stays as the safety net; under an edit it is a flagged problem edit for the user); horizon bakes above 0; no worker panic; the step between levels within criterion 3's bound.
3. **M8 acceptance case for rivers and erosion:** no visible pop at any level boundary, in the viewer above all; judged on a fixed descent from orbit to 1.8 m at the capture sites.
4. **Rivers run downhill on the drawn ground:** along every river in a chunk the drawn bed rises downstream by at most 0.5 m (frame-139490: uphill and sideways rivers).
5. **Decision measurement before any build** (1 to 2 days, in memory): realistic channel depth (SPIKE-50 d1) with the coarse carve in replace form and a detail-only correction (the chunk change filtered to the apply level adds nothing beyond the coarse carve). Stop rule, all three to pass on the real store:

| Condition | Pass |
|---|---|
| Level boundaries | step 15/14 and the entry step under 0.25 m p99; no visible pop in the owner's descent |
| Rivers | no bed rise over 0.5 m downstream |
| Chunk change | bounds margin of tens of metres; held-back tiles 0 at the capture sites without edits; the correction small against the erosion's detail |

6. **Pass:** build it in M8 (about a week). **Fail:** runtime erosion is a failed experiment for now: the chunk stage off by default, M8 closes on what works (coarse relief, shadows, flight, look), hydrology redone in M9.
7. No other hydrology or erosion work until the decision; option A (soft hand-over) is not built.

8. **D0 measurement failed the stop rule** (crusader, 2026-10-28, `docs/milestones/M8/d0/README.md` on `change/m8-group-d`): rivers 275 to 367 rises per 100 km against 90 today; held-back tiles 50 to 59 at two sites with the smallest margin; the level step passes only when the chunk keeps detail under 9.5 m.
9. **Fallback (owner, 2026-10-28):** hydrology and erosion are out of M8's acceptance; the final call waits for the gate run with the chunk stage off (explorer).
10. **If the chunk stage is off in M8:** keep the code that serves the learning and the redesign; during M9 the code that did not work is removed and its learnings recorded (`docs/learnings.md`, the spike results).
11. **M9 starts with a literature study** (owner, 2026-10-28): runtime hydrology and erosion with a level-of-detail pyramid and editing, compared against this architecture (readiness independence, determinism, plan-based renderer); it asks whether the chunk approach does not fit this application or was implemented wrongly. Design and execution follow it.

## Plan (proposed; M8 does not close with groups A to D open)
| Step | Work | Lane | Done when |
|---|---|---|---|
| 1 | A: split the stalled phase (chunk frame, refusals, editor update) in the stall probe, find and fix the cause; the 1.3 s full-planet editor frame; add the editor with chunks and a real edit to the stall smoke | crusader | no frame over 100 ms while editing on the 16384 store; smoke test gates it |
| 2 | B: why no horizon map was baked in the editor (regression since #786/#789/#797/#806?); re-bake the neighbours whose horizon reach an edit enters; zero-count check extended to the viewer and editor (the scenes have it, the window does not) | explorer | a plateau's shadow shows on its neighbour within criterion 6's 10 s; viewer counters checked |
| 3 | C: K views report their stored-cell reads; a test renders every K view on a store | explorer, with 2 | the test passes, no panic |
| 3b | D0: ADR 0084's coarse carve: SPIKE-49 sizes the one-pass carve on the real store (coarse-to-fine step at the findings 2 and 3 sites), then the change | spikes, then a lane | entry step and disconnect measured before and after; owner judges the stills |
| 4 | D: held-back tile count with chunks on vs off and with the asymmetric bound; the entry step at the coast; river imprint beyond the chunk range (coarse network in the base field) vs a wider range | crusader after 1 | a proposal with numbers, then the fix; criterion 3 re-measured |
| 5 | B6: the coarse horizon map's source and the dark spot | explorer after 2 | cause named, fixed or tracked |
| 6 | Owner decisions: E (D3 revisit: edits on top as default, erosion keeping mass), F and G scope (M9 or M8), the look, the 1 m / 500 m exception, the drawn-tile reading of criterion 6 (#823) | owner | decisions recorded |
| 7 | Acceptance window, then owner test 2 on the same sites | planner, owner | criteria 1 to 7 |

Estimate: steps 1 to 3 about a day; step 4 one to two days; E depends on the decision. Quota: about 3 to 5 % of the week.
