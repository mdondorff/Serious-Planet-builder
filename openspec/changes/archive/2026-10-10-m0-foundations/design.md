## Context

Greenfield repository seeded with the constitution, agents, skills and the frozen background documents. M0's job is to make later work verifiable (background report §15, M0; starter §8). Tooling decisions are in ADRs 0004, 0005, 0008, 0009 and 0012.

## Goals / Non-Goals

**Goals:** a workspace the layering rules can be machine-checked against; one command surface; a golden harness whose failures are actionable; a performance-harness protocol that cannot be run by accident; CI on both operating systems from the first push.

**Non-Goals:** any coordinate maths (M1), any renderer beyond the hello triangle, a window for the editor mode (M2), a real performance measurement (needs the owner), blessed goldens (owner only).

## Decisions

- **xtask is std-only.** It must build in about a second and can never pull a graphics crate into the tree. Subcommands shell out to `cargo` and `cargo nextest`.
- **Tier selection by test-binary name.** `gpu_*` = tier C, `oracle_*` = tier B, everything else tier A, expressed as nextest filtersets. Keeps tiers visible in `ls` and needs no custom attributes. Both prefixes must have at least one binary (nextest rejects a filter that matches nothing); `oracle_shader_validation` is the first tier-B test.
- **`test fast`** maps changed paths (working tree plus `main...HEAD`) to crates and runs those and everything above them in the chain; shared crates (`testkit`, `perf`, root manifests) or no change at all run everything. A run over 10 s prints a warning and is a failed step of `accept`.
- **Goldens:** `tests/goldens/<adapter-key>/<name>.png`, adapter key = `backend-name` slug. A missing golden writes `target/review/candidates/...` and reports `PENDING BLESS`; it fails only with `PLANET_GOLDEN_STRICT=1`, which CI flips once the owner has blessed. Metrics that do not need a golden (corner colour, filled area, no stray colours) always run, so the test still falsifies something before blessing.
- **Spec trace format:** requirement headings `### Requirement: ID Title`, body lines `Verify:`, `Status:`, `Source:`, `Review:`. `Status: planned` exempts a requirement from the coverage rule until the change that implements it flips it to `active`.
- **Performance harness:** `planet-perf` holds pure logic (adapter refusal, statistics, nvidia-smi parsing, JSON); the frame loop is a stand-in (hello-triangle round trip) until M2. `cargo xtask perf` refuses to run without `--machine-ready`, which only the owner's confirmation justifies.
- **Software adapter:** `force_fallback_adapter` selects WARP on Windows and lavapipe/llvmpipe on Linux. Verified locally on WARP; Linux is verified by CI (not yet run: no pushed branch, see `docs/status.md`).

## Risks / Trade-offs

- wgpu 30 API differs from older documentation; the code was written against the compiler. Mitigation: wgpu is pinned and upgraded deliberately (ADR 0004).
- Per-adapter goldens multiply what the owner must bless (WARP, lavapipe, real GPU). Mitigation: few, small (256 px) images; debug views only.
- `test fast` for a fresh checkout includes compile time; the 10 s budget holds only with a warm build cache.

## Open Questions

None blocking. The owner must accept or change the twelve proposed ADRs, install `gh`, and push a branch so CI can run (listed in `docs/status.md`).
