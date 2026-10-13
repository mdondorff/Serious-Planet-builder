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

## Stop and report to opus-planner (SendMessage) when
- an ADR draft (M4 items 4, 5, 6) is ready for review: keep working on the next item that does not depend on it;
- a perf measurement is needed (spike 2's GPU time, the item 7 perf scenes, the ground-1m A/B);
- spike 3's flight frames are ready for the owner's review;
- an exit criterion of M4 would change, or the constitution would;
- unforeseen trouble your reviewers could not resolve.
Otherwise keep going. When the queue is empty, write a stop report in `docs/status.md` and stop.
