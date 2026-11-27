# Learnings archived from docs/learnings.md (2026-11-26, the cap of ADR 0075)

Entries of the P3 E1 river stage on K7, superseded by the river-graph work or fixed since; still true unless a later entry says otherwise.

## 2026-11-14 E1's drawn "lakes" were mostly sea on inland ground below 0 m
Scope: project (K7 sea floor, window outlets, coastal band)
- 80 % were coastal-band water on 4,764 cells K7 put 3.5-42 m below 0 m, 1-23 km inland; the 4,317 km2 margin lake is a window-edge outlet never lowered (SPIKE-112; lanes digest 2026-11-13).

## 2026-11-14 The drawn ground differed from B + L by the crease detail only
Scope: project (runtime detail, GEN-187, VIEW-072)
- 1-7 m (p50 -2.1 m), seen as water flowing uphill; the base was bit-equal. Pinning the detail cost 31 % of frames until a memo (ADR 0122; #1490).

## 2026-11-14 The flight stalls were viewer residency, not machine load
Scope: project (viewer residency VIEW-070, G-RT)
- Resident tiles grew from 10,922 to 66,114 with no eviction (2-10 s stalls above about 20k); with the cap a 26-min flight had 0 stalls (#1473, #1485).

## 2026-11-14 The width law never reaches a visible size on the prototype
Scope: project (K7 width GEN-165, river drawing)
- Widest trench 1.9 cells; the drawn width's one-cell floor makes tributaries as wide as the main stem (gsurfi-reread-2026-11-11.md).

Full text of the 2026-11-26 entry, shortened in docs/learnings.md for the cap:

## 2026-11-26 River-graph fixes ran only downstream; S0, the base and the climate were treated as fixed
Scope: project (any pipeline gated on a look or on bars measured against a stage's own input)
What happened: from G2 (2026-11-22) to 2026-11-26, at least 10 river-graph fix rows moved a defect to another stage or bar. The attribution (2026-11-25) then traced 88-93 % of the residual look pattern, plus the blocky beds and the straight valleys, to S0, the start surface (K5 and the base), which no row changed. Causes:
- The plan defined S0 as the reference of every delta measure (budget, relief lost, saddles, crests), so changing S0 looked like moving the yardstick.
- The plan listed the base and the climate (K0-K4) as unchanged inputs, with a one-direction rule.
- G3 let main take only changes that move no stored output, which ruled out base fixes.
- The lead generalised the owner's remark "M1's look stays after G3; base changes need no river-graph change" (2026-11-23) into a test for every parked item, kept the items parked although the owner asked to reconsider them, and deferred the owner's 2026-11-25 pointer to fix the straight rivers in S0.
- Kept lakes came from a geometric rule (an early keep reading plus a 10 m hump cap), not from water balance (ADR 0111 point 5), so too many lakes could never be traced to precipitation or evaporation.
Assumption it changes: ADR 0120's lead role (attribution and sequencing); ADR 0111 point 5 (lakes by water balance) was not applied in the prototype.
Proposed follow-up: routing rule 10 (attribute before fixing; upstream stages and world parameters included); gate measures against an outside reference (GLO-30 statistics) as well as against the stage's input; kept lakes by water balance when the work resumes.
Obsolete when: never.

The 2026-11-14 block, moved 2026-11-26 for the cap (ADR 0075); still true unless a later entry says otherwise:

## 2026-11-14 The L basins come from the 814 m to L hand-over, not from B
Scope: project (hand-over 814 m to L; P3 E1)
- The solved 814 m surface drains at its grid (5.9 % of river length on closed basins); read at L it has 74.5 %: a D8 diagonal between two higher cells is a saddle in any bilinear or C1 read. Five B-stage variants leave 61-70 % against 71 % (spikes/drain-base/results/results.md, #1498).

## 2026-11-14 The basin metric decides a river verdict
Scope: project (river gate, BUILD-055)
- "Any pit over 5 cm" fails SPIKE-115's flat windows (29.1 %, 13.7 %); basins of 0.5 m and 16 cells under a 1e-7 m fill give 0-4.6 % on the same kind of surface (spikes/combo-117/results/results.md). State the metric with every share.

## 2026-11-14 K7 beds are not monotone
Scope: K7 (GEN-169, GEN-176, GEN-185)
- The burn only lowers, so line heights rise downstream: on r3a-e1 (R1h window) 63,167 of 219,390 line cells rise over 0.25 m (max 44.7 m). Cuts over 50 m come from H1 lines crossing L ridges (gprofile-read-2026-11-11.md; crusader's outbox).

## 2026-11-14 A stored channel level bounds the bed from above
Scope: project (K7 levels, river drawing)
- The drawn ground lies below it at 83 % of channel points (median -3.8 m, p5 -30 m); on r3a-p2 the line is a raised one-cell line beside the carved low (SPIKE-113). It is no water surface.

## 2026-11-14 Name a window by face, box and seed
Scope: project (spike and gate reports)
- SPIKE-114/115 called face 1, 8056-8675 x 760-1379, seed 14 "R3a"; the spikes lane called it R1h and used R3a for face 4, seed 15. Canonical: R1h, R3a, R3b (lead, 2026-11-14; #1506).

## 2026-11-14 A 4-neighbour hand-over aligns rivers to the grid
Scope: project (hand-over, ADR 0123 draft)
- 66.5-68.6 % of channel steps run along an axis, against 47 % for B alone or one round at L (spikes/combo-117/results/results.md).

## 2026-11-14 The river network cannot be rebuilt from B
Scope: project (store format, river geometry)
- Only 8.0 % of channel cells agree with routing on B + regional: store the L geometry (about 0.9 MB a region) or the regional change (spikes/rivers-at-l/results/results.md).

## 2026-11-14 One regional-kernel round at L is the optimum
Scope: project (regional kernel at L; K7 flood)
- Three rounds raise the relief to 156/143 %, ten to 521/358 %; Schott 2024 multi-scale with unchanged parameters gives about 3x. K7's canonical flood refuses flats of an eroded surface (spikes/erosion-at-l/results/results.md).

## 2026-11-14 Rounds and routing at L are not tile-bounded
Scope: project (partitioning, CON-30)
- The largest catchment holds 5.07 M cells against 0.59 M in a tile, with 27,636 pits; a region stage, or Barnes 2016's exact tiled flood (spikes/erosion-at-l/results/results.md).

## 2026-11-14 The layer read's clamp made steps at L cell-centre lines
Scope: project (layer read GEN-188; the bake's reads)
- Max 12.5 m, 1.5 % of crossings over 0.1 m (0.001 m without the clamp); the bake's twin reads clamp the same way. SPIKE-114's "41 % steps" counted curvature too (spikes' outbox; rivers-at-l results).

## 2026-11-14 No solver path keeps an inland lake
Scope: project (edits and lakes, GEN-162, ADR 0113)
- The breach cuts every basin; a 400 m dome keeps about 188 m after the rounds (SPIKE-112).

## 2026-11-14 Memoised rounds recompute the serial area sweep
Scope: project (regional solver, edit path)
- About 80 % of a round, so memoised rounds take 2x resident ones (159 s a round at 1.42e8 land cells); a per-catchment parallel sweep: 69 s on 8 threads, bit-equal (spikes' outbox).

## 2026-11-14 Later-stage start-surface fixes failed
Scope: project (start surface; GEN-180 removed)
- The coast tilt made R3a worse at every setting (12.0-15.6 % against 9.9 %); the band's crest rings dam a basin each (SPIKE-109, SPIKE-111; #1500).

## 2026-11-14 A probe that copies library code goes stale
Scope: project (gates and probes)
- G-LOCAL's L1 mismatch (437,249 cells, up to 691 m) was the probe missing GEN-173's graded flood; with it, 0 of 1,040,400 differ (glocal-f32-2026-11-12.md). Probes call the library.

## 2026-11-14 K7's seam difference opens in the carve
Scope: K7 (ADR 0119)
- Under both tilings the burn and breach are bit-identical; the carve differs on 7,032 cells (at most 0.012 m), deposition adds the 0.087 m maximum (k7-seam-2026-11-11.md).

## 2026-11-14 gpu_skeleton times out only under CPU oversubscription
Scope: project (tier C, prepush)
- The two pipelined loops take 2.8 s alone and 28-69 s with 20-80 busy processes on WARP; the RTX is unaffected (spikes/gpu-skeleton/results/results.md).

## 2026-11-14 The CPU latch recurs
Scope: project (perf windows, Rasierklinge)
- About 15-17 % `PercentProcessorPerformance` under load on 2026-10-29 and 2026-11-13; an EC reset (power button 30 s) cleared both; SPIKE-113's timings were void. Check under load before a window. Obsolete when a BIOS update removes it.

## 2026-11-14 Messages pause after 10 sends, and order files wake nobody
Scope: project (lane process)
- Without an owner message the app pauses a lane's sends; an idle lane does not read its order file (explorer idle about 5 h on 2026-11-14; ADR 0118).

## 2026-11-14 A default-on stage after one measurement made its defect worse
Scope: project (generators, process)
- The round-0 breach needed a second PR to switch off; three later-stage basin fixes were dropped (ADR 0118, ADR 0120).
