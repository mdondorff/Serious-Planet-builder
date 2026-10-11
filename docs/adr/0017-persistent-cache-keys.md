# 0017 Persistent cache keys: 128 bits from BLAKE3 over a canonical encoding
Status: proposed
Date: 2026-10-11
Source: ADR 0006 ("persistent and remote keys are at least 128 bits from a collision-resistant hash; the concrete function is fixed in the M3 cache change and recorded in an ADR before any disk format ships"), ADR 0010, M3 queue item 5; constitution CON-05, CON-06, CON-14, CON-27

## Context
Today every key is a 64-bit SplitMix chain (`hash_u64s`): fine for in-memory maps (the read-set hash of ADR 0006 and `GenContext` included), not for anything stored or shared. At 10^9 entries a 64-bit key collides with about 3 % probability, and `mix64` is invertible, so collisions can be constructed deliberately (a hostile or merely unlucky remote tile server would serve the wrong tile silently). A disk or remote cache needs keys that are wide, collision resistant, specified byte for byte, and identical on every platform (CON-14).

## Options (with trade-offs)
1. **BLAKE3, truncated to 128 bits**, over a canonical byte encoding. Cryptographic collision resistance; very fast (GB/s, SIMD, but the output is defined independent of the instruction set); one small pure-Rust crate (`blake3`, CC0-1.0 or Apache-2.0, with C/asm SIMD paths selectable at build time; the portable path is pure Rust). 128 bits of a 256-bit output keep the full-output security argument for collisions at 2^64 work, far beyond any plausible cache.
2. SHA-256 (RustCrypto `sha2`, MIT or Apache-2.0), truncated. Equally safe, three to five times slower, no advantage here.
3. A non-cryptographic 128-bit hash (xxh3-128, SipHash-2-4-128). Fast, but collisions can be constructed (xxh3) or the key is secret-dependent (SipHash); fine for accidental collisions, wrong for a shared remote cache.
4. Keep 64-bit keys and add a verify-on-read content check. Detects a collision after the fact, cannot prevent a wrong hit from being served first; two lookups per read.

## Decision
Option 1.
- **Key = first 16 bytes of BLAKE3 over a canonical encoding**, written as 32 lowercase hex characters in file names and logs. The encoding is the ordered list `format tag (u8) | kind (u8: height, mesh, ...) | hash version (u32 LE) | generator version (u32 LE) | implementation id (length-prefixed UTF-8) | face-mapping id (length-prefixed) | tile id (u64 LE) | output parameters (each tagged and length-prefixed) | read-set entries`, with all integers little endian, `f64` as its bit pattern, and every variable-length part length-prefixed so no two different inputs share an encoding.
- **The read set enters as entries, not as the 64-bit hash**: for each recorded read, `path (length-prefixed) | node content hash (16 bytes, BLAKE3-128 of the node's canonical encoding)`, sorted by path. `world-def` node hashes therefore get a 128-bit form beside the 64-bit one used by in-memory keys; the 64-bit chain stays for ids, lattices and in-memory maps (hot paths, never persisted).
- **Chained keys** (ADR 0006): a derived artefact's key includes the 16-byte key of its source, so a mesh key covers its height key bytes exactly.
- **Domain separation** through the kind byte and a fixed context string passed to BLAKE3's `derive_key` mode (`"planet-builder cache key v1"`), so a key of one kind can never equal a content hash of another use.
- **Versioning**: the format tag and hash version are in the encoding; changing the encoding bumps the tag and invalidates every stored entry, never aliases one. The 64-bit in-memory keys are untouched by this ADR.
- **Known answers**: a committed table of keys for a few fixed inputs (all kinds), asserted next to the tag and `HASH_VERSION`, identical on Windows and Linux (the determinism manifest of GEN-003 includes them).
- **Dependency**: `blake3` with default features off except `std`, portable implementation only (`cargo deny` records its licence; no build script needing a C toolchain). SIMD paths can be enabled later without changing any output.

## Consequences
- Easier: a remote cache and a shared team cache become safe; keys stay content addresses; `cache-verify` can compare a regenerated tile to a stored one by key alone.
- Harder: every key input must have a canonical byte form (the 64-bit chain accepted any `u64` list); `GenContext` read sets carry entries with 128-bit node hashes; known answers and the manifest change once.
- Cost: BLAKE3 over a few hundred bytes per key is well under a microsecond; keys are computed per tile at plan time, so the 5,000-node plan adds on the order of a millisecond only if persistent keys are computed for every node (they are needed only when a tile is looked up on disk).
- Tests (A): known-answer keys; every input changes the key (property test over version, implementation id, mapping, tile, each output parameter, each read entry); encoding is injective on a generated sample; keys equal on Windows and Linux.

## Follow-ups
- The storage ADR (0018) fixes how keys index the pack.
- If BLAKE3 turns out to be unwelcome (licence review, supply chain), SHA-256 is a drop-in at the same width; the format tag changes.

## Licences of adopted code or techniques
`blake3` (O'Connor, Aumasson, Neves, Wilcox-O'Hearn): CC0-1.0 or Apache-2.0, compatible with the project's licence; to be recorded in `deny.toml` when the crate is added.
