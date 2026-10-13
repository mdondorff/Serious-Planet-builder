# Architect memory

Refreshed 2026-10-13 at the M3 gate (session `opus-plannner`, worktree `opus-planner`). Confirm everything here against
`main` and `docs/status.md` before acting on it.

## Sessions
- **sonnet-explorer:** the one long-lived worker. Next: M3 close-out items 1-4, then M4 items 0-8 (status.md queues).
  Briefing: `docs/briefings/sonnet-explorer-m4.md` (on PR #100's branch until merged); the owner pastes the pointer.
- **sonnet-coder:** reset; its worktree was being removed by the owner. **sonnet-worker:** deprecated.
- This session: architect, own worktree `.claude/worktrees/opus-planner`. Owner renamed it `opus-plannner` (typo,
  2026-10-13): check `ListAgents` for the exact name coders must address.

## State of main (2026-10-13)
- **M3: GO WITH CONDITIONS (owner, 2026-10-13).** Conditions: (1) TEST-014 flight review done by the owner; (2) before
  the first regional field is stored: per-tile node set id, ADR 0021 pt 7 contract test, tile-level GEN-017 test;
  (3) ground-1m GPU A/B (3.0 -> 4.2 ms) in the first M4 perf window (mine); (4) gen-014-active; (5) det-manifest-fields.
- ADRs 0001-0020, 0022-0025, 0027-0030 accepted; 0021 and 0026 in part (0026 pt 5 decided by 0027).
- Perf: M3 gate window 2026-10-13 valid, no regression; 16384 bake 370.3 s.
- All ten specs still carry OpenSpec's "TBD" Purpose placeholder: queued (close-out item 4).

## Owner decisions 2026-10-13 (M4 shape)
- Worlds name their height field; eroded (GEN-017) default; `cpu-terrain-v1` selectable (M4 item 0).
- **Terrain gets an explicit VRAM share** (start: 2 GB of 8 GB mid tier, 3 GB of 14 GB high tier; final in M4 budget
  ADR) so future content (ground cover, objects, a game) has room. He thinks beyond renderer/editor: keep headroom.
- Residency is position-based: rotation/mouse look never triggers fetching; view direction only orders requests;
  teleport must be handled (no velocity prediction possible).
- Finest geometry level may be lifted (today level 26 = 15 cm tile, ~5 mm vertices), detail below via shaders (SPIKE-07).
- Today's mesh vertex is 44 B (11 f32); compact format (shared grid + height textures) is SPIKE-06.
- Cross-OS determinism: he asked its cost; I said keep it (cheap marginal cost over run/thread determinism).
- He is short on weekly quota: prefers M4 fully prepared so the worker can run for days without him.

## Owner decisions 2026-10-12
- Workflow: one coordinator (this session) + one long-lived worker the owner talks to directly, separate worktrees.
- Reviewer models: spec-guardian and verifier on Sonnet (trial); numerics-reviewer + architect on Opus. Token economy:
  review proportionate to risk; review small ADRs myself against the code.
- status.md = forward state + last 10 lines; full log in docs/log.md (newest first).
- `cargo xtask test gpu --real` needs no owner confirmation (never during a perf window).

## Open threads
1. Normals are not geomorphed (only if it shows in a view).
2. CON-08 reading (owner): only if a regional chain needs same-stage reads (ADR 0022 option 2).
3. Process proposal (log lines in archived changes) is superseded by the status/log split.
4. Owner items: TEST-008 split decision; editor window and egui per ADR 0015.
5. Housekeeping for the owner: 11.7 GB bake in `.claude/worktrees/sonnet-explorer/target/accept`; `.git/config.lock`
   from a hung `git branch -d` (use `--no-track` while it exists); old worktrees (smb-adr-review, scratchpad base,
   allow-gpu-real) removable.

## Lessons from the owner
- Coders edit proposed ADRs in their own PRs while you review them: merge main and combine texts before accepting.
  Before accepting, check open coder branches that touch the ADR (#68 edited accepted 0024; fixed via 0025, CON-23).
- sonnet-explorer's `gh pr merge` needs the owner's go-ahead in its session; a stall at merge means a prompt waits there.
- **ADR delegation (owner, 2026-10-12):** accept ADRs yourself when reviewers' recommendations are strong or the ADR is
  sound and unambiguous; bring the owner only constitution readings/amendments, open trade-offs, exit-criteria changes.
  Status line: "owner's delegation of 2026-10-12". Clarifications beyond an accepted ADR say "amends in part".
- Explain plainly, most important first with cost to reverse, recommend, then he decides. Ask only questions that
  change the work. He prefers autonomy on reversible decisions.
- **You execute owner decisions; coders never do.** Commit them with "owner, <date>".
- Idle peers don't read their inbox: for a new briefing, ask the owner to paste it; to wake a coder, ask the owner.
- Ask coders to confirm a diagnosis by ablation before fixing. Check reviewers' claims in the source before relaying.
- Squash-merged stacked PRs conflict: cherry-pick onto a fresh branch. Keep both sides of log conflicts.
- The worktree guard refuses compound git commands and `gh` via Bash: run plain git commands; gh via PowerShell
  `& "C:\Program Files\GitHub CLI\gh.exe"` (jq quoting: avoid `\"` in -q).
- `cargo xtask perf` only in my perf windows (ADR 0020); `--machine-ready` is the owner's.
- Goldens only via `cargo xtask golden-review` (ADR 0017); escalations go to the owner's `/bless`.
