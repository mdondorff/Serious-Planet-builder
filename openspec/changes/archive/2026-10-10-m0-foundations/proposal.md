## Why

The project has a constitution and a seeded repository but no code, no command surface, no CI, no recorded decisions and no subsystem specs. Every later change needs: a layered workspace the constitution can be checked against, `cargo xtask` as the single command surface, a verification harness (goldens, spec-lint, performance protocol) so work can be falsified at the cheapest tier, and the twelve decide-now ADRs (as proposals) so work can proceed on their leanings.

## What Changes

- Cargo workspace with the crate chain `core → world-def → generators → cache → streaming → frame → render → editor → app`, plus `testkit`, `perf` and `xtask`; GPU-free crates verified to have no graphics dependency.
- `cargo xtask`: `test fast|(all)|gpu`, `check`, `layering`, `spec-lint`, `accept`, `perf` (owner-gated), `bless` (list only), `repro` (stub until M1).
- Headless "hello triangle" on the software adapter with per-adapter goldens (candidate only; blessing is the owner's) and the three run modes of the single binary.
- Performance-harness skeleton (adapter check, clock and power logging, warm-up, medians, p99, JSON log); real runs need the owner's go.
- CI for Windows (WARP) and Linux (lavapipe), licence policy via `cargo deny`.
- Twelve ADRs in status `proposed` (coordinates, depth, topology, language, environment, generator purity, authored layers, app structure, testing, determinism, reference surface, spec tooling).
- Seed specs for the ten subsystems, with trace IDs and tiers. Requirements that M0 implements are `active`; the rest are `planned` and become active in the change that implements them.

## Capabilities

### New Capabilities
- `build-tooling`: workspace layering, xtask command surface, run modes, CI and licence policy.
- `testing`: verification tiers, goldens, image metrics, spec-lint, performance protocol, repro bundles, FramePlan snapshots.
- `coordinates`: frames, reference surface, geo conversions, cube-sphere addressing, hashing and numeric determinism rules.
- `world-definition`: WorldDefinition as single source of truth, schema versioning, hashing, read recording, invalidation.
- `generator-pipeline`: pure staged generators, determinism and metamorphic properties.
- `terrain-lod`: CPU node selection and morphing on the cube-sphere quadtree.
- `streaming-cache`: injected time and I/O, parent-first availability, content-addressed cache, budgets.
- `authored-layers`: geo-anchored, versioned authored content, freeze/bake, undo.
- `renderer`: FramePlan execution, depth, precision, shader validation, debug views.
- `atmosphere-ocean`: atmosphere LUTs, aerial perspective, exposure, shoreline.

### Modified Capabilities
None (first change).

## Impact

New directories `crates/`, `xtask/`, `.github/workflows/`, `perf/`, `docs/adr/*`; new files `Cargo.toml`, `deny.toml`, `rustfmt.toml`. No behaviour of earlier code is affected because none exists. Constitution rules covered: CON-01, 02, 04 (seam only), 16, 17, 18 (harness), 20, 21 (protocol), 24, 25, 26, 27.
