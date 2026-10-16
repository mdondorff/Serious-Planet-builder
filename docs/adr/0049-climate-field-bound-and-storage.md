# 0049 The climate field's bound and storage (amends ADR 0039 point 2), and the look file's version rule
Status: proposed (sonnet-explorer, M5 item 4, 2026-10-16; the code of `sun-and-base-shading` is built on it; reversible: a storage change is a MESH_VERSION bump)
Date: 2026-10-16
Source: ADR 0039 point 2; ADR 0027; ADR 0033 points 2-4; ADR 0035; constitution CON-07, CON-14; the planner's review of the look file change (#203)

## Context
ADR 0039 point 2 describes the climate field as "9x9 texels, two f16 channels, 324 B ... CON-14's bound (abs 0.01 degrees C and 0.1 mm) ... part of the compact tile (MESH_VERSION bump; ADR 0033's headroom pins re-checked with the new bytes)". Building it showed three things the text does not fit:
1. f16 has 11 significant bits. Its step is 0.0156 degrees at 16-32 C, 0.031 above 32 C, 2 mm at 1,024-2,048 mm and 4 mm above 2,048 mm, so "abs 0.01 degrees and 0.1 mm" against the bake cannot hold for f16 storage.
2. CON-14's bound compares two implementations of one field. The climate has one producer (the CPU) and the GPU only decodes the stored value.
3. Adding 324 B to a compact tile would grow the slot (8,840 B) and force a re-pin of every compact slot count; the compact texel has a reserved 16-bit channel that is zero.

## Decision
1. **The field is the f16 rounding** (round to nearest, ties to even) of the ADR 0027 reader's f32 value. The CPU computation is the definition; the GPU decode (`unpack2x16float`) is exact, so the CPU/GPU bound of CON-14 for this field is 0. Against the bake the declared quantisation is 2^-11 of the value (at most 0.016 degrees below 32 C and 4 mm of precipitation in the field's range). The values "abs 0.01 degrees and 0.1 mm" of ADR 0039 are replaced by this.
2. **Storage.** A compact tile keeps the field in the fourth (reserved) channel of its first 162 texels (temperature `k`, precipitation `81 + k`), so its size stays 8,712 B of texels plus the 128 B record and ADR 0033's compact slot counts and pins are unchanged (the re-check is a test that the slot bytes did not move). A compact tile therefore needs at least 16 cells per tile edge (the CLI refuses `--cells` below 16 with the compact format). A vertex-format tile (levels 0-2, CON-13 fallbacks, the reference format) carries the 324 B as its own block in the persistent payload (payload version 2) and its GPU vertex gains the field's value at the vertex as two f16 (48 B instead of 44 B; 58,608 B per tile at cells 32), so the vertex-format slot counts and their fractions of ADR 0033 point 2 are re-pinned (34,304 and 52,480 slots; R at most 45 % and 30 %), the per-frame upload of the vertex format fits 17 tiles and the node record grows from 192 to 208 B (the tile origin's up vector and the inverse of its distance from the planet centre, for the slope input).
3. **Keys.** The climate version word (the climate version, `READER_VERSION` and `GLOBAL_VERSION`) is part of every mesh key and persistent mesh key (output parameter 6). MESH_VERSION is 7. Tiles stored under version 6, including a pack's pre-filled coarse levels (STRM-021), are unreachable and must be written again.
4. **The look file's version rule** (the planner's review of #203): every change that adds a table or a key to the look file raises `schema_version` and adds an identity migration step, so that an older build says "newer schema version N" and not "unknown key". Schema version 2 adds `sun` and `albedo`.

## Consequences
- ADR 0039 stands except point 2's bound and byte description, which this ADR replaces.
- A viewer that wants cells below 16 uses the vertex format.
- The vertex format is the reference and fallback; its 9 % growth is inside its share (the compact format, the production one, did not grow).

## Licences of adopted code or techniques
None. Albedo values are public measurement ranges (Oke, *Boundary Layer Climates*, 2nd ed. 1987).
