# M7 planning: open questions for the owner

Status: draft (opus-planner, 2026-10-21). Nothing is built until the owner answers Q1.

## Where things stand
- The roadmap (background report §15) puts **M7 Cities** next: suitability, seeds, roads, parcels, buildings, city HLOD, terrain stamps from roads. Exit: a 10^5-building city at 60 FPS from 2 km altitude, with a seamless approach.
- Deferred out of M6 and waiting for a home:
  - global-scale authoring (continental edits, the global re-bake, a generator editor; owner, 2026-10-18);
  - AUTH-003 (version pinning, freeze and bake; owner, 2026-10-20);
  - geo-anchored objects (in the roadmap's M6 scope, not built);
  - region clip outside (parked).
- The owner's M6 test named "updating the generated world" as the second need after stability.

## Questions
| # | Question | Options | Recommendation |
|---|---|---|---|
| Q1 | What is M7? | A: Cities, as in the roadmap. B: "World authoring 2": the generator editor, global-scale edits, the re-bake, AUTH-003 pinning and freeze. Cities move to M8. C: B first, small, then Cities in the same milestone. | B. It answers the owner's own ask, the deferred items share one design (all change the global bake), and cities need pinned, freezable terrain and anchored objects underneath. |
| Q2 | Generator editor depth | parameters of the existing generator (sliders, re-bake) / a node graph (Houdini-like) / both, graph later | Parameters first, graph later: the definition is already a DAG, and a parameter UI over it is the cheap first step. |
| Q3 | Re-bake time the owner accepts after a global parameter change | seconds (a preview at a coarse resolution) / minutes (a full 16384 bake runs about 6 min plus passes) | A coarse preview within seconds, then a full bake in the background. |
| Q4 | Version pinning semantics | a world keeps its generator version until the owner upgrades it explicitly / it always follows the newest generator | Explicit upgrade, with a diff view of what changes. |
| Q5 | Freeze | freeze a region's heights as stored data (immune to generator and lower-layer changes) / freeze only against generator upgrades | Stored heights per frozen region (it also answers clip outside's "freeze mode" feeling). |
| Q6 | Geo-anchored objects in M7? | yes (markers, simple meshes placed on the surface) / with Cities | With whatever comes first that needs them; in B, a minimal anchor type for the generator editor's previews only. |
| Q8 | Region mask "keep inside" (a layer's edits apply only inside the region; mode 1 of ADR 0069). M6 built "cut inside" (mode 2), which the owner's test read as inverted; the M6 requirement row was worded as keep-inside. The editor now labels mode 2 as "cut". | build keep-inside now in M6 (about a day: every tile of the layer must read the region) / in M7 with freeze / drop it | In M7 with freeze: both are region-wide rules over a layer, and the owner found the hard clip of limited use for now. |
| Q7 | Editor process | keep the three lanes / fewer parallel sessions (quota) | Two active lanes plus spikes on demand (the current practice). |

## What happens without answers
- M6 closure continues: P1 hardening by automated tests, P2 fixes, the M6 report draft.
- No M7 code. The first M7 step after Q1 is one design ADR and a spike on the re-bake time (Q3).
