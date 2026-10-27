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
