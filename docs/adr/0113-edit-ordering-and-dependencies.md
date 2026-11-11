# 0113 Edit ordering and dependencies: landform and driver edits below erosion
Status: accepted for the prototype (owner, 2026-11-11: "approve with the conditions mentioned", after the external review of 2026-11-11), with the conditions below (it changes accepted ADRs 0007, 0066 and 0089). Production: G-EQ, G-INC on E1-E7 at the A/B gate
Date: 2026-11-08
Source: `docs/design/terrain-target.md` §7; SPIKE-105 §3 (X1-X4), §13.5, §18; ADR 0007, 0066, 0072, 0086

## Context
- Authored items are stages after the base and the erosion layer; an edit moves heights, but rivers and erosion do not respond (ADR 0007, ADR 0066 point 4).
- Edits under the erosion were deferred to M9 and not taken up (ADR 0086 point 3).
- Reach is a radius (ADR 0066 point 1); a river can change far downstream.
- The owner's P0: edits update erosion and hydrology (R1a, R4b).

## Decision
1. Landform height items and driver items (lithology, uplift, climate, class paint) sit **below** erosion: applied at K2 and K3; `B` gains a read-time term for them. (Amended by ADR 0117, proposed: the rainfall, temperature, lithology and uplift edits are driver regions of a world-level table, not items.)
2. Constructed features (roads, plots, objects) stay **above** erosion, exact (ADR 0089 point 11).
3. Dirtiness below erosion is found by keys over the routing (ADR 0112 point 6), not by a radius; the radius of ADR 0066 point 1 stays for items above erosion.
4. Propagation ends at bit equality of the canonical stored forms of a catchment-round's inputs, never at a tolerance.
5. An edit that changes the land mask or landmass connectivity re-derives the regions at its commit; all routing and erosion state of an affected region follows through the keys, which hold no region id. A merged region over the job limits goes to a batch re-run. A change of the sea level is a full rebuild.
6. A commit is one coherent set of keys (surface, hydrology, basins, water levels, materials) under a `commit_id`. A frame reads one commit; the runtime draws the previous commit until the new one's drawn tiles are ready (A3 §7.4; CON-22: never blocking); a superseded result is dropped. Per-tile refresh (ADR 0072) stays for items above erosion.
7. H0 and the regional solver are stages over global and regional domains that read items, and the rounds of one solve are an iterative stage with a fixed count (CON-08 v1.7, ADR 0116). Point 4's bit equality is CON-29's rule.

## Consequences
- Amends ADR 0007 (layer order), ADR 0066 points 1, 2 and 5 (items below erosion are bounded by keys, not by footprint or reach; point 4 is already amended by ADR 0068) and ADR 0089 point 11 (height edits leave the override stage); settles ADR 0086 point 3. Point 6 is new against ADR 0072 (runtime and editor work).
- Undo removes an item: earlier keys read back.
- Gates G-INC, G-EQ, on E1-E7 (E7: land bridge joined, then separated).

## Conditions (external review, 2026-11-11)
1. Point 6 is completed by: the smallest coherent simulation domain that a commit waits for; whether unaffected regions commit on their own; how failed and superseded work is handled. Terrain and water never show different generations.
2. An end-to-end undo and replay test includes basin splits and merges.
3. Preview-quality identities stay apart from authoritative state. The editor's latency contract names three levels (immediate preview, regional re-solve, full rebuild), each measured.
