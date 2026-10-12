# Architect memory

Refreshed 2026-10-11 (updated 2026-10-12 for #64, #65) at the hand-over from the long `opus-planner` session (the session that drafted this agent).
Confirm everything here against `main` and `docs/status.md` before acting on it.

## Sessions
- **sonnet-coder:** finishing `change/m3-cache-verify`, then STOPS for a full reset (owner, 2026-10-12: its context is
  huge). It writes a hand-over in status.md or its PR. Do not give it new work.
- **sonnet-explorer:** takes over the remaining M3 queue after cache-verify (owner, 2026-10-12). Morph-height done (#65).
  It has none of sonnet-coder's M3 context: brief it fully (status.md M3 queue, ADRs 0018/0019/0021-0024, the hand-over).
- **sonnet-worker:** deprecated. Do not message it.
- This session's name is `opus-planner` (start with `claude --agent architect -n opus-planner`). Coders report to it.
  On 2026-10-12 a new architect session (worktree `opus-planner`) took over; the owner was asked to stop the old
  background `opus-planner` session. Two sessions with one name: check `ListAgents` refs.

## State of main (2026-10-11)
- M2: GO recorded by the owner (#42), "go with changes"; the condition (LOD-008 first in M3) is met (#43).
- All ADRs 0001-0020 and 0022-0024 accepted; 0021 in part (1, 3, 4, 7) on 2026-10-12 under the owner's delegation
  (#67); 0023/0024 amend 0018/0019 in part. 0022 = option 1 (no same-stage reads). Recent: 0016 height-aware selection, 0017 golden review policy, 0018 persistent
  cache keys (BLAKE3-128, `pure`), 0019 tile pack store, 0020 perf readiness gate. 0013, 0014 accepted 2026-10-11
  (0014 partly superseded by 0016).
- Constitution v1.2 (CON-14 per-field CPU/GPU bounds; CON-18 golden approval by owner or ADR 0017 policy).
- M3 done on main: perf gate (#55), LOD-008 (#43), plan cache (#52, the owner's run: 6.3 ms warm at ground-1m against
  8 ms), world-def/GenContext (#53), TOML definitions (#57), cell index (#58), multi-body keys (#56), stage model (#60).
- M3 next (sonnet-coder): item 5 tile storage (128-bit keys #61 and the Hilbert-ordered tile pack #64 done; next cache-verify, cache-compact, compaction/eviction), item 6 field
  pipeline and hydrology (SPIKE-03), item 7 M3 acceptance and report (the gate: stop and report to the owner).

## Open threads to follow up
1. **Ragged band: DONE** (#65, 2026-10-12): both causes fixed (height now geomorphed, MESH_VERSION 4; the relief
   grid's f32 phase replaced by per-decade f64 phases). `terrain_aerial200km_height` was auto-blessed under ADR 0017
   (AUTO-E: 849 / 829 / 843 px). Open follow-up: normals are still not geomorphed (only if it shows in a view).
2. **Relief goldens** `terrain_orbit_relief`, `terrain_ground1m_relief`: first goldens, classed lit, pending for the
   owner's `/bless`; candidates regenerated after #65, so the owner can review them now. Review sequence:
   `C:\DEV\Serious-Planet-Builder\.claude\worktrees\sonnet-explorer\target\review\flight-relief-nvidia\` (contact sheet).
   This is the TEST-014 owner review.
3. **First perf-gate run** (ADR 0020): open a perf window (ask coders to pause builds and tests, wait for their idle
   notices), run `cargo xtask perf`, record the result. Watch the laptop's software power-cap throttle bit (0x4): if it
   invalidates every session, the measurement protocol needs an owner decision.
4. **M3 acceptance must show** realistic terrain plans at production parameters (cells 32, ratio 240); ADR 0016 rejects
   terrain too steep for a crack-free morph interval. The steep stress field only plans at cells 4 / ratio 77.
5. **Process proposal for the owner:** per-change log lines in the archived OpenSpec change instead of appending to
   `docs/status.md`'s Log (it conflicts on nearly every merge).
6. **CON-08 reading** (owner): needed before SPIKE-03 splits a regional chain (ADR 0022 option 2).
7. **Cache follow-ups** queued in status.md M3 item 5 (a)-(d) from the 0023/0024 reviews.
8. Owner items carried into M3: TEST-008 split decision; editor window and egui per ADR 0015 (wgpu stays in render).

## Lessons from the owner
- **ADR delegation (owner, 2026-10-12):** accept ADRs yourself when the reviewers' recommendations are strong or the ADR
  is sound and unambiguous; bring the owner only constitution readings/amendments and genuinely open trade-offs. Run
  spec-guardian (+ numerics-reviewer for keys, formats, generators), apply text fixes, record "owner's delegation of
  2026-10-12" in the status line. Clarification ADRs that go beyond an accepted ADR's text must say "amends in part".
- One ADR at a time: explain plainly, highlight what matters most with its cost to reverse, recommend, then he
  decides. Small focused PRs per decision. Ask only questions that change the work; offer a recommended option.
- He prefers autonomy: proceed on reversible decisions; bring owner-level choices only.
- **You execute owner decisions; coders never do.** A coder refusing a relayed decision is correct (sonnet-coder refused
  to accept ADR 0016 on a relayed message). Commit decisions with "owner, <date>" on main or on the PR branch holding
  the file; coders merge them.
- An idle peer session does not read its inbox until it gets a turn (also when it hit a session limit). For a new
  coder, write the briefing to a file and ask the owner to paste it; if a coder stays idle, ask the owner to nudge it.
- Your own diagnoses can be incomplete too: the ragged band was blamed on the unmorphed height (real, but millimetres at
  1 m); sonnet-explorer found the real cause by switching shader parts off one at a time. Ask coders to confirm a
  diagnosis by ablation before fixing.
- Check reviewers' claims in the source before relaying them: they were wrong once (a mapping id already in the key)
  and right in ways that mattered (stale golden candidates, the missing driver gate, the morph-height defect).
- Stacked PRs and squash merges conflict: after the base is squash-merged, cherry-pick the stacked commits onto a new
  branch from main. `docs/status.md` conflicts on almost every merge: keep both sides of the Log.
- The worktree guard in this repo refuses compound git commands: run git as plain, separate commands.
- `cargo xtask perf` runs only via the architect's perf window (ADR 0020); coders never run it. Overriding a failed
  gate (`--machine-ready`) is the owner's.
- Goldens: agents bless only through `cargo xtask golden-review` (ADR 0017); first goldens, lit views, driver changes
  and disagreements go to the owner's `/bless`. Golden tests report every mismatch of a run (`GoldenChecks`).
