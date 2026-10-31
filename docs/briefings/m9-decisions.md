# M9: decisions for the owner on return
Status: for the owner (drafted by the spikes session for opus-planner, 2026-10-31). No new findings: each line cites the spike results.
Sources: SPIKE-61 to SPIKE-77 (`spikes/m9-*/results/results.md`); ADR 0088 (proposed); `docs/briefings/m9-draft.md`.

## 1. ADR 0088, the M9 terrain bake (proposed; a data format)
- **Evidence:**
  - the two-pass bake has clean seams at four sites ([SPIKE-70](../../spikes/m9-global-routing/results/results.md), [SPIKE-72](../../spikes/m9-highlands-seam/results/results.md), [SPIKE-74](../../spikes/m9-expected/results/results.md));
  - it takes about 6-7.5 h for the planet ([SPIKE-71](../../spikes/m9-bake-time/results/results.md));
  - with the pipeline, three of four hill landforms pass criterion 1 (SPIKE-74).
- **Recommendation:** accept the mechanism:
  - pass 1, the carve per face at about 800 m;
  - pass 2 per tile, incise only, no uplift;
  - active-cell thermal and deposition;
  - the erosion layer as a separate store.
  Take decision 1's relief driver from decision 2 below (`k` from the relief class, not from L alone).
- **Cost to reverse:** low. The layer is separate and switched; turning it off restores today's field. The bake is re-run, not migrated.

## 2. Relief classes drive the terrain's amplitude (a new ADR; outline in SPIKE-76 section 5)
- **Evidence:**
  - with a class per landform, all four hill landforms pass criterion 1, the Alps included (in-sample, [SPIKE-76](../../spikes/m9-generator-design/results/results.md));
  - a default class map matches Earth's class shares (63 / 14 / 23 %) with smooth `k` ([SPIKE-77](../../spikes/m9-class-map/results/results.md));
  - Erg Chebbi is "not worse" with a sand-sea class (SPIKE-77);
  - `k` from L alone fails the Alps and the arid landforms ([SPIKE-75](../../spikes/m9-arid-alps/results/results.md)).
- **Recommendation:** accept. It brings:
  - a pre-bake relief-class lattice, with a default from L, a mountain-strength driver and a plateau rule;
  - authored overrides as items;
  - amplitude per class (`k`, the fine-crease floor, the carve's strength);
  - the removal of the hill band, the landform-keyed crease factor and the elevation ramp above 300 m.
- **Cost to reverse:** medium. The class lattice is a new channel and the global bake changes (section 4). The old weights stay in git history; a revert is a re-bake.
- **Open inside it (R7):** the Badlands' two sites need different classes (SPIKE-77); the arid classes have no default rule, because the generator's climate is independent of its landforms (SPIKE-75).

## 3. The terrain ADRs (verdict table: SPIKE-76 section 4)
| Verdict | ADRs | Evidence |
|---|---|---|
| keep | 0006, 0014, 0016 and 0074 (re-check level steps and capped tiles at R4's gate), 0019, 0022, 0041, 0056, 0058, 0071, 0073, 0085 | not touched by the spikes, or consumers of the heights |
| amend | 0026 (bake channels, version bump), 0028 (the field formula by class), 0045 (storage: layer and class channel), 0076 (merge: the class is a driver), 0088 (`k` from the class); extend 0066 (class paint as an item) | SPIKE-66, SPIKE-73, SPIKE-75, SPIKE-76 |
| supersede | 0030 by 0088; 0080 points 1-3 by the class amplitudes (point 4 stays) | SPIKE-65, SPIKE-66, SPIKE-73 |
- **Recommendation:** approve the table as the plan; the planner drafts each change after the decision.
- **Cost to reverse:** low per ADR (status lines and new ADRs; ADR 0078's rules).

## 4. The store re-bake
| Change | Why | Cost |
|---|---|---|
| Global bake without the hill band, with the plateau base elevation (`GLOBAL_VERSION` bump) | the hill band is replaced by class amplitude; plateaus need a base elevation not tied to the coast (SPIKE-77: land above 1 km from 12 % to 26 %, Earth 27 %) | about 6 min of baking (GEN-014); every height key changes |
| Coupled run and water fields re-baked on the new heights | they read the base (ADR 0071, ADR 0088 risks) | minutes; the coupled run about 20 s (GEN-088) |
| Relief-class lattice and ADR 0088's ring mean | the class drives `k`; the runtime reads one value per point | under 1 GB |
| The erosion layer | ADR 0088 | about 7-10 GB ([SPIKE-61](../../spikes/m9-bake-prerun/results/results.md)); about 6-7.5 h overnight |
- **Recommendation:** re-bake once, after the class ADR and ADR 0088 are accepted, as R4 PR 1's first step. Keep `global16384` until the new store passes the gate, then delete it.
- **Cost to reverse:** low. The old store stays on disk until the new one passes (135 GB free, SPIKE-61).
