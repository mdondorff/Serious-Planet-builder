## Why
- G0 contracts reading 8 (lead, 2026-11-17): markers are unique per (reach, anchor), and a marker's id hashes its reach.
- G1d creates `trib_junction` markers, and the spikes lane's G1c already uses that id; the lead orders it into GEN-189 at the start of G1d (2026-11-17).

## What changes
- GEN-189:
  - Ids row: a marker's id is `hash(node hash, on_reach)`;
  - Uniqueness row: markers of one kind once per (level, anchor, reach).
- `bake::river_graph`:
  - `marker_id(body, kind, level, anchor, on_reach)` = `id_hash(node_id(...), on_reach, 0x8A2E_0008)` (reading 15: the salt of the spikes lane's G1c);
  - `node_id_of(body, node)` picks the rule by the node's marker field;
  - the id validator reads `node_id_of`;
  - uniqueness counts a marker kind per reach.
- TEST-140: the marker id with its salt, its mismatch, and two junctions on one anchor on two reaches. A kind change on a trunk's end retires the junction on that reach and the tributary ending there.

## Impact
generators (`bake/river_graph.rs`); tests `river_graph.rs`, `g0_graph.rs`. No stored graph exists. A topology change of a reach now also changes the ids of its markers.
