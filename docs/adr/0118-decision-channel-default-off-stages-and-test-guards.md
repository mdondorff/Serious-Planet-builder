# 0118 Decisions through one file; new generator stages land switched off; test guards and tolerances
Status: accepted (owner, 2026-11-12: "lgtm" to the architect's three workflow changes; point 4 from the owner's challenge of the same morning); point 1 and point 5's bulldozer clauses superseded by ADR 0120 (owner, 2026-11-12)
Date: 2026-11-12
Source: `docs/routing.md`; constitution CON-31; ADR 0055

## Context
- The app pauses a session's messages to other sessions after 10 without an owner message. On the night of 2026-11-11, the architect's answers reached the lanes only through the decisions file for about 8 hours.
- A round-0 breach stage was switched on in the default plan after one measurement, before the gate measure on the production path. It made the defect it targeted worse and needed a second PR to switch it off.
- About a third of the architect's decisions that night were test-form questions: a tolerance to raise, or a guard that a fix had made vacuous.
- The same breach treated a cause the diagnosis had located in an earlier stage, the start surface (owner, 2026-11-12).
- The repository audit's batch C (a housekeeping run) was approved on 2026-11-10 but never queued. It was marked as the architect's, so no lane or queue row carried it (owner, 2026-11-12).

## Decision
1. The architect's decisions for the bulldozer and the lanes are written to the decisions file, `C:/DEV/planner-inbox/architect-decisions.md`. It is outside the repo, and its entry numbers are never cited in the repo.
   - The bulldozer and every lane read it at the start of each turn, and record the entries they act on, by number, in their state or inbox file.
   - The architect sends a message only for an entry that stops or changes running work.
2. A new generator stage, or a rule that changes results, lands switched off in the default plan and the bake profile.
   - A separate PR switches it on once the gate measure that motivated it passes on the production path. That PR cites the measurement.
3. Test forms (CON-31):
   - (a) A tolerance is never raised to make a result fit. A changed tolerance carries its derivation, a formula or a quantum, in the test's comment.
   - (b) A guard that only asserts that a test exercises its case may be restated by the lane when a fix removes what it counted, if every property assertion stays unchanged. The PR body states the restatement and why.
   - (c) A change to a property assertion goes to the architect.
4. When a diagnosis names an earlier pipeline stage as the cause, the fix goes to that stage. A treatment at a later stage states in its proposal why the source cannot be fixed.
5. Every work list that is approved is planned the same day: a row in `docs/status.md`'s queue, with a lane and a target date. Execution follows priority. That covers an audit's batches, a review's conditions and an owner decision with follow-ups.
   - The bulldozer writes the row, also for items whose content is the architect's, and drives it to done.
   - An item that waits on the architect is listed as waiting, not left out.
   - The bulldozer's daily plan check reports every row past its target date, with the reason (owner, 2026-11-12).

## Consequences
- Decisions survive the message cap. Silence from the architect does not stop a lane that reads the file.
- Switching a stage on costs one more PR, and the default plan never carries an unmeasured stage.
- Restating a guard needs no architect turn; a raised tolerance still fails review.
- A later-stage fix needs a written reason before it is ordered.
- An approved list cannot fall between roles: the queue shows every item, its lane and its state.
