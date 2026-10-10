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
Option 2. A cache key covers **every input its output depends on**: `hash(read-set hash, generator_version, hash_version, implementation_id, face-mapping id, output parameters, tile_id)`. Output parameters are everything that shapes the result without being a WorldDefinition read (resolution, layer, halo width). Body parameters (ADR 0001) are WorldDefinition nodes and enter through the read set; until `world-def` exists they enter as an explicit body-parameter hash.
- Generators are pure functions and read the WorldDefinition only through a **read-recording context**; the recorded read set is hashed into the key. No globals, no wall-clock time, no unseeded randomness (CON-06).
- The context records **queries and the hash of their results, including empty results** (a spatial query that found nothing still enters the key, so adding an item there invalidates the tile). WorldDefinition nodes are split finely enough that a typical edit stays local; a generator that reads a whole table is invalidated by any edit to it.
- **Every cached artefact type has its own key and version** (height, mesh, later textures and instances). A derived artefact's key includes the key of its source (`mesh_key` contains the height key), and a staged tile's key includes the keys of the earlier-stage tiles it read (a Merkle chain), so an upstream edit invalidates everything downstream.
- **Consumers key derived data by the artefact key, never by `TileId` alone.** This includes GPU residency: a buffer whose key differs from the requested one is replaced, not reused.
- `generator_version` (and each artefact's version) is bumped by hand whenever output changes. Each generator has a **version guard**: a known-answer table of output hashes for a few fixed tiles that also asserts the version it was recorded against, so changing output without a bump fails tier A; `implementation_id` names the implementation (CPU reference, GPU variant, face mapping id). Within one cache each field layer has exactly one implementation (CON-07).
- Generation is staged: global-coarse → regional with halo → local; a stage reads only earlier outputs (CON-08).
- `cargo xtask cache-verify` regenerates a sample from scratch and compares (arrives with the cache in M3).
- Hash function: a specified, versioned, pure-Rust hash over a canonical byte encoding. The SplitMix64 chain (M1) serves ids, lattices and in-memory keys (64 bit). **Persistent and remote keys are at least 128 bits from a collision-resistant hash**: at 10⁹ entries a 64-bit key collides with about 3 % probability, and `mix64` is invertible, so collisions can be constructed. The concrete function is fixed in the M3 cache change and recorded in an ADR before any disk format ships.

## Consequences
- `world-def` exposes hashable nodes with stable canonical encodings; `generators` get a `Generator` trait with a contract test suite every implementation passes.
- A minimal read-recording `GenContext` (seed and body parameters only) is an early M3 task, so generator signatures change once and the purity contract tests start before the full WorldDefinition.
- Easier: incremental invalidation, remote caches. Harder: every generator input must flow through the context; tests must prove it (A).

## Licences of adopted code or techniques
None adopted. If a third-party hash crate is chosen, record its licence here by superseding ADR.
