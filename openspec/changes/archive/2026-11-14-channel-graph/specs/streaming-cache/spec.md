## ADDED Requirements
### Requirement: STRM-084 The channel graph is a stored kind read through the commit
The bake SHALL store the channel graph as one record of kind 20 per L tile, holding the pieces of the segments inside the tile's block of H1 cells and one H1 cell beyond it, and the commit record SHALL name the blocks (commit layout version 3).
- A piece is a run of vertices of one segment on one face: the segment id and part, its first vertex's index, the segment's vertex count and all its lake cuts (so the producer's pieces follow from any one piece), the planar steps, the `Q_w` and bed codes.
- A damaged record, another layout or key, or a block other than the one asked is refused naming the record; a count the payload cannot hold is an error.
- A feed over the commit reads the blocks a window touches and returns K7's channels; a graph in area units is refused by the writer.
- Channels read through the commit give the L tiles of a bake that had them in memory.

Verify: A
Status: planned
Source: Architect 2026-11-10 and 2026-11-11

#### Scenario: Round trip
- **WHEN** a face is baked with a graph and then baked again with the channels read through its commit
- **THEN** every L tile of the second bake equals the first byte for byte
