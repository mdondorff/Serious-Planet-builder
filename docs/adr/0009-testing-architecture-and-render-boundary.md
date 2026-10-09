# 0009 Testing architecture and render boundary
Status: proposed
Date: 2026-10-10
Source: background report §16, §17; constitution CON-03, CON-16..20

## Context
The renderer is the hardest part to test. If it also made decisions (which nodes, which LOD), most bugs would only show up in pixels.

## Options (with trade-offs)
1. Test mostly through rendered images. Slow, adapter-dependent, hard to localise failures.
2. **Thin executor + GPU-free `FramePlan`**, four verification tiers, debug views, repro bundles.

## Decision
Option 2.
- **Tiers:** A pure (anywhere, seconds), B oracle (GPU code vs CPU twin, software adapter), C pixels (wiring, per-adapter goldens, debug views compared exactly), D human (approved once by the owner, then becomes tier C).
- **Naming convention for the runner** (`cargo xtask test`, nextest filtersets): integration test files `gpu_*.rs` are tier C, `oracle_*.rs` tier B, all other tests tier A.
- **FramePlan:** `frame` turns camera, world state and settings into a serializable `FramePlan` (visible nodes with LOD and morph factors, resident tile handles, instance batches, atmosphere and exposure parameters, debug-view selection). `render` executes it and decides nothing. FramePlans are snapshot-tested and stored in repro bundles. Terrain node selection is a pure CPU function; GPU culling is allowed only for instances, behind a differential test.
- **Goldens:** stored per adapter in `tests/goldens/<adapter>/`; debug views compare exactly; lit views get tolerances only after the owner approves them. A missing golden writes a **candidate** to `target/review/candidates/` and is reported as pending; promotion is owner-only (`/bless`).
- Every requirement cites its test with `// spec: ID`; `cargo xtask spec-lint` enforces it (CON-16).
- Counts (draw calls, triangles, resident tiles, upload bytes, VRAM) are asserted on any adapter; timings only on the reference machine via the performance harness (CON-20, CON-21).

## Consequences
- Easier: failures localise to a tier; most requirements never touch a GPU. Harder: discipline to keep decisions out of `render`.
- Tests: every visual feature ships a debug view and a tier-C test (CON-18).

## Licences of adopted code or techniques
Tooling only; image comparison metrics (FLIP/SSIM) are chosen when the first lit view needs them.
