## ADDED Requirements

### Requirement: GEN-029 The region mask
The stamp stage (GEN-026) of the layer a region names as `mask_layer` SHALL multiply the delta of each of its items at a point (hill, crater, plateau, ridge and channel, GEN-028) by 0 where a region of mode 2 contains the point or a region of mode 1 does not, and by 1 elsewhere; the factors of several regions multiply, so a point is cut when any of them cuts it. The edge is hard: a delta is kept or removed per point, never scaled in between. A region changes no height itself and a region of mode 0 changes nothing. The delta range of a stage over a cap (the bounds of GEN-026, ADR 0067 point 4) SHALL be that of the unmasked stage, which still encloses every masked delta. The pass-through of a tile with no item and no region in its footprint, the key chain, and the output being unchanged beyond `reach_m` of every item and region (a mode-1 region reaching every tile of its layer) are those of the stamp stage.

Verify: A, C
Status: planned
Source: ADR 0069, ADR 0067, GEN-026, M6 plan row E5, owner (2026-10-19)

#### Scenario: Inside and outside
- **WHEN** a hill reaching across the edge of a mode-2 region and across that of a mode-1 region is applied
- **THEN** the height inside the mode-2 region is the source's, the height outside the mode-1 region is the source's, the rest carries the hill, and a region of mode 0 changes nothing

#### Scenario: Other layers
- **WHEN** a region clips layer A and a hill lies in layer B over the same place
- **THEN** layer B's hill is applied in full

#### Scenario: Several regions and the bounds
- **WHEN** two mask regions overlap a stamp, and the caps around them are sampled densely
- **THEN** a point cut by either keeps no delta, and every sampled masked delta lies within the range the bounds give

#### Scenario: Contract
- **WHEN** the stage runs on a tile with a recording reader
- **THEN** every region it touched is in the footprint the tile declared, a tile away from every item and region keys and renders as its source, and editing a region re-keys the clipped layer's tiles it touches and no others
