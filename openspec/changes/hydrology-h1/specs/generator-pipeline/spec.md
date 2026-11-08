## ADDED Requirements
### Requirement: GEN-145 H1 derives the channel graph and the basins from a routing
H1 SHALL derive from a routing (clipped MFD area, GEN-141) the channel graph and the basins, kept apart from the diffuse field.
- A cell with `A_d >= T` (the runoff-weighted diffuse area, `T` a parameter, default 75 km^2) SHALL start a channel; every D8 receiver of a channel cell SHALL be a channel cell or an outlet, and its filled level SHALL be lower (a channel continues downstream and does not restart).
- The channel discharge `Q(c)` SHALL be the local term `a(c) r(c)` plus `Q` of every D8 donor (zero local term in an outlet cell); the sum of `Q` at the outlets SHALL equal the sum of the local terms over the land to a relative error of 1e-9, and `A_d` at the outlets SHALL equal the sum of the (weighted) true cell areas of the cells that drain to them to 1e-9, also across cube edges.
- A segment SHALL run from a head or a confluence to the next confluence or the mouth, its id the hash of the body, its downstream end cell and its upstream end cell; every channel cell except the mouths SHALL be in exactly one segment, and every segment SHALL reach a mouth through its downstream segments.
- A basin SHALL be a connected set of cells whose filled surface stands more than `pit_min_m` above the ground, with its floor, its spill point (the rim cell of the lowest filled level), a 16-step area and volume curve, its class (lake with an active outlet, endorheic without) and water level by the steady balance, and an id that is a function of its lowest cell (ties by the global cell id) only. A basin below the area and the depth limits SHALL be an artefact, counted and not kept.

Verify: A
Status: planned
Source: A3 sections 5.2 to 5.5 and 12.1 (G-CON), inbox #99, ADR 0111 (proposed)
- The run on the prototype landmass (`examples/hydro_landmass`) is a measurement, not a gate; the gates are the tier-A tests.

#### Scenario: channels and basins
- **WHEN** lands of ridges and of terraces (with and without a runoff field), and a sphere of 24 cells a face edge with water crossing cube edges, are routed with the round rules and given to H1; and a land with a bowl is given strong and no net evaporation and edited
- **THEN** no segment is disconnected, no channel cell lacks a downhill channel receiver or an outlet, `Q` is conserved at confluences and at the outlets, a channel and a segment cross a cube edge, the bowl is one lake or one endorheic basin with its floor, spill level, curve and water level, its id moves only when its lowest cell does, and a one-cell dent is an artefact
