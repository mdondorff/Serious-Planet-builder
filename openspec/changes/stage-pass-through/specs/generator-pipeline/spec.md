## ADDED Requirements

### Requirement: GEN-027 A stage with nothing to add is the identity
An authored stage SHALL say, for a tile, whether it passes the tile through (`HeightField::passes_through`); it MAY say yes only when its output for every sample of the tile, for the tile's climate and for the height bounds of a cap inside the tile is its source's (equal values; the samples bit for bit). A stamp stage (GEN-026) SHALL pass a tile through exactly when the footprint it declares for the tile (WDEF-007, with the stage's halo) is empty, and a wrapper of a field SHALL forward the answer with the chain. A stage that changes its output where no item reaches SHALL NOT claim to pass any tile through.

Verify: A
Status: active
Source: ADR 0068, ADR 0066 points 1 and 4

#### Scenario: Identity
- **WHEN** a stamp stage has no item in reach of a tile, and a coarse tile away from the stamp, and another over it
- **THEN** the first two are passed through and their samples, bounds and climate equal the base's bit for bit, and the third is not passed through
