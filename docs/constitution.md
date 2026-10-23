# Constitution

**Status:** v1.5, 2026-10-23 (CON-23: ADR maintenance; amended by the owner, ADR 0078). v1.4: 2026-10-15 (CON-11: geometry strict, presentation shading by tested bound; amended by the owner, ADR 0043). v1.3: 2026-10-15 (CON-07 and CON-14 scoped to fields; presentation data carved out; amended by the owner, ADR 0035). v1.2: 2026-10-11 (CON-18 golden approval amended by the owner, ADR 0017). v1.1: 2026-10-10 (CON-14, ADR 0010). v1: 2026-10-09. Owner: Matthias Dondorff.

This file holds the project's non-negotiable rules. Every rule has an ID and a verification tier (A pure, B CPU oracle, C pixels, D human; see `openspec/specs/testing`). Rules are deliberately short; the reasoning lives in the ADRs and in `docs/background/architecture-report.md`.

**Precedence:** this constitution > accepted ADRs (`docs/adr/`) > OpenSpec specs (`openspec/specs/`) > background docs (`docs/background/`, frozen, non-normative). When two sources conflict, the higher one wins and the lower one is corrected (background docs are never corrected; they are history).

**Amendments:** only by the owner. Claude may propose an amendment (as an ADR with status "proposed" that names the rule) but never edits this file on its own.

## 1. Architecture and layering

- **CON-01** One binary with three run modes (editor/viewer, `generate` headless CLI, `test-render` headless harness) built on shared library crates. *Verify: A* (all modes build and start in CI).
- **CON-02** Crate dependencies point downward only: `core → world-def → generators → cache → streaming → frame → render → editor → app`. `core`, `world-def`, `generators`, `cache`, `streaming` and `frame` compile without any graphics crate. *Verify: A* (CI builds them with no GPU dependency in the tree).
- **CON-03** The renderer executes a `FramePlan` and makes no decisions about what to draw. Terrain node selection (which nodes, which LOD, morph factors) is a pure CPU function in `frame`. GPU-side culling is allowed only for instances and only behind a differential test against a CPU reference. *Verify: A, C.*
- **CON-04** Streaming and scheduling code receives time, I/O and task spawning through injected interfaces, so it runs deterministically in tests with fakes. *Verify: A.*

## 2. World description and generation

- **CON-05** The WorldDefinition is the single source of truth. Every runtime representation (tiles, meshes, textures, instances) is a disposable cache that can be regenerated from it. *Verify: A* (`cache-verify`).
- **CON-06** Generators are pure functions of `(definition read-set hash, generator_version, implementation_id, tile_id)`. They read inputs only through a read-recording context; no global state, wall-clock time or unseeded randomness. *Verify: A.*
- **CON-07** Within one cache, each field layer is produced by exactly one implementation. The CPU implementation is the canonical reference; a GPU implementation must match it within the tolerance in CON-14. A *field* is data that a generator produces, that the cache stores or keys, or that CPU systems (collision, picking, placement, anchors) or other fields read; data the GPU derives only for display is presentation data, not a field (CON-14). *Verify: B.*
- **CON-08** Generation is staged: global-coarse → regional (per tile with a halo) → local (per point). A stage reads only outputs of earlier stages. *Verify: A* (metamorphic tests: border agreement, partition invariance, coarse/fine agreement).
- **CON-09** Authored content is stored separately from the procedural base, anchored in geo-coordinates, and records the `generator_version` and definition hash it was authored against. Surviving generator changes is a nice-to-have; the guaranteed options are version pinning and freeze/bake. *Verify: A.*
- **CON-10** The WorldDefinition file format has its own schema version and tested migrations, independent of `generator_version`. *Verify: A* (fixture files from older versions load).

## 3. Numerics and precision

