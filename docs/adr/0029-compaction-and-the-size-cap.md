# 0029 Compaction and the size cap: write then delete, an injectable eviction rank
Status: accepted (owner's delegation of 2026-10-12: the architect accepts ADRs that reviewers find sound and unambiguous; accepted by opus-planner after reading it against ADR 0019 and the code; amends ADR 0019 "space reclamation" in part under CON-23; drafted by sonnet-explorer)
Date: 2026-10-12
Source: ADR 0019, ADR 0024 points 8 and 11, ADR 0025; STRM-013; constitution CON-05, CON-14, CON-22

## Context
ADR 0019 specifies compaction (rewrite the live entries in Hilbert order, byte-deterministic) and a size cap that evicts the least recently used entries using a last-hit sidecar. Building compaction (STRM-013) showed three places where the code cannot follow the text as written, or should not.

## Decision
1. **Write, then delete, instead of temp file and rename.** The new packs are written under their final names after the highest existing sequence number, each with its index; only then are the packs that were read removed (data file first, ascending). Newest-copy-wins and the torn-tail rules make every intermediate state a valid store (duplicates at worst), so no rename is needed and a crash at any point is repaired by the next run. A leftover `.tidx` without its `.tpack` (a crash between the two removals) is harmless. A removal that fails (on Windows, a reader that has the old pack open) ends the run with an error naming the file after the new packs are complete: the store is then valid with duplicates, and the next compaction removes the old pack.
2. **The eviction rank is an input.** `compact` takes a rank per live record; the highest rank is kept longest, ties go to the later index order, and eviction stops at the first record that does not fit. The last-hit sidecar of ADR 0019 will supply the rank when it is built. Until then the default rank is the sequence number of the pack holding the winning copy (write order). **That is not LRU, and it stops carrying age after a compaction** (the rewritten records share the new packs, so ranks tie and the tie rule evicts by index order). A capped run on an already compacted store therefore evicts by kind, face, level and Hilbert index, not by age. This is accepted for now because a cache that evicts the wrong tile only regenerates it (CON-05); the cap is a safety bound on disk use, not a quality policy.
3. **The cap counts the kept records** (header and payload bytes). Pack headers (24 bytes each) and index files (40 bytes per entry) come on top; ADR 0019's "cache directory" is approximated, not exact.
4. **Compacting twice rewrites the same bytes under new sequence numbers.** Contents and the report's hash are identical; file names differ.

## Consequences
- Compaction is simple to reason about and test (tier A); a crash is never worse than duplicates.
- A real LRU needs the sidecar and a way to carry last-hit times through a rewrite; that is a later change, and this ADR's interface (the rank input) is where it plugs in.
- Records after a damaged record header in a pack are dropped by compaction (they were unreachable already).

## Licences of adopted code or techniques
None.
