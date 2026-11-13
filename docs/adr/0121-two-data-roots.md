# 0121 Two data roots: the repository and SeriousPlanetData
Status: accepted (owner, 2026-11-13)
Date: 2026-11-13
Source: owner, 2026-11-13; amends ADR 0120 point 7 (the path of the order files)

## Context
- Agents stored data in many places: `C:/DEV/planner-inbox`, `C:/tmp`, `C:/temp`, `%TEMP%`, and region bakes in a worktree's `target/stores/`.
- Data in scattered places is hard to find, to clean up and to keep apart from what must last.
- Claude Code itself writes task output and scratchpads to `%TEMP%`; agents do not choose that place.

## Decision
1. Agents store data only in two roots: the repository (the main checkout and its worktrees, including their `target/` folders) and `C:/DEV/SeriousPlanetData`.
2. `target/` holds build and test temp files and spike-like bakes; they are removed with the worktree.
3. `C:/DEV/SeriousPlanetData/worlds/` holds bakes that must last: region and world stores used by more than one task.
4. `C:/DEV/SeriousPlanetData/lanes/` holds the lead's and lanes' coordination files: order files, outboxes, handovers and the digest (formerly `C:/DEV/planner-inbox`).
5. `C:/DEV/SeriousPlanetData/tmp/` holds temporary data shared between worktrees.
6. Agents never choose `C:/tmp`, `C:/temp` or `%TEMP%`; the harness's own use of `%TEMP%` is not covered.
7. Enforcement is the CLAUDE.md line for now; no check or hook.

## Consequences
- `docs/routing.md` and the lead's agent file name `C:/DEV/SeriousPlanetData/lanes/`.
- `cargo xtask lanes` reads the inbox from `PLANET_INBOX_DIR`, else `planner-inbox` beside the main checkout; its default moves to the new folder in a small xtask change.
- The E1 region bake (`r3a-e1`, in crusader's worktree) stays there as a spike-like bake until a lasting store is baked into `worlds/`.
