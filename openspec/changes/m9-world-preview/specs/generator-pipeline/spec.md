## ADDED Requirements

### Requirement: GEN-103 The default class source and the whole-planet preview
`planet-generators` SHALL provide the default class source of ADR 0089 decision 3 as CPU fields behind the class-weights interface of decision 1: the plate mask (GEN-094), the memberships (GEN-095) and the base elevation without the hill band (GEN-096) as pure functions of the seed and the direction, the class weights as `u8` on the lattice of GEN-093 (bilinear reads, continuous across face edges), the untapered blended `k` (GEN-097) and the hill band's amplitude from `k` (GEN-098). They SHALL use `libm` and integer hashing only and give the same bits for any thread count (CON-14). The source SHALL NOT change any stored field or `GLOBAL_VERSION`; wiring it into the bake is `m9-bake`'s.
`cargo xtask preview world --seed N [--out DIR]` SHALL write, from those functions and no store, equirectangular PNGs of at least 2048 x 1024 under `target/preview/world-seed-N` or `--out`, in under a minute on the reference machine:
- land and sea; the elevation with hillshade; today's elevation with hillshade; both side by side;
- the plate mask with the plate margins; the plateau field and the back-arc;
- the relief class by largest weight and by the blended weights; `k`.
Every image SHALL carry the seed, `GLOBAL_VERSION`, `MESH_VERSION`, the class list and class source versions, the preview version, and a legend; the command SHALL also write a report of the class shares, the share of land above 2 and 4 km, the highest land and the largest step of `k` between lattice points.

Verify: A
Status: planned
Source: opus-planner (2026-11-01, inbox #37), ADR 0089 decisions 1, 3 and 10, GEN-093 to GEN-098; SPIKE-77, SPIKE-79, SPIKE-80, SPIKE-91

#### Scenario: Same seed, same bytes
- **WHEN** the lattice and the preview images of seed 1 are built on 1 and on 8 threads, and again for seed 2
- **THEN** every lattice point's weights sum to 255, the bytes of the lattice and of every image are equal between the thread counts, and seed 2 differs from seed 1

#### Scenario: Memberships and the blended k
- **WHEN** a point has core 1 and base elevation 3500 m, another has no mask term and plateau field 1, and a third has weights 128 hills and 127 mountains
- **THEN** the first holds weight 255 in high mountains, the second 255 in plateau, and the third's untapered `k` is `(128 x 5 + 127 x 6) / 255`
