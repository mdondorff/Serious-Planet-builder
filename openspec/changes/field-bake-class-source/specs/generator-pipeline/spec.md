## ADDED Requirements

### Requirement: GEN-115 The field bake can sample the class source
`global::class_sample(seed, d, inputs)` SHALL be the global-coarse sample at `d` whose elevation is the class source's (`BakeInputs::base`, GEN-096 and GEN-098), clamped to the legacy range:
- the continent value is the legacy sample's, bit for bit, and the shore (elevation above 0) is the legacy one;
- the temperature and the precipitation are the legacy climate formula over the class source's elevation;
- `bake_blocks_sampled` and `bake_block_sampled` bake any pointwise field into blocks; a block is the same bytes on any thread count and block size (CON-14); the legacy entry points are those functions over `sample(seed, d)` and give today's bytes;
- the class source's band is read through the class tables, so a change of its amplitude or shape is a parameter, not another code path;
- the read set of the legacy generator (GEN-013) is unchanged: a bake that takes the class source carries the source's identity in its key (STRM-073) beside the seed, and `GLOBAL_VERSION` and the implementation id stay the legacy generator's because the key tells the two apart.

Verify: A
Status: planned
Source: owner 2026-11-04 (planner inbox, change `field-bake-class-source`); ADR 0088, ADR 0089; depends on GEN-096 and GEN-098 of the m9-bake change

#### Scenario: The class sample
- **WHEN** 1,500 directions over the sphere are sampled with the legacy and the class field
- **THEN** the continent values are equal bit for bit, the sign of the elevation (land or sea) is equal at every point, the class elevation is `BakeInputs::base` as f32 and differs from the legacy one by more than 1 m over most land, the temperature is the formula's at the class elevation and the rain within 1 mm

#### Scenario: Blocks
- **WHEN** the 24 blocks of level 1 (4 cells) are baked from the class field on 1 and on 4 threads
- **THEN** the bytes are equal and the legacy sample of a direction is unchanged
