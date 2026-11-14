## ADDED Requirements
### Requirement: GEN-164 H0 read from a stored bake equals the in-memory H0 at cube edges and corners
H0 read from a stored bake SHALL equal the in-memory H0 within the store's quantisation step at the cells of a cube edge and of a cube corner: the height of an H0 cell is the C1 read of the field at the cell's centre, the rainfall and temperature the bilinear read, with samples beyond a face edge taken from the face it meets.
- The production block codec is lossless, so the step is 0 and the two reads SHALL be equal bit for bit on every cell, every edge cell's stencil reaching over its face edge.
- A block the store does not hold SHALL make the read an error, never a placeholder.

Verify: A
Status: planned
Source: Architect 2026-11-10 (item 3's stored-field read), GEN-139, GEN-107

#### Scenario: A stored bake across the cube edges
- **WHEN** a 16-cell bake in blocks of 4 is stored and the H0 cells of a 12-cell face edge are read from the store and from the in-memory bake
- **THEN** every height, rainfall and temperature of the 864 cells, 6 x 44 edge cells and 24 corner cells included, has equal bits, and a read from an empty store is an error
