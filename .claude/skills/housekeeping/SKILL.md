---
name: housekeeping
description: Clean the working documents (status, log, learnings, spikes, ADR statuses, briefings, specs) at a milestone boundary so they show the current state and keep history findable. Use after a milestone's acceptance and before the next milestone starts, or when the owner asks for a housekeeping run.
---
Housekeeping is split (owner, 2026-10-16): the **architect** curates, deciding what is open, what is history and what an old decision means. The **coder** then does the mechanical pass from the architect's list. Prefer fixing the cause of dust over cleaning it (see "Root causes" below). If a step finds dust that a rule in this skill should have prevented, fix the rule as well.

## When
- At every milestone acceptance, after `cargo xtask accept <M>` and the report, before the next milestone's queue starts (the `accept` skill calls this skill).
- On the owner's request.

## Architect: curation (one docs PR)
1. **`docs/status.md` is forward-only.** It keeps only:
   - open items under "Waiting for the owner";
   - the current milestone's queue (open items only);
   - the current stop report, if it is unanswered;
   - tracked items and quarantines.

   Move everything else:
   - finished queues go into `docs/milestones/<M>-report.md` (an appendix "Queue as executed") if the report lacks them, otherwise they are deleted, because the log has one line per change;
   - answered stop reports, handovers and decided owner items are deleted (their outcome is in the log);
   - the "Recent" section is deleted if it duplicates the log.

   Target: under 10 KB.
2. **Archives per milestone.** Move the finished milestone's lines of `docs/log.md` and its fragments of `docs/log.d/` (delete the files; merge them with `cargo xtask log`) to `docs/archive/<M>/log.md`, and its closed spikes from `docs/spikes.md` to `docs/archive/<M>/spikes.md`. Leave a one-line pointer per archive in the current file's header.
3. **Learnings by scope.** For each entry of `docs/learnings.md`, decide:
   - **project** learning (still true and useful beyond the milestone): it stays;
   - **milestone** learning (useful as history): it moves to `docs/archive/<M>/learnings.md`;
   - **obsolete** (its condition is solved, it no longer affects any decision, and the history adds little): it is deleted, with one line in the archive file ("removed: <date> <title>: <why>").

   An entry disproved by a later one is moved or deleted, never left beside it without a mark.
4. **ADR status index.** Check every ADR's status line against later ADRs. When a later ADR decided an open point, edit only the earlier ADR's status line to say so (allowed for accepted ADRs). Then refresh the table in `docs/adr/README.md`: number, title, status, open points (with the milestone or trigger that decides them).
5. **Briefings.** Replace or delete `docs/briefings/*` for the finished milestone; the next milestone's briefing is written when its queue starts.
6. **Owner items.** List the dormant ones (an open question with no near trigger) with their trigger in the status file; do not ask the owner about them now.
7. **Documentation model (ADR 0055).** Check that every current number lives in a spec, not only in an ADR amendment chain; that no new readings ADR appeared; and that archived changes hold only `proposal.md` and spec deltas. Fix the cause in the skill or CLAUDE.md if any of these drifted.
8. **Agent memory.** Update `.claude/agent-memory/architect/MEMORY.md` with the milestone's decisions and delete what the repo now records.
9. **Process review** (owner, 2026-10-18). One table in `docs/milestones/<M>-report.md` (section "Process review"), each measure against the previous milestone, from recorded data only (no estimates without a mark):

   | Measure | Source |
   |---|---|
   | edit-to-first-test time, `test fast`, `test`, `test gpu` wall time | xtask timings, `target/test-logs/` |
   | CI median per job, flaky reruns, quarantines | GitHub runs (`gh run list`) |
   | PR lead time; time waiting on CI and on the owner | PR timestamps |
   | perf windows run, and how many gave no verdict | log, perf reports |
   | rework: reverted or redone changes, corrected figures | log, learnings |
   | coordination stalls: idle sessions, unwatched CI, lost messages | log, status, owner nudges |

   Then three to five improvements ranked by gain against cost, each with an owner session. Easy-to-reverse ones go into the next milestone's queue; a trade-off goes to the owner. A process cost that a budget check could have caught gets a budget (below).
   **Budgets** (BUILD-014): `cargo xtask test fast` prints `BUDGET EXCEEDED: edit-to-first-test ...` over 30 s of build plus tests, and `cargo xtask ci-stats` flags a CI job median over 16 min. The tools only report: any such line, seen in a run or in this review, becomes a Tracked item in `docs/status.md` (the figure, the date, the cause if known). Run `cargo xtask ci-stats` for the CI row of the table above.
10. Then write the coder's list (below) as a queue item.

## Coder: mechanical pass (one change, from the architect's list)
- Stale references in specs and code comments: "not yet built", "proposed ADR NNNN" for accepted ADRs, superseded requirement IDs, dead `file:line` references. Each fix is a text change; never change a requirement's meaning (that is a spec delta).
- Archive moves the architect named, as file moves with history (`git mv` where a whole file moves).
- Check that `openspec/changes/` holds no finished change that was not archived.
- Run `cargo xtask spec-lint` and `cargo xtask test fast`.

## Root causes (prevent the dust)
- **Status grows** because finished items stay: the coder deletes a queue item's text when its change is archived (the log line is the record), and the architect deletes answered stop reports when answering them. The header of `docs/status.md` states this rule.
- **The log conflicted on every PR** because every PR prepended to one file. Fixed (BUILD-010): one log fragment per change (`docs/log.d/<date>-<id>.md`), read with `cargo xtask log`; spec-lint rejects a malformed one. A fragment is never edited after merge; a conflict in `docs/log.md` means someone wrote there, so keep both sides and move the new line to a fragment.
- **Learnings rot** because entries have no scope or end: the `learning` skill asks for `Scope:` (project or milestone) and `Obsolete when:`.
- **ADR statuses drift** because a later ADR decides an earlier one's open point: the `adr` skill updates the earlier status line and the README table in the same PR.
- **Spikes collide** with the main coder: the `spike` skill's parallel-session practice.
- **Process costs grow unnoticed** between reviews: each has a budget that its tool checks and reports when exceeded (`test fast` 20 s; edit-to-first-test and CI time, BUILD spec). An exceeded budget becomes a Tracked item in `docs/status.md`, not a remark.

Run notes: append one line per run to `docs/archive/README.md`: date, milestone, what moved, what was deleted, and any rule fixed.

Arguments: $ARGUMENTS
