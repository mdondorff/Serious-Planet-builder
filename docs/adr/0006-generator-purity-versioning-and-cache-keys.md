# 0006 Generator purity, versioning and cache keys
Status: proposed
Date: 2026-10-10
Source: background report §4 (Cache tiers), §5, §11, §17 items 6 and 9; constitution CON-05..08

## Context
Tiles are disposable caches of the WorldDefinition. Seed-only keys cannot detect that a generator or an input changed; a key that misses an input silently serves stale tiles. A later cloud tile service stays cheap only if keys are content addresses.

## Options (with trade-offs)
1. Seeded generation only. Simple; no invalidation story.
2. **Content-addressed DAG keys** with an explicit generator version and implementation id. Precise invalidation (an edit invalidates only tiles that read the changed node); needs a read-recording context.

## Decision
Option 2. A tile key is `hash(read-set hash, generator_version, implementation_id, tile_id)`.
- Generators are pure functions and read the WorldDefinition only through a **read-recording context**; the recorded read set is hashed into the key. No globals, no wall-clock time, no unseeded randomness (CON-06).
- `generator_version` is bumped by hand whenever output changes; `implementation_id` names the implementation (CPU reference, GPU variant, face mapping id). Within one cache each field layer has exactly one implementation (CON-07).
- Generation is staged: global-coarse → regional with halo → local; a stage reads only earlier outputs (CON-08).
- `cargo xtask cache-verify` regenerates a sample from scratch and compares (arrives with the cache in M3).
- Hash function: a specified, versioned, pure-Rust 64/128-bit hash over a canonical byte encoding (chosen in M1 for ids and lattices; the cache key hash is fixed in the M3 cache change and recorded in an ADR before any disk format ships).

## Consequences
- `world-def` exposes hashable nodes with stable canonical encodings; `generators` get a `Generator` trait with a contract test suite every implementation passes.
- Easier: incremental invalidation, remote caches. Harder: every generator input must flow through the context; tests must prove it (A).

## Licences of adopted code or techniques
None adopted. If a third-party hash crate is chosen, record its licence here by superseding ADR.
