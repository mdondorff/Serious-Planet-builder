# M7 Ground detail and edit health: report (DRAFT, 2026-10-23)

Drafted by opus-planner from the log (`cargo xtask log`), perf windows 24 and 25, SPIKE-28 to 32 and the GT pull request #566. No `cargo xtask accept M7` exists; the exit criteria are the owner's (2026-10-21, `docs/briefings/m7-planning.md`, "Scope reshuffle").

## Verdict (proposed): GO once #566 is merged
- Every exit criterion has evidence (section 1); the ground-detail look is approved by the owner as the floor, not the target (2026-10-23): quality rises in M8.
- Needed for the GO: #566 merged with the blessed goldens; the owner's GO.
- Optional: a confirming perf window after #566 (the GT pass changes albedo inputs, not the shader's work; window 25 is from before it).

## 1. Exit criteria
| Criterion | Result | Evidence |
|---|---|---|
| Rock, sand and grass distinguishable at 1.8 m and 0.5 m within 1 to 10 m | PASS (tier D, owner, 2026-10-23: the floor) | GT stills; lit goldens at 1.8 m and 0.5 m in #566 (REND-037), blessed by the owner; G2a procedural detail (REND-034, LOOK-009), G2b gloss (REND-035, LOOK-010) |
| Ground detail with specular at most 1.0 ms GPU at 1440p | PASS | window 25: +0.70 ms (1.8 m), +0.90 ms (0.5 m) of terrain-pass time (window 24, before G2c: +2.4 / +2.9); `docs/perf/window-25.md` |
| 5 cm near plane without z-fighting or clipping | PASS | G0 (#526, VIEW-004): eye 0.5 m, near 5 cm; one depth step 7e-8 m at 1 m |
| The problems list finds and fixes flagged edits | PASS | A5 (VIEW-047): steep, stacked and held-back edits with Fly to, Reduce, Merge strokes, Delete, one undo each; incremental at 5,041 items (VIEW-048) |

## 2. Also delivered
| Item | Ids |
|---|---|
| ADR 0074 (capped steep tiles), complete | VIEW-044, VIEW-040 |
| Edit scaling after SPIKE-29: stamp max-delta cache, editor frame cap 400 (5.6 s to 45 ms at 5,000 edits), item index by reach class, plan cache kept across edits | GEN-032, VIEW-051, WDEF-014, STRM-034 to 037, WDEF-015, WDEF-016 |
| Scale references (F7), atmosphere top continuity (P2-7) | VIEW-050, REND-036, ATMO-023 |
| The editor opens the production store; `cargo xtask store`; relief scale 1 to 4 | VIEW-052, WDEF-017, GEN-033 |
| Documentation caps and the 120-word statement rule | ADR 0075, BUILD-023, BUILD-024 |
| Spikes: instancing (GPU culling), edit scale, driver maps, two-scale hydrology, realism | SPIKE-28 to 32 |

## 3. Carried into M8 (Tracked in `docs/status.md`)
- Orbit flicker and dark-pixel flicker: not reproduced; need the owner's live check.
- Editor-mode CPU cost at cube edges and corners (under 10 fps near "TheEdgeHill").
- Cold plan over empty ground that sees a 5k/20k cluster: 3.75 / 9.4 s (candidate: a per-cell bound aggregate).
- Tiles flash to a coarser level while moving (P1-4, P2-8).
- P1-5 editing frame rate on a quiet machine (owner retest).

## 4. Process notes
- The editor ran on the 256-cell in-memory bake (cells of about 39 km) until VIEW-052; the owner's flatness reports came partly from that. A test session states which world it runs on.
- Two perf windows met a GPU budget by a rewrite (window 24 over, G2c, window 25 under).
- Realism spikes ran cheaply (hours each) and set M8's scope before any M8 code.
