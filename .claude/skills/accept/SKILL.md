---
name: accept
description: Run milestone acceptance and write a milestone report. Use when all OpenSpec changes for a milestone are archived.
---
1. Run `cargo xtask accept <milestone>`. If the xtask command doesn't exist yet, stop and say so.
2. For each exit criterion of the milestone (from its OpenSpec changes and `docs/background/architecture-report.md` §15), record: pass / fail / needs owner, with evidence (command output, image path, harness JSON).
3. Run the `verifier` agent on the milestone as a whole and include its table.
4. Write `docs/milestones/<milestone>-report.md` with: summary, criteria table, open risks, learnings from this milestone, proposed ADRs awaiting the owner, and the recommendation for the next milestone.
5. If this is a go/no-go gate (M2, M5), end the report with an explicit "Go / No-go / Go with changes" recommendation and stop for the owner.
6. Add one line to `docs/status.md`.

Milestone: $ARGUMENTS
