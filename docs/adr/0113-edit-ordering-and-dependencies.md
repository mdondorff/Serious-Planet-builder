# 0113 Edit ordering and dependencies: landform and driver edits below erosion
Status: proposed (spikes session for opus-planner, 2026-11-08, inbox #100); the owner accepts after the external review and the D2-D10 pass; a reading of CON-08 and changes to ADR 0007 and 0066 need the owner
Date: 2026-11-08
Source: `docs/design/terrain-target.md` §7; SPIKE-105 §3 (X1-X4), §13.5 (D4); ADR 0007, 0066, 0072, 0086

## Context
- Authored items are stages after the base and the erosion layer; an edit moves heights, but rivers and erosion do not respond (ADR 0007, ADR 0066 point 4).
- Edits under the erosion were deferred to M9 and not taken up (ADR 0086 point 3).
- Reach is a radius (ADR 0066 point 1); a river can change far downstream.
- The owner's P0: edits update erosion and hydrology (R1a, R4b).

## Decision
1. Landform height items and driver items (lithology, uplift, climate, class paint) sit **below** erosion: applied at K2 and K3; `B` gains a read-time term for them.
2. Constructed features (roads, plots, objects) stay **above** erosion, exact (ADR 0089 point 11).
3. Dirtiness below erosion is found by keys over the routing (ADR 0112 point 5), not by a radius; the radius of ADR 0066 point 1 stays for items above erosion.
4. Propagation ends at exact equality of a catchment-round's inputs, never at a tolerance.
5. A commit is a set of keys; the runtime draws the previous commit until the new one is ready (ADR 0072, CON-22); a superseded result is dropped.
6. CON-08 is read: H0 and the regional solver are global-coarse and regional stages that read items; "a stage reads only earlier outputs" holds across rounds of one solve as a fixed order (owner to confirm the reading).

## Consequences
- Amends ADR 0007 (layer order), ADR 0066 points 1 and 4; settles ADR 0086 point 3.
- Undo removes an item: earlier keys read back.
- Gates G-INC, G-EQ.
