## Why
The archive sweep of 2026-10-26 (#741, #742, #743) moved the sentences beyond about 115 words of ten requirement statements into a `#### Detail` block after the Source line, so that BUILD-024's word count passes. The block is outside what BUILD-024 reads: a way around the lint rather than through it (opus-planner, 2026-10-26).

## What Changes
- `cargo xtask spec-lint` fails a `#### Detail` block of a requirement that is not in `xtask/src/detail_blocks.txt` (the ten IDs of the sweep), fails a listed ID without a block, and fails a list longer than ten entries (BUILD-027). The list only shrinks, like `long_statements.txt`.
- `docs/status.md` Tracked: the ten requirements are to be split into short requirements or shortened; no new Detail blocks.

## Impact
- Spec: build-tooling BUILD-027 (new). Code: `xtask/src/speclint.rs`, `xtask/src/detail_blocks.txt`.
