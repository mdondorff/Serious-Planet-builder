## ADDED Requirements

### Requirement: GEN-029 The region mask
The stamp stage (GEN-026) of a layer SHALL leave the height at a point as the base's when a region of mode 2 filed in that layer (WDEF-012) contains the point: every delta of the layer's items (hill, crater, plateau, ridge and channel, GEN-028) is cut there, whatever the order of the items; the edge is hard (a delta is kept or removed per point, never scaled in between). Where no region contains the point the stage is what it was. A region changes no height itself and a region of mode 0 changes nothing. The delta range of a stage over a cap (the bounds of GEN-026, ADR 0067 point 4) SHALL be that of the unmasked stage widened to hold 0 when a region of mode 2 is among the items that meet the cap (a point the region cuts has no delta at all). The pass-through of a tile with no item and no region in its footprint, the key chain, and the output being unchanged beyond `reach_m` of every item and region are those of the stamp stage; a region re-keys the tiles of its layer that its reach touches and no others.

Verify: A
Status: active
Source: ADR 0069, ADR 0067, GEN-026, M6 plan row E5, owner (2026-10-19)

#### Scenario: Inside
- **WHEN** a hill reaching across the edge of a mode-2 region and a ridge crossing it are applied
- **THEN** the height inside the region is the base's, the height outside carries the hill and the ridge, a region of mode 0 changes nothing, and the result does not depend on the order of the items

#### Scenario: Other layers
- **WHEN** a region clips layer A and a hill lies in layer B over the same place
- **THEN** layer B's hill is applied in full

#### Scenario: Bounds
- **WHEN** a mode-2 region overlaps stamps and lines and the caps around them are sampled densely
- **THEN** every sampled masked height lies within the bounds of the stage, and a point inside the region is the base's bit for bit

#### Scenario: Contract
- **WHEN** the stage runs on a tile with a recording reader
- **THEN** every region it touched is in the footprint the tile declared, a tile away from every item and region keys and renders as its source, and editing a region re-keys the tiles of its layer that it touches and no others
