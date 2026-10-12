## ADDED Requirements

### Requirement: GEN-015 Reading the global bake
A reader SHALL give the global-coarse channels at a tile sample position `(k, l)` of tile `(x, y)` at level `L` with `m` cells per edge (one sample of halo allowed) from the integer lattice index `(x m + k, y m + l)` of a face lattice of `m 2^L` cells (a halo of at most one tile edge: `k` and `l` from `-m` to `2 m`; a lattice position farther out is an error, never a clamp), never by inverting a direction: with the bake's own lattice of `res` cells (both powers of two) the cell and the weight are exact dyadic functions of the integer, and a position on the bake's lattice is the stored sample itself (a neighbour whose weight is zero is not read). Interpolation SHALL be bilinear in the form `a (1 - w) + b w` (along s, along t, then across) on `f64` values of the stored `f32` channels, rounded to `f32` once, so that a position on a face edge, read from either face that has it (the neighbours swap and `w` becomes `1 - w`), gives equal values (a zero compares equal to a zero of the other sign), and the same position SHALL read the same bits through any tile decomposition. A position beyond a face edge SHALL hold the edge value (ADR 0027 point 4). A bad tile shape, an out-of-range halo and a source that cannot give a vertex SHALL be errors that name what is wrong, and the reader's version (`READER_VERSION`) and known answers SHALL be pinned. Land after interpolation is decided by the interpolated continentalness as `f32`; the interpolated elevation is the elevation.

Verify: A
Status: active
Source: CON-05, CON-08, CON-14, ADR 0006, ADR 0022, ADR 0026, ADR 0027

#### Scenario: Lattice points and edges
- **WHEN** a position on the bake's lattice is read at any target resolution, or a position on a face edge between bake samples is read from each face that has it
- **THEN** the stored sample comes back (zero sign aside) and the faces agree

#### Scenario: Partition invariance
- **WHEN** one face position is read through tiles of different levels and cell counts
- **THEN** the bits are equal

#### Scenario: Halo and errors
- **WHEN** a position one sample beyond a face edge is read, or a tile shape or source is bad
- **THEN** the edge value comes back, or the error names the shape or the missing block
