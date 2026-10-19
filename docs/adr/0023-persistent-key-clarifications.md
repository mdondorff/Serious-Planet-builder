# 0023 Clarifications to the persistent key encoding (ADR 0018)
Status: accepted (owner's delegation of 2026-10-12: the architect accepts ADRs that reviewers find sound and unambiguous; accepted by opus-planner after the spec-guardian and numerics reviews and their text fixes; points 2 and 3 amended by ADR 0066, 2026-10-19)
Date: 2026-10-11
Source: spec-guardian and numerics reviews of m3-cache-keys (2026-10-11); ADR 0018 (accepted, never edited: CON-23), ADR 0006, ADR 0021; constitution CON-05, CON-14

## Context
ADR 0018 is accepted and fixes the persistent key as the first 16 bytes of BLAKE3 over a canonical encoding. Implementing it (STRM-003, `planet_cache::key`) turned up places where the ADR text is imprecise or the implementation had to choose. Most points pin down what a second implementation (a remote cache server, a verifier) must do to produce the same bytes. Some go beyond ADR 0018's text; under CON-23 this ADR **amends ADR 0018 in part** where listed under "Amends ADR 0018" below, and ADR 0018 is otherwise unchanged. Nothing is stored under these keys yet, so every point can still change.

## Clarifications (what the code does; the owner may amend)
1. **Empty query result.** The result hash of a query that found nothing is `derive_key("planet-builder query result v1", [])[..16]` (the domain-separated hash of the empty input), not the plain `blake3("")`. This follows ADR 0018's own domain-separation bullet. Pinned as a known answer.
2. **Query result material.** Items sorted by the whole tuple `(id, content hash, direction bits)` (ids are unique within a body, so this is id order), each as `id u64 LE | content hash u64 LE | direction bits 3 x u64 LE` (48 bytes, fixed width, so no count is needed). The ADR names ids and content hashes; the direction bits are included because moving an item inside its cell changes the output. Ids and content hashes are 64-bit until ADR 0021 point 4 widens them: **the 64-bit item hashes make this part of a key only 64-bit collision resistant (and `mix64` is invertible), which falls short of ADR 0006's "at least 128 bits from a collision-resistant hash". Gate: no item file format and no remote cache ship before item ids and content hashes are 128-bit (ADR 0021 point 4).**
3. **Query entry parameters.** Kind 1 (cell): `body u32 LE | cell raw id u64 LE`. Kind 2 (whole body, a tile over the cell limit, ADR 0021 point 5): `body u32 LE`. There is no layer yet (ADR 0021 point 3); adding one bumps the format tag.
4. **Body parameter hash.** `body id u32 LE | seed u64 LE | radius f64 bits u64 LE | surface id (u32 length prefix + UTF-8)`, under the context `planet-builder body hash v1`. The body id is inside it as well as a named field of the key.
5. **Node hash.** `path (u32 length prefix + UTF-8) | type byte (1 integer, 2 float) | value bits u64 LE`, under `planet-builder node hash v1`; a derived value (a body's derived seed) is hashed as a node under its `derived.` path.
6. **Source key presence.** A key with no source and a key whose source is all zero bytes must differ, so the source is written as a presence byte (0 or 1) followed by 16 bytes. This is format tag 2; tag 1 (no presence byte) was never stored.
7. **Which slot holds which version.** `generator version` is the version of the field (height) generator for both a height key and a mesh key; the mesh version is output parameter tag 3, the grid size tag 1 (u32 LE) and the skirt fraction tag 2 (its f64 bits as u64 LE); tag 3 is u32 LE. **Tag 1 (resolution) counts grid cells (intervals) per tile edge, for every kind**: a height tile of `cells` intervals has `cells + 1` samples per edge and is keyed by `cells`, so a mesh's source key equals the key of the height tile it is built from. Future artefact kinds follow the same rule: the generator version slot carries the generator's version, the artefact's own construction version is an output parameter.
8. **Radius.** A key takes the body's radius as an argument because a field does not know it; it must be the finite positive radius of the definition (anything else is a panic, not a key). Reading it through the context (so it enters the read set) is the cleaner design; it needs `Bound` to expose it and is left to the staged-pipeline change.
8a. **Floats and strings.** An `f64` is hashed as its exact bit pattern with no canonicalisation, so `0.0` and `-0.0` give different keys (a miss, never an alias; unlike the halo in ADR 0022, which is canonicalised before it is hashed). NaN cannot reach a key today (the definition loader refuses non-finite numbers and the radius is checked); a derived value that could be NaN must be refused or canonicalised first, because NaN bits differ between platforms. Strings are hashed as their UTF-8 bytes as given, with no Unicode normalisation.
9. **Determinism manifest.** ADR 0018 says the GEN-003 manifest includes the known answers. `generators` cannot depend on `cache`, so the known answers are asserted in `cache`'s tests, which run on Windows and Linux in CI with the same literals; moving them into the manifest means writing it from `app`.
10. **Not yet in a key:** the margin or halo of a regional output as an output parameter (ADR 0021 point 2, ADR 0022), the layer, and the persistent keys of staged outputs (the stage model has 64-bit keys). Their output-parameter tags and entry kinds are reserved when they arrive.

## Amends ADR 0018 (in part, CON-23)
- Point 1 resolves a contradiction inside ADR 0018 (the empty result as "BLAKE3-128 of the empty string" against the rule that query result hashes use `derive_key` with their own context) in favour of domain separation.
- Point 2 adds the direction bits to the query result material ("ids and content hashes" in ADR 0018).
- Points 3 and 4 add the body id to the query parameters and to the body parameter hash.
- Point 6 replaces "16 zero bytes for none" with a presence byte followed by 16 bytes (format tag 2).
- Point 9 asserts the known answers in `cache`'s tests on both CI operating systems instead of the GEN-003 manifest, which CON-02 forces (`generators` cannot depend on `cache`).

## Decision
Accepted as the reading of ADR 0018 and as the partial amendment listed above. A different reading of any point later bumps the format tag and re-records the known answers (nothing is stored under these keys yet). Code follow-up: the resolution unit of point 7 (`height_key` documents `res` as samples per edge while `mesh_key` passes cells).

## Consequences
- A second implementation can reproduce the bytes from this ADR and the known answers.
- Points 2 and 8 are the ones with a cost: 64-bit item hashes cap a key's collision resistance, and the radius is a caller argument.

## Licences of adopted code or techniques
None.
