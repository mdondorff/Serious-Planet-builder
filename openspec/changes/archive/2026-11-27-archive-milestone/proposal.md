## Why
- The housekeeping skill's archive steps were done by hand at the P3 closure (#1677): 282 log fragments merged into one file, 39 spike rows moved, the header pointers edited.
- Both reviewers and the P3 closure plan (S1, and the precondition of D3) ask for them as a tool (lead, 2026-11-27).

## What changes
- BUILD-072: `cargo xtask archive-milestone <M> [--before YYYY-MM-DD] [--dry-run]`.
  - It merges the fragments of `docs/log.d/` into `docs/archive/<M>/log.md` in `cargo xtask log`'s order, then deletes them.
  - It moves the closed spikes' rows of `docs/spikes.md` to `docs/archive/<M>/spikes.md`.
  - It adds the header pointers when they are missing.
  - It deletes nothing else.
- TEST-159: tier-A tests on a fixture docs tree.
- The README's xtask table and the xtask usage text name the command.

Expected golden changes: none (an xtask command).

## Impact
- `xtask/src/archivems.rs` (new), `xtask/src/main.rs`, `README.md`.
- Specs: BUILD-072 (build-tooling), TEST-159 (testing).
