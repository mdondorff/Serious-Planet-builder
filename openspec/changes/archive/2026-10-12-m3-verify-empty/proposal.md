## Why

ADR 0025 follow-ups: (e) a verify run that compared nothing (an empty store, all packs skipped, only entries of another kind) passed with 0 checked; point 4 makes it fail unless `--allow-stale` and at least one entry is stale. (f) The node set id had no requirement and no pinned known answer (ADR 0025 point 3, like the keys in ADR 0023 point 9).

## What Changes

- `VerifyReport::compared_nothing` is now "nothing matched and nothing mismatched"; `passed_allowing_stale` also needs a compared entry or at least one stale entry. The CLI error says which case it is; `--allow-stale` does not help an empty store.
- `World::node_set_id()`; STRM-010 (node set id encoding, pinned known answer `5ac2ac48...`, checked against an independent hand-built derivation); STRM-004 text and scenario; Source lines of STRM-004 and STRM-009 gain ADR 0025.
- The old test asserted an empty store passes; it is replaced by `an_empty_store_compares_nothing_and_fails_even_when_stale_entries_are_allowed` (missing directory, existing empty directory, only another kind) and a CLI test. Nothing weakened.
- Numerics review: the length prefix is checked (`u32::try_from`), the test writes the 56 material bytes out literally, the pinned hex was not cross-checked with a second BLAKE3 implementation (none on the machine). Recorded for later: once other kinds are stored, a `--sample` can pick only other-kind records and fail "compared nothing"; sample height records only before then.
- Not here: STRM-003 "not yet built" and the node set id doc reference, in m3-cache-hardening (#70), to avoid conflicting edits.
