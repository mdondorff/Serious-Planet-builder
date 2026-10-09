## Context

ADR 0006 (purity, versioning), ADR 0010 (determinism), report §5 and §16. The generator reads no WorldDefinition yet (read recording arrives with `world-def` in M3); its only inputs are `seed`, `FaceMapping` and tile id.

## Decisions

- **Point function, not tile function.** Height is a pure function of `(seed, unit direction)`. A tile is a lattice of such samples taken from `TileId::sample_direction`, whose directions are bit-identical for shared borders and parent/child lattices (change m1-coordinates). Border agreement and partition invariance therefore hold by construction and are tested, not assumed.
- **No transcendentals at all** in the generator: integer lattice hash, smoothstep, lerp, `libm::floor`. The noise contributes no platform dependence; `libm` is only used for `floor`.
- **Threads:** `generate_region` splits the id list into contiguous chunks, joins in order, so output order and values equal the single-thread run.
- **Versioning:** `GENERATOR_VERSION = 1`, `IMPLEMENTATION_ID = cpu-ref-height-v1`, tile content hash includes both and `HASH_VERSION`. The committed known-answer hashes fail loudly when output changes.
- **Dumps** live in `generators` (they are CPU data products); the face net colours each pixel by the face found from its own 3D direction, so a wrong mapping shows as a wrong colour.

## Risks

Value noise is visually crude; it is a test generator, not the final terrain (M3 replaces/extends it with a new generator version).
