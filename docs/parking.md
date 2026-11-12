# Parking list

Owner, 2026-11-12 (ADR 0120). These items are outside the current milestone, one line each. Nobody works on them. Only the owner moves an item into a milestone; when one moves in, another moves out or the date moves. New owner ideas land here by default. Each line ends with its source and, where useful, its trigger.

## Terrain prototype (P2 and the A/B gate)
- CP2: rivers and lakes drawn on the stored region, G-RT frame time, the thermal-dam options brief; then the A/B gate (`docs/briefings/terrain-prototype.md`).
- G-RT river-flight smoke (the 2,048-slot atlas, dropped == 0), then a G-RT perf window (crusader's next after river-frames; handover 2026-11-12).
- River look frames over R3a for the owner's river look review (the straight river; shape below H1 resolution).
- G-PROFILE re-run on tiles R1h (21, 2), R3a (1, 12), R3b (26, 22) after the starting-surface decision.
- K7 TEST-130: K7 on one layout at 1, 3 and 8 threads and two schedules bit-identical (ADR 0119 point 3; explorer; ID reserved).
- G-SEAM reading on the four terrain types (ADR 0119 point 6; spikes); the ledger row says "not run".
- A3 §12.4 P1: thermal-dam investigation; compaction peak, temp disk and crash-safe replacement tests (G-STORE extension); a smaller and a larger body (G-SCALE).
- A3 §12.4 P1: editor commit readiness, supersession, undo/replay with basin split and merge, latency of three levels (after ADR 0117).
- Briefing item 16: editor path and preview levels E1-E6.
- Briefing item 17: noise of the base bands (gradient noise, crease filter or turned lattice), decided at the gate.
- Per-round convergence indicators (spikes).
- GEN-175 follow-ups: the narrowed-fill refusal; remove `PlanParams::standard`.
- Ledger row for the k=4 memory pair (10,565 against 11,136 MiB, hashes equal; #1447).

## Starting surface (look work; the owner decides on a picture sheet after flying E1)
- Candidates:
  - band slope cap GEN-179 (merged, off);
  - monotone plateau GEN-183 (PR #1449, verified, parked as draft);
  - the valley network, candidate C (SPIKE-111, finishing its time-box).
- The coast tilt GEN-180 is dropped (2026-11-12): it does not drain the plains.
- Flag PR GEN-181/TEST-129 (`--band-cap`, `--coast-tilt`, plateau switch on generate and test-render) and the production-path picture sheet (R1h, R3b, switches off against on, DEM reference beside it).
- Starting-surface bars (z0 basins over 30 cells at most 7 % of land, depth p95 at most 270 m) and the switch-on, after the owner's look approval.
- The valley-network decision brief: a world-level stored lattice (format, CON-10) and a new starting-surface design.

## World parameters (ADR 0108)
- Audit table of every Earth-specific value in continents.rs, global.rs, classes.rs, the band, the carve and the rounds (0.5 day, read-only).
- Lift: a `BasePreset` block in the world definition; the radius from the world definition in ClassSource; the Earth preset reproduces today's bits (GEN-184 reserved).
- Off-Earth presets in the scorecard (ADR 0108 point 6), after the lift.

## Formats and owner decisions waiting
- ADR 0117 schema, with two format questions: the flood epsilon in the commit spec; height bounds in the world definition.
- River at a cube face edge, fix (B): overlapping runs, width in metres per face (store format; REND-063 reserved).
- Audit batch C: sync specs from `spikes/audit-batch-c/results/open-changes-2026-11-12.md` (lead).
- Audit batch E: waits on the owner's list.

## Perf and stores
- Perf: REND-052 verdicts, ground-detail and channel-read cost; TEST-099 waits for a perf window; a current store first (MESH_VERSION 11).
- All stores on disk are refused by keys since #1218/#1225 (read-only references); in-memory `--world` runs only until a new store.

## Later milestones (moved from `docs/status.md` "Dormant", 2026-11-12)
- Volcanoes (SPIKE-102 D5; owner, 2026-11-05: after M9). Young edifices as an additive layer; old ones in the base. Trigger: terrain round 2.
- Isostatic rebound (GEN-124): dropped from M9; a flexural version is the candidate. Trigger: hypsometry tuning, rifts, escarpments.
- Glacial erosion (ADR 0106) and coastal erosion (ADR 0107): out of the prototype's scope (replan, 2026-11-08).
- ADR 0045 (bodies, bake resolution, storage budget): proposed. Trigger: multi-body work.
- Region keep-inside for Cities (owner, 2026-10-21, Q8). Trigger: Cities.
- Global authoring preview (owner, 2026-10-31): a coarse whole-planet preview before the bake. Trigger: after M9's bake, or SPIKE-69.
- Coarse-first refinement of an edit (owner, 2026-10-20). Trigger: a brush case missing its budget.
