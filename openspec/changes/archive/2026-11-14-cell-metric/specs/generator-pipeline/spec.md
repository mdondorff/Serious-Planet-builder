## ADDED Requirements
### Requirement: GEN-138 The grids of the bake may carry the physical cell metric
A `Grid` over a window of a cube face SHALL be able to carry a physical metric: the area of each cell and the distance between neighbouring cell centres of the equiangular face (`TangentWarp`) in place of the uniform `dx^2`, `dx` and `sqrt 2 dx`.
- The area of a cell SHALL be the exact spherical quad: the solid angle of the gnomonic patch between its edge tangents times the radius squared; the cells of a face SHALL sum to a sixth of the sphere (relative error under 1e-12 at 257 cells an edge) and the cells along opposite edges of a face SHALL be equal (the equiangular map is symmetric, so faces that meet at a cube edge agree on their edge cells).
- The distance SHALL be the great circle between the centre directions; at the face's centre it SHALL be `dx` on the axes and `sqrt 2 dx` on the diagonals, at a corner about 0.94 `dx` on the axes, and `d(a, b) = d(b, a)`.
- The routing SHALL take the receiver slope, the multiple-flow-direction slope and the starting area of a cell from the metric, so that the area returned at the outlets is the sum of the cell areas; the carve SHALL take the stream-power distance and the breach slope from it.
- `CarveParams.cell_metric` SHALL be false by default, and a grid without a metric SHALL give the uniform values bit for bit; a layer baked with the switch on SHALL carry it in the layer key and one baked with it off SHALL keep its key.

Verify: A
Status: active
Source: SPIKE-105 §12 F5, inbox #97 (owner D1, 2026-11-08)

#### Scenario: The physical cell metric
- **WHEN** the cells of faces of 8, 64 and 257 cells are summed, the edge cells of opposite sides compared, the distances read at the centre and at a corner, a routing is run over a face with a halo and a carve twice with the metric and once without
- **THEN** the sums are a sixth of the sphere, the edge cells are equal, the distances are those above and symmetric, the outlets hold the total area, the two metric runs are the same bits and differ from the uniform run, and a grid without a metric gives `dx^2`, `dx` and `sqrt 2 dx` exactly
