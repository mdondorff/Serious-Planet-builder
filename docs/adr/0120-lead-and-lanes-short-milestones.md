# 0120 A lead and lanes; short milestones with an exit list; a parking list for new ideas
Status: accepted (owner, 2026-11-12: "The plan is accepted", the lean proposal of the same evening)
Date: 2026-11-12
Source: owner, 2026-11-12; supersedes ADR 0118 point 1 and point 5's bulldozer clauses; replaces the architect and bulldozer split of `docs/routing.md` (owner, 2026-11-09)

## Context
- The architect and bulldozer split produced conflicting rules on who sets the order of work (2026-11-12).
- Approved work was lost despite the queue rule: ADR 0108's audit (11-05 to 11-12) and audit batch C (11-10 to 11-12).
- Work went ahead in directions the owner had not seen. Three fixes for the starting surface's basins were made at later stages, then dropped (breach stage, band cap, coast tilt).
- M0 to M7 ran on short exit lists that `cargo xtask accept` checked. The terrain prototype ran on a growing table of gates and items.
- The owner has about one hour a day, spread across the day.

## Decision
1. **Roles.** One lead session (session `opus-architect`, agent `architect`) owns:
   - what, how and when;
   - the milestone exit list, the queue order, IDs, perf windows and the gate ledger;
   - gate verdicts, reviews and the daily digest.

   The bulldozer role is retired.
2. **Lanes merge their own PRs** under `CLAUDE.md`'s "Git" conditions. A PR that departs from an accepted ADR, changes a contract number or claims to close a gate waits for the lead's review.
3. **Milestones.** A milestone has at most five exit items in `docs/milestones/<id>.md`.
   - An exit item may be the owner's look approval, recorded as a sign-off line.
   - Every queue row in `docs/status.md` names the exit item it serves.
4. **The parking list.** `docs/parking.md` holds every approved or proposed item outside the current milestone, one line each. New owner ideas land there by default. Only the owner moves an item into a milestone; when one moves in, another moves out or the date moves.
5. **The check.** `cargo xtask plan` prints the exit items with their state and refuses:
   - a queue row without an exit item;
   - an exit item without a row.

   Until it lands, the lead keeps the list by hand.
6. **Two modes.**
   - Look work (exploration) runs as spike code with picture sheets. It gets no requirement IDs, ADRs or gates until the owner picks a variant.
   - Engine work keeps the full process.
   - No production PR for a look direction the owner has not seen.
7. **Orders.** Lane orders are entries in the lane's file `C:/DEV/planner-inbox/<lane>.md`, written by the lead. A message is sent only to stop or change running work.
8. **The digest.** The lead writes one digest a day for the owner:
   - the exit list with its state;
   - a picture sheet when there is one;
   - at most three decisions, each with a recommendation and the lead's objection;
   - five lines on what changed;
   - what the lead thinks is wrong or overbuilt.
9. **Owner messages** start with "decision:", "question:" or "comment:". A comment is not an order.
10. **Rule budget.** A new process rule is a check (xtask or CI), or it retires an older rule.

## Consequences
- One owner of order and timing. The lead's context carries more; the lanes carry CI and merges.
- Ideas wait on the parking list instead of entering work in flight.
- Review on 2026-11-19, keep or revert, against:
  - the owner's time per day;
  - decisions that waited more than a day;
  - PRs undone after landing;
  - look directions built unseen;
  - dropped items;
  - the lead's context clears.
- Fallback if the lead's context cannot carry the load: a thin executor that runs the lead's queue and owns nothing.
