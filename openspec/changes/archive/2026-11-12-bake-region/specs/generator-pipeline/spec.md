## ADDED Requirements
### Requirement: GEN-177 A region of a face is baked to a commit with a channel graph
A region bake SHALL take a rectangle of regional cells of one face, a base, a climate and a size spec, solve the regional surface, find the hydrology, produce the lakes and the channel graph, and write the L tiles of the land tiles, the basin tables, the graph blocks and the commit through the writer.
- A region within 4 regional cells of a face edge is refused.
- The result is the same bytes at any thread count and on a second run.
- No store format changes.

Verify: A
Status: planned
Source: Architect 2026-11-12 (item 15, G-RT fixture), STRM-084, GEN-170

#### Scenario: A fixture region
- **WHEN** a region of a small fixture face is baked
- **THEN** the commit holds a graph, its channels read back through the reader and its tiles are present
