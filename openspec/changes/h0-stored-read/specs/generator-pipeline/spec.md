## ADDED Requirements
### Requirement: GEN-164 H0's base can be read from a field source
The preview base SHALL be buildable from a field source (a stored bake or an in-memory grid): the height of each H0 cell is the C1 read of the source at the cell's centre, the rainfall and temperature the bilinear read, with samples beyond a face edge taken from the face it meets.
- The base read from a stored bake SHALL equal the base read from the in-memory bake within the store's quantisation step (the stored block codec is lossless, so 0), on every cell, including every cell of a cube edge and of a cube corner, and the H0 routing of both SHALL be one routing.
- A source that cannot answer a read SHALL make the build an error, never a placeholder.

Verify: A
Status: planned
Source: Architect decision D11 and inbox #123 (item 3's stored-field read), GEN-139, GEN-107

#### Scenario: A stored bake across the cube edges
- **WHEN** a 16-cell bake in blocks of 4 is stored and H0 bases of 12 cells a face edge are built from the store and from the in-memory bake
- **THEN** every height and rainfall of the 864 cells, 6 x 44 edge cells and 24 corner cells included, has equal bits, the receivers, areas and outlets of the two routings are equal, and an empty store is an error
