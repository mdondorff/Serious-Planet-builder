## ADDED Requirements

### Requirement: GEN-031 The wet floor
The generator SHALL compute for every vertex of the global bake its wet floor (ADR 0071): the minimum over the paths from the ocean to the vertex, on the graph of the 8 neighbours of the cube-sphere lattice (copies of one node on a shared face edge merged), of the maximum code on the path. The code of an ocean vertex SHALL be 0, that of a vertex of finite elevation `e` SHALL be `clamp(floor(2 e) + 24000, 1, 0xFFFE)` (an infinite elevation clamps, a NaN elevation gives 0xFFFF and is never crossed), and the floor of a vertex no path reaches SHALL be 0xFFFF. The code of a bound `b` in metres SHALL be `bound_code(b)`, the same expression rounded down and clamped, with 0 for a NaN bound, so that a vertex of elevation at most `b` has a code at most `bound_code(b)`. The tiled computation SHALL use three passes: per block a union-find that emits the edges at which components of the block merge (the block's border nodes and a virtual ocean node as terminals), one global pass over those edges in weight order that gives the floor of every border node, and per block a bucketed priority flood seeded from the block's ocean vertices and the border floors. The result SHALL equal the serial priority flood over the whole bake bit for bit, for every block size and thread count (CON-14). The channel SHALL be a function of the ocean bits and the elevations alone, with no dependence on the sea level or the ocean area threshold beyond the ocean bits themselves.

Verify: A
Status: active
Source: ADR 0071 point 3, SPIKE-21, GEN-021

#### Scenario: Equal to the serial flood
- **WHEN** random grids of 4 to 32 cells with NaN elevations and ocean vertices, and real bakes of 64 and 128 cells, are computed in blocks of every power of two from 1 and on 1 and 3 threads
- **THEN** every vertex's floor equals the serial reference's

#### Scenario: A ridge decides the floor
- **WHEN** a ridge of 90 m crosses a flat plain of 10 m with the ocean on one side
- **THEN** the vertices on the ocean's side have the code of 10 m, those on the ridge and beyond it the code of 90 m, and a bound of 100 m is above the far side's code, a bound of 64 m below it

#### Scenario: Lagoons, diagonals and pits
- **WHEN** a lagoon lies behind a 40 m barrier joined to the ocean only diagonally, and a pit is enclosed by a ring of NaN elevations
- **THEN** the lagoon's floor is the barrier's code, the pit and its surroundings inside the ring have 0xFFFF, and a NaN is never crossed

#### Scenario: Codes
- **WHEN** elevations of 0, 0.4, 0.5, -0.1, NaN and infinities and bounds from -740 to 740 m are coded
- **THEN** the codes round down, clamp inside 1 to 0xFFFE, keep the sentinels and are monotone, and a vertex at or below a bound has a code at or below the bound's
