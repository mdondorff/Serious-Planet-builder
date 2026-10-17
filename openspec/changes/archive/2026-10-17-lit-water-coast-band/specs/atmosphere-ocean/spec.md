## MODIFIED Requirements

### Requirement: ATMO-012 Water tiles are chosen by a per-tile CPU query at plan time
The nodes of a frame that carry water SHALL be chosen on the CPU (CON-03) by asking the water field W (GEN-023) for the `(4 + 1)^2` sample directions of each node's tile, memoised per tile (`WaterTileCache`): a tile carries water when any sample reports water or by the coastal band (ATMO-019), and its water is its most frequent sample's body (ties: the lower body id), so a tile holds one level and one liquid; nothing is stored per tile and `MESH_VERSION` and the compact slot pins are unchanged (ADR 0051 point 6, option 1). The answer of a tile SHALL be computed in the tile's worker job (the lit path of a world with water) and kept in a `WaterBook`, which the plan's cache reads first (ADR 0056): a plan whose tiles were loaded by their jobs asks the field nothing and reads no store block on the frame thread; a tile with no entry is computed on the plan thread and counted (`queries`, `tile_misses`, and `band_queries` for the band's reads). A cancelled job computes none. The book holds one entry per tile loaded (a few bytes, never evicted) and a tile job spawns up to eight short threads to load the tile's store blocks in parallel. The answer is a pure function of the tile and the field, so it does not depend on the thread or the order (CON-14). The plan SHALL carry, per water node, the terrain node's index, the level (rounded once to f32), the liquid slot and the waves' reduced phases, and the liquids' appearance (at most four per frame); an unknown liquid, more than four liquids and a level that is not a finite height within 1e5 m SHALL be refused naming the id, the numbers or the tile. A plan without water SHALL keep its text and hash; the text form of one with water SHALL round-trip exactly. Known limit of option 1: a dry basin below the level inside a tile that also holds sea is drawn flooded, and a lake smaller than the sample spacing of a coarse tile can be missed at that tile's level. Class: the selection is plan data (no GPU twin); the level, the body and the kind are fields read from W.

Cost of a query on the plan thread, measured ad hoc on an in-memory bake (not a performance verdict, CON-20): 3.6 us per new tile (0.14 us per field query, 25 queries per tile) for the 7,536 tiles of the scripted views at bake sizes 256 and 1024, 0.01 us per tile for a warm plan; over a stored bake a tile of level 0 to 3 touches up to 25 blocks of about 20 ms each (window 6: 8 s frames), which is why the jobs do it.

Verify: A
Status: active
Source: ADR 0041 point 8, ADR 0051 point 6, CON-03, CON-04

#### Scenario: Selection
- **WHEN** a plan of a coastal view is made over the reference sea
- **THEN** the water nodes are exactly the nodes with a wet sample, the first plan asks 25 queries per tile and a second plan asks none and is identical, and a later time moves the phases and nothing else

#### Scenario: Tile jobs fill the book
- **WHEN** the jobs of a set of tiles ran over a world with water on the lit path, and the plan's cache then answers the same tiles
- **THEN** it makes no field query and no miss (every tile is a book hit), the answers equal those of a cache with no book, a cancelled job leaves no entry, and a world not on the lit path computes none

#### Scenario: Two bodies, a missing liquid
- **WHEN** a tile's samples hold two bodies, a liquid the table does not define, five liquids, or a level of 6,000 km
- **THEN** the tile takes the more frequent body (a tie the lower id), and the others are refused naming the liquid id, the numbers or the tile

#### Scenario: Text form
- **WHEN** a plan with water is written to text and read back
- **THEN** it equals the original, a plan without water has no water line, and a damaged line is refused

### Requirement: ATMO-013 Gerstner waves with phases reduced on the CPU
The waves SHALL be plane waves in the planet-fixed frame (ADR 0041 point 5): each liquid has up to four, with the wave vectors `k_i = 2 pi / lambda_i x D_i` (the directions the vertices of a regular tetrahedron, wavelengths 1, 0.62, 0.38 and 0.23 of the liquid's, amplitudes 1, 0.55, 0.3 and 0.15 of its wave height, dispersion `omega = sqrt(g k) / (1 + viscosity)`); the CPU SHALL reduce `(k_i . O + phi_i - omega_i t) mod 2 pi` in f64 per tile origin `O` and plan time `t` (the injected clock, CON-04) and the shader SHALL add only `k_i . local`, `local` the tile-local position of the surface point, so a planet-sized number meets a wave vector nowhere in a shader (CON-11). Two tiles that hold one point therefore agree about its phase, across face edges too (no per-tile tangent projection). A wave's weight is the product of four continuous factors: its tangent share (a smoothstep from 0.25 to 0.6 of the length of its direction's part along the surface, so a wave pointing out of the surface has none), its resolution (a smoothstep from four to eight vertices per wavelength at the vertex's own spacing, the node's sample spacing times one plus the vertex's morph factor, so a wave the grid cannot hold fades out, continuously across LOD borders and also over deep water where the grid is spaced for the seabed's distance) the pixel footprint (a smoothstep from two to four pixels per wavelength, `PIXEL_FADE`, of the wavelength over the size of the pixel on the surface, the larger of the screen-axis derivatives of the tile-local position, in the fragment stage only: a wave under about two pixels per wavelength fades out of the shading instead of aliasing; the vertex stage has no footprint and no fade, ADR 0056) and the shore (a smoothstep from 0 to four times the sum of the amplitudes of the depth, so the surface never dips below the terrain where the water is thin: the vertical displacement is at most 0.57 of the depth). Liquids with no wave height (lava) have no waves. Class: presentation data (ADR 0035; no GPU twin, never stored or keyed). Bound: displacement and slope against the f64 reference on the same f32-rounded inputs and CPU-reduced phases within `1 mm + 1e-3 x (sum of the amplitudes)` per component (ATMO-014), with the footprint included (footprints from 2 cm to 500 m, tier B); the largest `|k . local|` of a wave with weight over the scripted views from orbit to 1 m is asserted below 250 rad (measured 17 rad) against a limit of 1,000, and by construction it is at most about 1.3 cells (so the f32 argument keeps its digits).

Verify: A
Status: active
Source: ADR 0041 point 5, ADR 0043, CON-11, CON-14

#### Scenario: Known answers
- **WHEN** the tile phase of a wave of `k = 0.1` rad/m at a planet-sized origin is reduced at t = 0, 1.5 s and 1e6 s
- **THEN** it equals the 50-digit value within 1e-8 rad, lies in `[0, 2 pi)`, and repeats after a thousand periods within 1e-6

#### Scenario: Seams
- **WHEN** a point of the shared border of two tiles, also across a face edge, is evaluated from each tile's origin with f32-rounded local numbers
- **THEN** displacement agrees within the bound and slope within 1e-3

#### Scenario: Pixel footprint
- **WHEN** the weights are taken at footprints from a fraction of a wavelength to many wavelengths
- **THEN** a wave is whole from four pixels per wavelength, gone under two, monotone and continuous between, and a footprint of 0 changes nothing; the shader agrees with the reference, and a far grazing pixel (115 m along the view) has no wave left

#### Scenario: Weights and the shore
- **WHEN** the weights are taken at spacings around four to eight vertices per wavelength, under an up that is a wave's own direction, and at depths from 1 mm to 40 m
- **THEN** they are continuous, a wave on a grid of fewer than four vertices per wavelength or out of the surface has none, and the vertical displacement never exceeds half the depth

## ADDED Requirements

### Requirement: ATMO-019 The coastal band: the sea covers the terrain below the level that the ocean reaches
A tile with no sea of its own SHALL carry the sea when one of its 25 samples is band and an ocean vertex of the bake lies within the reach of the tile's centre (ADR 0056). A sample is band when the bake calls it land (continentalness above `SEA_LEVEL_C`) and its bake elevation is at most `B(W)`, the highest elevation at which flat ground can still be cut below the level: the fixed point of `E = W + 0.75 A(E)`, `A(E) = (80 + 900 smoothstep(-200, 300, E)) x 0.15` (the erosion's amplitudes at the carve floor of flat ground, 0.75 the lower bound of the crease sum); `B(0)` is about 64 m (63.9), and `B` rises with `W` faster than `W`. Steep ground (slope factor above the floor) is not covered and can stay dry where it dips below the level. The reach is 100 km counted from the nearest point of the tile (100 km plus 0.71 of the tile's edge from its centre), so a parent is wet wherever its children are; it is probed on a grid of 51 by 51 directions over the disc, a spacing of reach/25 (4 km at the finest tiles), so an inlet narrower than the spacing can be missed. The wet tiles only grow with `W`. An inland basin has no ocean vertex within the reach and stays dry (owner constraint 2). The pass floods per vertex where `W - h_morph > 0` (ATMO-004). The coast views of the lit scenes and the capture SHALL stand at a beach found by the terrain's height (sea 2 km out with the plan carrying the sea and the terrain at least 5 m under the level, land 500 m in at least 3 m over it, the crossing of the level bisected, `find_coast`). Class: plan data (no GPU twin), a function of the bake, the level and the tile (CON-14).

Verify: A, C
Status: active
Source: ADR 0056, ADR 0041 point 2, ADR 0053, window 6 captures

#### Scenario: Coverage oracle on a real world
- **WHEN** an aerial (2 km up) and a low (30 m) view from the sea towards a coastal plain below the level, and a view at a beach, of the in-memory eroded world of 1,024 cells are drawn in the class view, and every pixel whose CPU ray meets the sea sphere where the terrain is under -12 m and an ocean vertex lies within 100 km is compared
- **THEN** at most 1 % of them are not water (measured 13,121 of 15,458 on the selection of ADR 0053, 0 of 15,458, 16,320 and 372 on the band), and at most 1 % of the pixels over terrain above +12 m are water

#### Scenario: Band, basin and level
- **WHEN** tiles next to an ocean, beyond the band, and in a basin with no ocean vertex are asked, and the level rises
- **THEN** the first carries the sea at the sea's level, the others are dry, the set of wet tiles only grows with the level, and the answer does not depend on the order of the questions

#### Scenario: The dip
- **WHEN** the dip is computed
- **THEN** `B(0)` is 63.9 m within 0.5, and for levels from -300 m to 3,000 m `B` is a fixed point (the cut at `B` reaches the level), ground just above it stays above the level, and `B` rises with the level

#### Scenario: Beach
- **WHEN** a world has a plain below the level east of its sea and a beach further on
- **THEN** the coast is at the beach, where the terrain crosses the level, and the plain's edge is no coast where the sea does not reach 2 km out
