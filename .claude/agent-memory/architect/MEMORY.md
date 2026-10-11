# Architect memory

## State on 2026-10-11 (seeded from the opus-planner session)
- **Milestone:** M2 GO recorded by the owner (#42), "go with changes": LOD-008 first in M3. M3 in progress.
- **Coder:** `sonnet-coder` owns M3 (briefed 2026-10-11; `sonnet-worker` is deprecated). Briefing: accept ADR 0016, land
  the stack #43 -> #46 -> #44 -> #45 (renumber #45's ADRs to 0018/0019), then the "Queued for M3" list up to the M3 report.
  The briefing may have been held for the owner's approval; if nothing landed, check with the owner.
- **ADRs:** 0001-0012, 0015, 0017 accepted; 0016 accepted by the owner 2026-10-11 (status line still to be changed on
  #43); 0013, 0014 proposed (review with the perf numbers).
- **Owner reviews pending:** TEST-014 flight sequence; steep views; official perf rerun on #46 (plan time was 25.75 ms
  at ground-1m on #43, #46 measured about 6.8 ms warm on a dev machine).
- **Watch:** M3 acceptance must show realistic terrain planning at production parameters (cells 32, ratio 240); ADR 0016
  rejects terrain too steep for a crack-free morph interval.
- **Golden policy:** ADR 0017 + `golden-review` skill. `terrain_ground1m_depth` and `terrain_aerial200km_height` are shaded
  (contour lines differ per adapter). Auto-bless only via the tool; escalations go to the owner's `/bless`.

## Lessons from the owner
- He wants one ADR at a time: explain, highlight, recommend, then he decides; small focused PRs per decision.
- He prefers autonomy: decide and proceed where a decision is reversible, ask only for owner-level choices, give a
  recommended option with every question.
- Check reviewers' claims in the source before relaying them; reviewers have been wrong (a mapping-id "bug" that was
  already handled) and right in ways that mattered (stale golden candidates, a missing driver gate).
- Stacked PRs and squash merges conflict: after a squash merge of the base, cherry-pick the stacked commits onto main.
