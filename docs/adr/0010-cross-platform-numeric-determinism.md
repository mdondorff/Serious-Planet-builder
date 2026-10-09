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
- Generator and `core` code that feeds tile data calls `libm::{sin, cos, atan2, ...}`, never `f64::sin` and friends. A lint-style test greps generator crates for the banned method calls.
- Hashing and noise lattices work on integer coordinates derived from f64 positions snapped to the tile grid; the hash is specified, versioned and pure Rust.
- No fast-math, no `mul_add` unless it is explicit and deterministic in software (`libm::fma`), no thread-count-dependent reductions (fixed-order chunking).
- CPU output is bit-exact across runs, thread counts, Windows and Linux. CPU vs GPU float output agrees within 1e-4 relative error (CON-14); GPU tiles are cacheable but not canonical.
- CI compares tile hashes between Windows and Linux (job added with the first real tile in M1).

## Consequences
- `libm` is slower than hardware intrinsics; hot generator loops may need dedicated approximations (which must then be deterministic too).
- Tests: known-answer hashes committed to the repo and checked on both OSes; thread-count invariance; the banned-call scan.

## Licences of adopted code or techniques
`libm` crate: MIT. Adopted at M0 (workspace dependency of `planet-core`).
