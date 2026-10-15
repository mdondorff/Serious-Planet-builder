# Briefing for sonnet-explorer: M3 close-out, then M4 (opus-planner, 2026-10-13)

The owner gave M3 "go with conditions" on 2026-10-13. Your work is queued in `docs/status.md`; this file tells you how to run it.

## Read first
1. `docs/constitution.md`, `CLAUDE.md`.
2. `docs/status.md`: "Waiting for the owner" (the M3 go and its conditions), "Queued for M3 close-out" (items 1-4), "Queued for M4 Streaming" (items 0-8 and the owner's decisions a-d at its top).
3. `docs/milestones/M3-report.md`; ADRs 0016, 0020, 0025, 0028, 0030; specs `streaming-cache`, `terrain-lod`, `testing` (only the requirements an item names).

## Order
1. M3 close-out items 1-4, one OpenSpec change and one PR each.
2. M4 items 0-8 in order. Spikes 1-3 produce spike entries (`/spike`), not production code. Items 4-6 are proposed ADRs (`/adr`).

## Workflow rules
- `git fetch` first, then a branch per change from `origin/main` (`change/<id>`); merge `origin/main` into a branch, never rebase a pushed one; no force-push; no `git stash`.
- Failing test first at the cheapest tier; every test cites its requirement ID.
- numerics-reviewer for core, generators and shaders; spec-guardian before archiving; verifier before declaring done. Fix what they find.
- Merge (squash) only when CI is green on Windows and Linux. Add one line at the top of `docs/log.md` per archived change.
- Goldens only through `cargo xtask golden-review` (ADR 0017); anything it escalates goes to the owner's `/bless`.
- `cargo xtask test gpu --real` is yours to run (no confirmation needed). Never run `cargo xtask perf`: ask opus-planner for a perf window.
- Never weaken, skip or re-parameterise a test or raise a budget to get green.
- Spike images worth keeping (contact sheets the spike entry cites): downscaled copies under 1 MB each (e.g. 2048 px JPEG, `magick <in> -resize "2048x2048>" -strip -quality 88 <out>.jpg`) in `docs/spikes/`, linked from the spike entry; full sets stay in `target/review/`, which is not kept.

## Known facts for M4 (opus-planner, 2026-10-13)
- **A cold plan exceeds the frame budget on its own.** Node selection with a cold per-tile cache at ground-1m cost 28.6 ms (2026-10-11) and 25.6 ms (2026-10-12) in perf windows, against 6-7 ms warm and a 20 ms p99 budget. A teleport (ADR 0032 point 2) therefore drops at least one frame before any streaming happens. SPIKE-05 and the teleport scene must report plan time separately from streaming and upload time, so the cause of a missed frame is attributed correctly; whether the teleport frame is exempt or the cold plan must get faster is a decision for ADR 0032's completion (state it there, with the measurement).
- Ground-1m GPU median rose from 3.0 to 4.2 ms after #65 (44-byte vertex, likely); the A/B is in the first M4 perf window (`docs/learnings.md`). SPIKE-06's compact format may change the answer.
- Hand estimates in ADR 0033's context (about 50 KB per tile, 0.4 GB for the ground-1m draw set, 3-6x for the residency set) are not measured: SPIKE-05 replaces them.

## Tool notes (Windows, worktree sessions)
- **One worktree (owner, 2026-10-15):** work only in `.claude/worktrees/sonnet-m4`. Every change, spikes included, is a branch there (`git switch --no-track -c change/<id> origin/main`); do not create further worktrees, for subagents neither. Its single `target/` keeps incremental builds; 15 per-change worktrees had grown to roughly 100 GB of build folders. Spikes need no worktree of their own: their code is feature-gated or an example and lands as a PR like any change.
- `gh` is on the PATH (owner, 2026-10-13): call it as plain `gh`, never by its full path, so permission rules such as `gh pr merge *` match. If a session cannot find it, the session started before the PATH change: report it rather than using the full path.
- Worktree guards may refuse compound or looped git commands: run git commands one at a time.
- Commit messages with several lines: write the message to a file under `$CLAUDE_JOB_DIR/tmp` (or another scratch directory) and `git commit -F <file>`; piping a here-string into `git commit -F -` failed in PowerShell. Same for PR bodies (`gh pr create --body-file`).
- New branch after a squash merge: `git fetch`, then `git switch --no-track -c change/<id> origin/main` (the local branch of a squash-merged PR cannot fast-forward).
- If `gh pr merge` is refused by the permission system or waits for a prompt, do not work around it: report to opus-planner and continue with the next item that does not depend on the merge.

## Stop and report to opus-planner (SendMessage) when
- an ADR draft (M4 items 4, 5, 6) is ready for review: keep working on the next item that does not depend on it;
- a perf measurement is needed (spike 2's GPU time, the item 7 perf scenes, the ground-1m A/B);
- spike 3's flight frames are ready for the owner's review;
- an exit criterion of M4 would change, or the constitution would;
- unforeseen trouble your reviewers could not resolve.
Otherwise keep going. When the queue is empty, write a stop report in `docs/status.md` and stop.