- **CON-11** Authoritative positions are float64 in the planet-fixed frame. Geometry (vertex positions, depth, anything that places a pixel) reaches the GPU only as tile-local or camera-relative float32 values, and no shader computes geometry from values of planet-radius magnitude. Presentation shading (CON-14) may use such values when its declared bound, including temporal stability under camera motion at 1 m altitude, is tested at the worst case. *Verify: A, C* (precision test at 1 m altitude, far from the origin; for presentation shading, its declared-bound test).
- **CON-12** Depth uses reversed-Z with a 32-bit float depth buffer and an infinite far plane. *Verify: C.*
- **CON-13** Precision budget: render jitter < 0.25 px at 1 m camera distance anywhere on the planet; tile-vertex quantization ≤ 1 mm at the finest LOD; geo-anchor round-trip error ≤ 1 mm. *Verify: A, C.*
- **CON-14** Determinism: hashing and noise lattices are integer-based; generator code uses the `libm` crate for transcendental functions; no fast-math; reductions have a fixed order independent of thread count. CPU outputs are bit-exact across runs, thread counts, and Windows and Linux. CPU and GPU outputs agree per field within a mixed bound `|Δ| ≤ max(rel·|v|, abs)` stated in the field's spec (heights: rel 1e-6, abs 1 mm; normals: 0.01°; masks: abs 1e-4). Neighbouring tiles in one cache share one `implementation_id`, so CPU and GPU tiles never meet at a seam. The CPU/GPU bounds apply to fields (CON-07). Presentation data (shading normals and their stored encoding, detail below the finest level, lighting, atmosphere, water surface motion, emission, blending, morphing) has no CPU twin: each such attribute states its class and its own precision bound in its spec, verified by tests and golden images; CON-11 and CON-13 still apply. An attribute that becomes an input to a field or a CPU system needs a CPU reference and an ADR first. *Verify: A, B* (CI compares tile hashes across operating systems).
- **CON-15** Geo conversions go through a `ReferenceSurface` interface (sphere first). Geo-coordinates are the serialization format for authored anchors. *Verify: A.*

## 4. Verification

- **CON-16** Every requirement in `openspec/specs/` has a trace ID and a declared verification tier. Every automated test cites the requirement ID it covers (`// spec: COORD-003`). `cargo xtask spec-lint` enforces both. *Verify: A.*
- **CON-17** Verify at the cheapest tier that can falsify the requirement. Tests are deterministic; a flaky test is fixed or quarantined with a tracking entry, never retried silently. *Verify: A.*
- **CON-18** Every visual feature ships with a debug view and at least one tier-C test. Golden images are approved ("blessed") by the owner, or by an agent under the owner's policy in ADR 0017; everything that policy escalates waits for the owner. *Verify: C, D.*
- **CON-19** Every visual bug gets a repro bundle (camera path, definition hash, generator version, settings, adapter) that `cargo xtask repro` replays. *Verify: A.*
- **CON-20** Count-based budgets (draw calls, triangles, resident tiles, upload bytes, VRAM allocated) are asserted in tests on any adapter. Time-based budgets are measured only on the reference machine, using the performance-harness protocol. *Verify: A, C.*

## 5. Performance gates

- **CON-21** Reference machine: Rasierklinge (RTX 3080 Ti Laptop GPU, 16 GB), charger connected, fixed performance power profile, discrete GPU confirmed by the harness. Targets at 1440p internal resolution: 60 FPS (16.6 ms) median, p99 ≤ 20 ms on the standard flight paths; VRAM within the active cap (8 GB mid-range tier, 14 GB high tier); RAM ≤ 8 GB; GPU uploads ≤ 2 ms per frame. *Verify: C* (performance harness).
- **CON-22** Streaming never blocks a frame: some LOD is always available for every visible node (parent-first requests). *Verify: A* (dive test with fake I/O), *C*.

## 6. Process

- **CON-23** Decisions are recorded as ADRs. An accepted ADR's decision, facts and evidence never change; a new decision is a new ADR. Maintenance that keeps the meaning is allowed and traced by a `Maintained:` line (ADR 0078): status, moved links, spec-owned values, renamed domain terms, errata markers.
- **CON-24** OpenSpec holds the current specs; work happens as OpenSpec changes that are archived when done. Background docs are frozen and non-normative.
- **CON-25** All build, test and tooling commands go through `cargo xtask`; no bash or PowerShell scripts in the repo.
- **CON-26** Development is Windows-native (MSVC toolchain); Linux is covered by CI. WSL is not part of the workflow.
- **CON-27** Licences of dependencies and adopted techniques are recorded (ADRs, `cargo deny`); code under incompatible licences is not adopted.
