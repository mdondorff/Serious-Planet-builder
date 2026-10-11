## Decisions

- **Body 0 uses the world seed itself.** ADR 0001 says a body's seed derives from the world seed and its id; for the single planet the derivation is the identity, so existing content keeps its identity and known answers. Other bodies mix their id in.
- **The derived seed is recorded as a read.** `GenContext::body_seed` records `world.seed` and a derived entry `derived.body.N.seed`, so a body's id is part of every read set. Stored nodes cannot use the `derived.` namespace (a stored `body.N.seed` could otherwise be shadowed by, or collide with, the derived entry).
- **Keys use a 64-bit read-set hash.** Fine for in-memory keys; persistent and remote keys need the 128-bit hash of the cache change (ADR 0006).
- **Radius stays an output parameter** of mesh keys until body parameters enter through the read set (queue item 4, multi-body seams).
- **A zero definition hash is refused** on replay: bundles without one predate the definition and cannot be checked.
- **`HeightGenerator` in front of `HeightField`, not instead of it.** Inner loops need plain values; the context is read once per field. `Bound` is crate-private so the key's inputs always come from the context that produced the values.
- Known trap: a new `HeightField` with inputs beyond the seed must override `inputs_hash` (the default is the seed-only read set); generators must build their field through a context.
