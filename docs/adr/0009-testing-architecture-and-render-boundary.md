# 0009 Testing architecture and render boundary
Status: accepted
Date: 2026-10-10
Source: background report §16, §17; constitution CON-03, CON-16..20

## Context
The renderer is the hardest part to test. If it also made decisions (which nodes, which LOD), most bugs would only show up in pixels.

## Options (with trade-offs)
1. Test mostly through rendered images. Slow, adapter-dependent, hard to localise failures.
2. **Thin executor + GPU-free `FramePlan`**, four verification tiers, debug views, repro bundles.

## Decision
Option 2.
- **Tiers:** A pure (anywhere, seconds), B oracle (GPU code vs a CPU twin, or backend validation of shaders; software adapter by default, and the real adapter under `cargo xtask test gpu --real`), C pixels (wiring, per-adapter goldens, debug views compared exactly), D human (approved once by the owner, then becomes tier C).
- **Naming convention for the runner** (`cargo xtask test`, nextest filtersets): integration test files `gpu_*.rs` are tier C, `oracle_*.rs` tier B, all other tests tier A.
- **FramePlan:** `frame` turns camera, world state and settings into a serializable `FramePlan` (visible nodes with LOD and morph intervals, resident tile handles and mesh keys, instance batches, atmosphere and exposure parameters, debug-view selection). `render` executes it and decides nothing. Terrain node selection is a pure CPU function; GPU culling is allowed only for instances, behind a differential test.
- **Every scene's plan** (tile scene, terrain, later ones) has a full text form with floats as bit patterns, a committed snapshot test over all fields, and a repro bundle with replay (CON-19). A summary hash is not a substitute.
- **Render boundary:** `render` may **reject** an inconsistent plan with an actionable error (missing resource, stale mesh key, bad resolution); it never **substitutes** (dummy resource, parent tile, fallback LOD, skipped draw). Fallbacks are `frame`'s job. Per-vertex or per-pixel functions of plan data (e.g. CDLOD morph between the plan's interval, ADR 0014) are allowed, and each has a tier-B oracle against a CPU twin with a tight bound.
- **Tier B:** every GPU compute path (generators, instance culling) ships its CPU differential in the same change, judged against the per-field bounds of CON-14.
- **Goldens:** stored per **adapter family** in `tests/goldens/<adapter>/`, with the key free of driver and compiler versions (e.g. `vulkan-llvmpipe`, not `vulkan-llvmpipe-llvm-20-1-2-256-bits`); the full driver string is recorded next to the goldens and printed on every mismatch, so a driver change is reported as such. CI fails when an adapter it runs on has no golden directory (silent PENDING is only for names listed in `pending.txt`), and CI runner images are pinned so drivers change only on purpose. A missing golden writes a **candidate** to `target/review/candidates/`; promotion is owner-only (`/bless`).
- **Golden classes**, recorded per golden: *debug* views compare exactly on every adapter; *shaded* views compare exactly per adapter (cross-adapter differences such as 1 LSB are expected and get their own goldens); *lit* views get tolerances only after the owner approves them.
- Every requirement cites its test with `// spec: ID` on a `#[test]` function; `cargo xtask spec-lint` enforces it and checks that each active requirement is cited from at least one test at its declared tier (`gpu_*` C, `oracle_*` B, other A) (CON-16).
- Counts (draw calls, triangles, resident tiles, upload bytes, VRAM) are derived from the plan so tier A can assert budgets on any adapter; tier C checks that `render` matches them. Timings only on the reference machine via the performance harness (CON-20, CON-21).

## Consequences
- Easier: failures localise to a tier; most requirements never touch a GPU. Harder: discipline to keep decisions out of `render`.
- Tests: every visual feature ships a debug view and a tier-C test (CON-18).
- Review 2026-10-10 found gaps against this decision; follow-ups are queued in docs/status.md (golden key and CI, terrain plan text form and repro bundle, spec-lint tiers, morph oracle and dead field, the dummy-texture substitution in `exec.rs`, counts from the plan). The ADR is accepted once they land without surprises.

## Licences of adopted code or techniques
Tooling only; image comparison metrics (FLIP/SSIM) are chosen when the first lit view needs them.
