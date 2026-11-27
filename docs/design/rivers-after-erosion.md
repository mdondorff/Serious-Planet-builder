# Rivers after erosion: a proposed project (design sketch)

Status: proposal, non-normative (owner, 2026-11-27; ADR 0129). Not scheduled. Every number below is illustrative; a value enters a spec only when a milestone schedules the work, and the project needs its own ADR then. Source: the external review of 2026-11-27 (Carbon), adapted by the lead.
Related: CON-05, CON-09, CON-22; ADR 0066, ADR 0069 (multi-point world items), ADR 0072 (per-tile item refresh), ADR 0108 (world parameters, GEN-184), ADR 0113 point 2 (items above erosion), WDEF-012 (`mask_layer`), REND-067 (river ribbons, reserved), `docs/design/hydrology-downstream-seams.md`.

## 1. Thesis: split the world bake from river authoring
| Layer | Role | Editable live? | Authority |
|---|---|---|---|
| 1. Landform erosion with B's major rivers (bake time) | eroded mountains, hills, talus and valleys, plus a plausible first set of major rivers and H1 lakes from B (`region_bake.rs`) | no: a bake-time baseline, masked locally where an author takes over | the regional bake (K5, K6, K7 tiles, ADR 0119) |
| 2. Assisted rivers and lakes above erosion (editor) | rivers and lakes authored as items above erosion (CON-09, ADR 0113 point 2) with terrain-assisted tools, clipping layer 1's water where needed (WDEF-012) | yes, per edit at interactive speed, with undo | per-tile refresh (ADR 0072) over the tiles an item reaches; geometry on the sphere, so no tile or face seams |

## 2. Proposals for the lead's design when scheduled
1. **B's first rivers, stabilized** (candidates for the landform erosion milestone, each behind a switch and gated before its rows run):
   - the face-edge refusal in `region_bake.rs`: segments fade near cube-face edges instead of refusing the region;
   - K7's breach limit (`breach_max_m`, 500 m today) against crest loss: a local gully bound, judged as a trade against pits;
   - angular twigs: burn and ribbons restricted to major trunks (a discharge threshold from the world's runoff), with the 814 m polylines smoothed first;
   - inland holes under sea level: connected-sea outlets (#1644) and the inland carve floored at sea level.
2. **Assisted authoring tools:**
   - trace downhill from a valley head on the baked surface to the sea, a lake or a confluence;
   - a guided valley path between two clicks (a bounded least-cost path that follows valleys and threads around spurs);
   - a monotone water profile solved over the small authored river graph, with meanders scaled by the valley's flatness, and cuts over a limit highlighted;
   - fill a basin: find the lowest rim saddle, set the spill height, and offer an outflow river;
   - auto-seed a region: trace its main valleys and insert them as normal, editable river items.
3. **Evaluating rivers above erosion per tile (ADR 0072):** authored rivers and lakes are world items (ADR 0069 polylines with a reach and a depth limit), paired with WDEF-012 masks that suppress layer 1's water and burn where an author takes over. The bed is lowered per tile by a smooth minimum against the baked surface, and the tile emits the same channel graph and lake records the runtime already reads.

## 3. Seams to protect while rivers are parked
| Subsystem | Rule |
|---|---|
| Runtime and downstream consumers | the channel graph (kind 20), lake records (kind 19), the river and water-mask readers and the GEN-189 validators stay producer-agnostic: they read committed rivers and lakes without assuming K6 made them |
| World definition and items | multi-point geo items (ADR 0069), WDEF-012's mask layer and the per-tile refresh (ADR 0072) stay; an item may reference another item's id (river to lake, tributary to trunk) under deterministic hashing |
| Landform erosion | the B bake can suppress its channel burn inside WDEF-012 masks, or everywhere on a dry world (ADR 0108 runoff zero) |
| Surface sampler and editor | the above-erosion layer of ADR 0113 point 2 stays able to carry bounded lower-only height deltas and water emission per tile, with no regional re-bake |

## 4. The cheapest test before any build (CON-17)
- A spike of about two days, when unparked: on one baked R1h store and one M1 store, prototype the trace, the guided valley path and the smooth-minimum bank carve in the editor.
- It fails if authoring 10 branched rivers and 2 lakes takes over a few minutes, if a tile refresh is too slow for interactive editing, or if the banks look artificial under a low sun. Its reversal cost is the spike itself, with no change to main.
