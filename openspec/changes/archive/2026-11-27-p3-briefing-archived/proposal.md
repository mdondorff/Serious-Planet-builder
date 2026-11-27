## Why
- The owner decided D10 (2026-11-27): the terrain prototype briefing (`docs/briefings/terrain-prototype.md`, the P1 to P3 work block) is archived to `docs/archive/P3/briefings/`.
- BUILD-040 and BUILD-042 read it. BUILD-042 (an item that becomes done moves the briefing's re-estimate) is retired with it. BUILD-040 is decoupled from it.

## What changes
- BUILD-040: the gate table of `docs/design/terrain-target.md` section 12.1 still needs a contract-change line. The line now goes into that document's own `## 14. Contract changes` list, which takes the briefing's 13 lines. The briefing's owner-decisions table is no longer checked.
- BUILD-042: removed; its check and tests go with it, and its ID goes into `openspec/retired-ids.txt` (BUILD-073).

Expected golden changes: none (an xtask check and docs).

## Impact
- `xtask/src/process.rs`, `xtask/src/speclint.rs`; `docs/design/terrain-target.md`; the briefing moved; `docs/parking.md` and `docs/README.md` pointers.
