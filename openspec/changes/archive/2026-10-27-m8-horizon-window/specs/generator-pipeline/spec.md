## MODIFIED Requirements

### Requirement: GEN-043 The coupled run through the climate views
`generators::coupled` SHALL draw the stored run onto a tile's climate texture (9 x 9 texels, GEN-021) so the viewer's climate views show the global river network: with the height lens the temperature channel holds the run's height (sea at -30 C, land from the height grey of the height debug view up to 40 C) and the precipitation channel is 0; with the area lens the precipitation channel holds the drainage area on the log scale of the area map (0 mm at one cell's area, 3000 mm at the largest area of the run) and the temperature channel is -30 C.

Verify: A
Status: active
Source: opus-planner (2026-10-24, M8-1 part c), ADR 0077

#### Detail
A texel's height is the run's at its cell; its area is the largest in the cells the texel covers, so a river a coarse tile cannot resolve still shows. `generators::chain::CoupledLensField` SHALL put the lens over any field: every height, key and reading is the base's, the climate is the lens's, and the climate version word (hence the mesh key of every tile) carries the run's identifier, the lens and the run's algorithm version. The texture is a pure function of the run, the lens and the tile. The lens's read records and its answer to whether it reads cells SHALL be those of the layer the key walk reaches below it at the tile (`key_source`, STRM-029), so a tile a stage passes through keys like the base's, never panics the key.

#### Scenario: What a tile shows
- **WHEN** the lenses are applied to the tiles of a run on a small lattice, to the whole face as one tile and to a fine tile
- **THEN** the height lens holds sea at -30 C and land above it, the area lens holds the largest area of a texel's cells (the main stem on the wet half of the scale from the coarse tile), the other channel is flat, and two draws are equal

#### Scenario: The lens field
- **WHEN** a field is wrapped in the lens
- **THEN** its heights are equal to the base's bit for bit, its climate is the lens's, and its mesh key differs from the base's and between the two lenses and between two runs
