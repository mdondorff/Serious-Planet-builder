## Why

Two follow-ups from the ADR 0023/0024 reviews (status.md, M3 item 5 (c) and (d)): a long-lived store that meets a payload with a bad CRC returns `Corrupt` on every lookup instead of regenerating once (ADR 0019 validation, ADR 0024 point 7), and a pack cut short while a reader scans it fails `TileStore::open` although ADR 0024 point 4 makes that a torn tail.

## What Changes

- `TileStore::get`: an entry whose record fails its checks (`Corrupt`) is reported once; the next lookup is a plain miss. It stays a miss even if an older pack holds an intact copy (ADR 0024 point 8: a corrupt newest copy is a miss). The files are not touched; `keys_for`, `all_entries` and `sample_records` still list the entry, so a verifier sees the damage.
- `scan` (and the tail check): a file that ends before the length taken from its metadata is a torn tail at that position, in the header read, the zero-fill check and the payload CRC stream. Other read errors stay errors. A writer never grows a file when it truncates to that position.
- Doc and spec references to "proposed ADR 0022/0023/0024" now name the accepted ADRs; STRM-003's "cache-verify, not yet built" is corrected (wording only).
- Modified: STRM-008 (text, one scenario changed, one added). Tests: two in `cache/tests/pack.rs` and two scan unit tests in `pack.rs`.
- No design.md: the change is two local behaviours of one module, fully described above.
- Not here: the node set id reference moves to ADR 0025 when that ADR is merged. The store-level open of a file cut mid-open is not reproducible deterministically; the scan and tail check are tested directly.
