## ADDED Requirements

### Requirement: GEN-040 The global coupled run
`generators::coupled` SHALL run stream-power landscape evolution on the cube-sphere bake lattice (ADR 0077 point 1): per step a priority flood fills the depressions, each land cell takes the steepest receiver down the filled surface with a jitter `1 + j (2 h - 1)` (`h` a hash of seed, cell and direction), the drainage area is accumulated downstream, and the heights take the implicit update `z = (z + dt U + f z_r) / (1 + f)`. The run SHALL be single-threaded in a fixed order with `libm`, so its results are bit-identical across runs and platforms (CON-14); every receiver is lower on the filled surface; the outlets drain the land area.

Verify: A
Status: active
Source: opus-planner (2026-10-23, M8-1), ADR 0077, SPIKE-33, SPIKE-35

#### Scenario: Network, area and determinism
- **WHEN** a 12-cell lattice with a 60 % land world runs 30 steps twice, with jitter 0 and 0.8
- **THEN** no receiver drains uphill and no chain cycles, the outlets' area equals the land area, both runs agree bit for bit, the mean change per step falls, and the jitter changes the receivers

### Requirement: GEN-041 The coupled run's debug maps
`generators::coupled` SHALL draw the heights and the drainage area of a face of the coupled run as images: heights as the grey of the height debug view (sea at or below 0 in blue), the area on a log scale from the area of one cell (dark) to the largest area on the face (bright), so the river network reads as the bright lines. The maps are a pure function of the arrays.

Verify: A
Status: active
Source: opus-planner (2026-10-23, M8-1 part c), ADR 0077

#### Scenario: What the maps show
- **WHEN** the maps of a run on a small lattice are drawn
- **THEN** a sea cell is blue, a land cell is grey by height, the cell with the largest area is the brightest and a cell with one cell's area the darkest, and two draws are equal

### Requirement: GEN-043 The coupled run through the climate views
`generators::coupled` SHALL draw the stored run onto a tile's climate texture (9 x 9 texels, GEN-021) so the viewer's climate views show the global river network: with the height lens the temperature channel holds the run's height (sea at -30 C, land from the height grey of the height debug view up to 40 C) and the precipitation channel is 0; with the area lens the precipitation channel holds the drainage area on the log scale of the area map (0 mm at one cell's area, 3000 mm at the largest area of the run) and the temperature channel is -30 C. A texel's height is the run's at its cell; its area is the largest in the cells the texel covers, so a river a coarse tile cannot resolve still shows. `generators::chain::CoupledLensField` SHALL put the lens over any field: every height, key and reading is the base's, the climate is the lens's, and the climate version word (hence the mesh key of every tile) carries the run's identifier, the lens and the run's algorithm version. The texture is a pure function of the run, the lens and the tile.

Verify: A
Status: active
Source: opus-planner (2026-10-24, M8-1 part c), ADR 0077

#### Scenario: What a tile shows
- **WHEN** the lenses are applied to the tiles of a run on a small lattice, to the whole face as one tile and to a fine tile
- **THEN** the height lens holds sea at -30 C and land above it, the area lens holds the largest area of a texel's cells (the main stem on the wet half of the scale from the coarse tile), the other channel is flat, and two draws are equal

#### Scenario: The lens field
- **WHEN** a field is wrapped in the lens
- **THEN** its heights are equal to the base's bit for bit, its climate is the lens's, and its mesh key differs from the base's and between the two lenses and between two runs

### Requirement: GEN-056 The coupled run's lattice is independent of the bake's
`generators::coupledinput::reduce_bake` SHALL give the run's starting heights on a lattice of `n` cells per face edge from a bake of `res` cells per face edge, `n` dividing `res` (anything else refused with both numbers). A bake cell holds the mean of its four corner vertices and a run cell the mean of the `(res / n)^2` bake cells it covers, summed in `f64` in a fixed order and rounded to `f32` once, so the result is bit-equal for any thread count and block structure of the source (CON-14). The run SHALL start from the reduced heights exactly as from the same heights given directly. The world's run SHALL use `min(res, 2048)` cells per face edge.

Verify: A
Status: active
Source: opus-planner (2026-10-24, M8-1 (c) rest, owner: solid software), ADR 0077, CON-14

#### Scenario: Reduction and run
- **WHEN** synthetic bakes of 16 to 64 cells are reduced to runs of 1 to 16 cells per face edge, with 1 to 64 threads and blocks of 0 to 64 cells, and a run of 8 cells is made from bakes of 32 and 64 and from the heights reduced by the definition
- **THEN** every run cell equals the definition's mean, a plane reduces to the plane at the cell centre, all thread counts agree bit for bit, the two runs agree bit for bit in heights, area and receivers, a run that does not divide the bake is refused and a missing vertex is named
