# 0026 Global-coarse field representation and bake layout
Status: proposed (sonnet-explorer, 2026-10-12; easy to reverse until a bake is persisted; for the architect to accept under the owner's delegation or send back)
Date: 2026-10-12
Source: m3-global-fields (GEN-013, GEN-014); spec-guardian and numerics reviews of it; ADR 0006, ADR 0019, ADR 0022, ADR 0023 point 10; constitution CON-06, CON-07, CON-08, CON-14, CON-20, CON-21; architecture report §5 and the M3 row

## Context
M3 needs a global-coarse bake (continents, elevation, climate) that regional fields later read, a persistent form of it, and a 1 km bake in under ten minutes. m3-global-fields built the field and an in-memory bake. The choices below fix what is stored and read; they are cheap to change until the first bake is persisted (the next change) and regional fields read it.

## Decision
1. **One field, four channels.** Continentalness, elevation, temperature and precipitation are one field `global-coarse` with one version and one implementation id (`cpu-global-coarse-v1`). Under ADR 0022 option 1 (GEN-007: a stage reads only earlier stages, which includes the global stage) the channels cannot be separate same-stage fields while they feed each other. Splitting them per channel needs ADR 0022 option 2 and the owner's CON-08 reading.
2. **Grid.** Vertex-centred, `(res + 1)^2` samples per cube face with the edge samples stored on both faces (equal bits, GEN-013), `res` a power of two, at the vertices `TileId::sample_direction` gives for a level-0 tile (the same lattice as tile samples of any level). A bake finer than the in-memory limit is a set of tile-aligned blocks of this lattice, not one array.
3. **Channels.** `f32` each: continentalness (about -1.7 to 1.7, dimensionless; land is above `SEA_LEVEL_C = 0.10`, decided from the stored `f32`), elevation in metres above the reference surface (-9000 to 9000, continuous across the coast), mean annual temperature in degrees Celsius, mean annual precipitation in millimetres (0 to 4500). The sea level is part of the contract: moving it is a version bump.
4. **What "1 km" means.** At Earth's radius a face edge is about 10,000 km, so 8192 cells are 1.2 km and 16384 are 0.6 km. The M3 criterion ("global 1 km bake <= 10 min") is read as 16384 cells per face edge (6 x 16385^2 = 1.6 G samples), baked as tiles and never held whole (6 x 16385^2 x 16 B is 26 GB). The cost per sample is about 4 us per core (estimated, not measured on the reference machine): 6400 core-seconds, 320 s on 20 threads. GEN-014 holds the clause (tier C, measured by the architect).
5. **Reading.** Regional fields read the bake by bilinear interpolation in face coordinates on the lattice of the coarsest block that covers the tile plus a one-sample margin (so a tile never needs samples beyond its face neighbours' blocks); the interpolation is exact `+ - * /`, and the bake's block keys enter the reader's key as one upstream key per block (ADR 0022: "one key per input field" is revisited here: a tile that spans blocks lists them all).
6. **Keys.** The bake's output key is a persistent 128-bit stage key under its own tag, version and known answers (ADR 0023 point 10), including the field id, version, implementation id, seed (through the read set), face-mapping id and `res`. The face-mapping id belongs in every key: `bake` takes any `FaceMapping`, and `grid_hash` does not cover it.
7. **Reads through a context.** `global::sample(seed, d)` takes a seed. Before a bake is keyed, a generator wrapper gets the seed through `GenContext` per body (CON-06), with a contract test that generation under a recording context reads only what it declared (ADR 0021 point 7).

## Consequences
- The persistent form is a new record kind in the tile store (ADR 0019/0024 packs): payload codec like STRM-009 (shuffled `f32` planes, LZ4), kind byte 3, node set id per ADR 0025 point 3 (the global field reads only the world seed).
- A change of any formula is a `GLOBAL_VERSION` bump and new known answers (tier A; the test asserts the recorded version).

## Licences of adopted code or techniques
None.
