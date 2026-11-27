## Why
- `cargo xtask next-id` takes the highest requirement number from every file of `openspec/`, the change archive included. An ID that was removed lives only in the archive. Deleting the archive (D3 of the P3 closure plan, the owner's call) would let such an ID be issued again.
- The lead's order (2026-11-27): move the retired IDs into a committed file before D3, which waits for the owner. This PR deletes nothing.

## What changes
- BUILD-073: `openspec/retired-ids.txt` lists the 55 requirement IDs that are in the change archive and in no main spec (generated once on 2026-11-27).
  - `next-id` reads the file and no longer reads `openspec/changes/archive/`.
  - spec-lint checks the file's form, that no requirement of a main spec or added by an open change reuses a retired ID, and that every requirement of the change archive is in a main spec or in the file.
- TEST-160: tier-A tests of the checks and of next-id; a test runs the check on the repository's own file.
- With the archive left out, next-id gives the same number for every requirement prefix as before.

Expected golden changes: none (xtask and a text file).

## Impact
- `xtask/src/retiredids.rs` (new), `xtask/src/nextid.rs`, `xtask/src/speclint.rs`, `xtask/src/main.rs`; `openspec/retired-ids.txt` (new).
- Specs: BUILD-073 (build-tooling), TEST-160 (testing).
