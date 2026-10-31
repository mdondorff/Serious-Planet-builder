# M9: decisions for the owner on return
Status: for the owner (drafted by the spikes session for opus-planner, 2026-10-31). No new findings: each line cites the spike results.
Sources: SPIKE-61 to SPIKE-80 (`spikes/m9-*/results/results.md`); ADR 0088 (proposed); `docs/briefings/m9-draft.md`.

## What the combined generator gives ([SPIKE-80](../../spikes/m9-combined/results/results.md))
With decisions 1, 2 and 5 together (class map and plateau rule, plate-margin belts, class amplitudes, the two-pass bake), at coarse global resolution and at re-matched scorecard sites:
- land above 0.5 / 1 / 2 / 3 / 4 / 5 km is 48 / 27 / 7.6 / 3.3 / 1.8 / 0.75 %, against Earth's 47 / 27 / 6.7 / 3.2 / 1.9 / 0.8 %; the highest land is 8.0 km (Earth 8.8 km), in belts, not blobs;
- class shares are near Earth's: plains 61 % (Earth 61 %), hills 20 % (14 %), mountains 19 % (25 %);
- the plains, Loess, Erg Chebbi (authored sand-sea class) and Himalaya improve on today. Three hill landforms pass criterion 1, but only with the hill band kept and keyed to the class; the Alps fail at one site (0.47) and the Badlands get worse;
- the evidence is a stand-in (the new elevation added to today's stored field), not a re-bake; decision 4's re-bake is the real test.

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
  - the removal of the landform-keyed crease factor and the elevation ramp above 300 m;
  - the hill band kept, with its amplitude from the class (30 m on plains to about 825 m in high mountains), not removed: without it the hill landforms fail criterion 1 at re-matched sites (SPIKE-80 corrects SPIKE-76).
- **Cost to reverse:** medium. The class lattice is a new channel and the global bake changes (section 4). The old weights stay in git history; a revert is a re-bake.
- **Open inside it (R7):** the Badlands' two sites need different classes (SPIKE-77); the arid classes have no default rule, because the generator's climate is independent of its landforms (SPIKE-75).
- Derived classes: see [SPIKE-81](../../spikes/m9-class-drivers/results/results.md) (a rule table over plate history, rock hardness, aridity and height; it adds badlands and sand sea as derived classes, and the Badlands score about equal to today's baseline at driver-chosen sites).

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
| Global bake with the class-keyed hill band, the plateau base elevation and the plate-margin belts (`GLOBAL_VERSION` bump) | the hill band's amplitude follows the class (SPIKE-80); plateaus need a base elevation not tied to the coast (SPIKE-77: land above 1 km from 12 % to 26 %, Earth 27 %) | about 6 min of baking (GEN-014); every height key changes |
| Coupled run and water fields re-baked on the new heights | they read the base (ADR 0071, ADR 0088 risks) | minutes; the coupled run about 20 s (GEN-088) |
| Relief-class lattice and ADR 0088's ring mean | the class drives `k`; the runtime reads one value per point | under 1 GB |
| The erosion layer | ADR 0088 | about 7-10 GB ([SPIKE-61](../../spikes/m9-bake-prerun/results/results.md)); about 6-7.5 h overnight |
- The belt amplitude 5,200 → 7,000 m joins the `GLOBAL_VERSION` bump: land above 2 / 3 / 4 km 7.1 / 3.2 / 1.4 % against Earth's 6.7 / 3.2 / 1.9 % without Antarctica ([SPIKE-78](../../spikes/m9-belts/results/results.md)); with SPIKE-79's plate-margin belt mask the amplitude is 6,000 m ([SPIKE-79](../../spikes/m9-belt-shape/results/results.md)).
- **Recommendation:** re-bake once, after the class ADR and ADR 0088 are accepted, as R4 PR 1's first step. Keep `global16384` until the new store passes the gate, then delete it.
- **Cost to reverse:** low. The old store stays on disk until the new one passes (135 GB free, SPIKE-61).

## 5. Mountain belts from plate margins (part of the class ADR or its own; the owner's call)
- **Evidence:**
  - the range zone is an isotropic noise, so today's high ground forms round patches (aspect 3.4) ([SPIKE-79](../../spikes/m9-belt-shape/results/results.md));
  - convergent margins of about 14 Voronoi plates, with coastal arcs, give belts: 96 % of land above 2 km has aspect 4 or more, area-weighted aspect 15, against a median of 7.5 for Earth's named ranges (SPIKE-79);
  - they keep the hypsometry, and with width variety and back-arc plateaus the highest land reaches 8.0 km ([SPIKE-80](../../spikes/m9-combined/results/results.md));
  - noise isolines also give belts, but they close into rings (SPIKE-79).
- **Recommendation:** accept as the default mountain-strength driver of the class map, inside the class ADR: it replaces the range zone, and an authored class still overrides it. The plates are a mask only (no plate tectonics, no plate ages); continents stay as they are.
- **Cost to reverse:** low with decision 4's re-bake. The plate layer is a pure function of the seed, and dropping it restores the range zone.
