# 0010 Cross-platform numeric determinism
Status: proposed
Date: 2026-10-10
Source: background report §3 (Determinism policy), §17 item 5; constitution CON-14

## Context
Rust's `f64::sin`, `exp` and friends call the platform maths library, which differs between MSVC (Windows) and glibc (Linux). "Bit-exact across machines" is unreachable without removing that dependency from generator code.

## Options (with trade-offs)
1. Platform libm. Fast, differs per OS.
2. **Pure-Rust `libm` crate** for transcendentals in generator code, integer-based hashing and noise lattices, no fast-math, fixed-order reductions.
3. Fixed-point generators. Exact, but awkward for fields.

## Decision
Option 2. Concretely:
- Generator and `core` code that feeds tile data calls `libm::{sin, cos, atan2, ...}`, never `f64::sin` and friends. The ban is enforced by clippy `disallowed-methods` over the whole workspace (AST based, so `f64::sin` paths, function pointers like `.map(f64::sin)` and `powi` / `mul_add` / `sin_cos` are all caught); the text scan in `crates/core/tests/no_platform_math.rs` is a fallback until that lands.
- Upgrading `libm` is an output change: it needs the bit-level known answers to pass unchanged, or a generator version bump.
- Hashing and noise lattices work on integer coordinates derived from f64 positions snapped to the tile grid; the hash is specified, versioned and pure Rust.
- No fast-math, no `mul_add` unless it is explicit and deterministic in software (`libm::fma`), no thread-count-dependent reductions (fixed-order chunking).
- Generator output is finite (asserted by the tier-A height tests): a NaN's payload bits differ between platforms and would change content hashes.
- CPU output is bit-exact across runs, thread counts, Windows and Linux on x86_64. aarch64 and macOS are expected to match (`libm` only routes correctly rounded `sqrt` / `fma` / `rint` to hardware, chosen by architecture, not OS) but are not tested. GPU tiles are cacheable but not canonical; the CPU vs GPU tolerance is in CON-14 (amended below).
- CI compares outputs between Windows and Linux: each OS writes a determinism manifest (`planet generate --manifest`: height and mesh hashes for tiles on every face, the cube corners and levels 0 to 28, reference-surface round trips, version lines), and the `determinism` job diffs them with `cargo xtask det-diff` (GEN-003). The committed known answers additionally pin the values themselves.

## Amendment to CON-14 (accepted by the owner, 2026-10-10)
CON-14's former "CPU and GPU float outputs agree within 1e-4 relative error" was ill-defined and too loose for terrain: a purely relative bound means nothing near sea level (h ≈ 0), and at 4–8 km of relief it allows 0.4–0.8 m, about 400× CON-13's 1 mm vertex budget. New wording (constitution v1.1):

> CPU and GPU outputs agree per field within a mixed bound `|Δ| ≤ max(rel·|v|, abs)` stated in the field's spec (heights: rel 1e-6, abs 1 mm; normals: 0.01°; masks: abs 1e-4). Neighbouring tiles in one cache share one `implementation_id`, so CPU and GPU tiles never meet at a seam (CON-07); a cache test enforces this once the cache exists.

GEN-008 carries the per-field bounds (change con14-tolerance). No GPU generator exists yet, so no code changes.

## Consequences
- `libm` is slower than hardware intrinsics; hot generator loops may need dedicated approximations (which must then be deterministic too).
- Tests: known-answer hashes committed to the repo and checked on both OSes; the cross-OS manifest diff (GEN-003); thread-count invariance; the banned-call check (clippy, text scan as fallback).
- Follow-ups (queued in docs/status.md): clippy `disallowed-methods` replacing the text scan; `app` covered by the check.

## Licences of adopted code or techniques
`libm` crate: MIT. Adopted at M0 (workspace dependency of `planet-core`).
