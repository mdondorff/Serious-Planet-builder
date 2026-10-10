## Context
ADR 0003 review findings (branch change/adr-0003-review).

## Decisions
- Normals use a frame derived from the vertex direction, the second option named in ADR 0003 (the first was halo samples from the neighbour lattice, which needs cross-face lattice mapping).
- Tiles of different levels still have different normals at a shared border (different step). The shader does not morph normals, so this is a visible lighting discontinuity once shading exists: measured 0.094 degrees between levels 0 and 1, 0.037 (4 to 5), 0.010 (6 to 7) and below 1e-4 from level 10; invisible with the current smooth terrain, to be handled (morphed normals or a shared step) when lighting arrives.
- Cost: mesh generation is about 3.8x slower (4.3 ms instead of 1.1 ms per 64-cell tile, release build) because each vertex now needs five height evaluations; to be weighed against streaming throughput in M3/M4.
- corner_neighbors probes eight directions around the corner at 1e-3 of a tile; the tests establish ground truth without from_direction.

## Review follow-ups (verifier)

- Added a test that normals follow the terrain (agree with the lattice slope within 0.02 degrees and the terrain tilts them by more than 0.05 degrees) after two mutants (radial normal, level-blind step) survived; tie tests for the independent partition definition (cube edges, corners, dyadic boundaries, agreement with the library); a test pinning the corner labels to their face coordinates. Wording corrected (normals are not morphed; candidates unchanged on WARP).
