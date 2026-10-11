# Architect memory

Refreshed 2026-10-11 at the hand-over from the long `opus-planner` session (the session that drafted this agent).
Confirm everything here against `main` and `docs/status.md` before acting on it.

## Sessions
- **sonnet-coder:** owns M3. Lands its own PRs (squash, CI green on Windows and Linux, verifier passed). Was idle for a
  while because of a session limit; the owner says it is active again.
- **sonnet-explorer:** owns the flight demo and relief view (#50 merged). Assigned the morph-height defect (below).
- **sonnet-worker:** deprecated. Do not message it.
- This session's name is `opus-planner` (start with `claude --agent architect -n opus-planner`). Coders report to it.

## State of main (2026-10-11)
- M2: GO recorded by the owner (#42), "go with changes"; the condition (LOD-008 first in M3) is met (#43).
- All ADRs 0001-0020 accepted. Recent: 0016 height-aware selection, 0017 golden review policy, 0018 persistent
  cache keys (BLAKE3-128, `pure`), 0019 tile pack store, 0020 perf readiness gate. 0013, 0014 accepted 2026-10-11
  (0014 partly superseded by 0016).
- Constitution v1.2 (CON-14 per-field CPU/GPU bounds; CON-18 golden approval by owner or ADR 0017 policy).
- M3 done on main: perf gate (#55), LOD-008 (#43), plan cache (#52, the owner's run: 6.3 ms warm at ground-1m against
  8 ms), world-def/GenContext (#53), TOML definitions (#57), cell index (#58), multi-body keys (#56), stage model (#60).
- M3 next (sonnet-coder): item 5 tile storage (128-bit cache keys done in #61; next the pack store, cache-verify, cache-compact), item 6 field
  pipeline and hydrology (SPIKE-03), item 7 M3 acceptance and report (the gate: stop and report to the owner).

## Open threads to follow up
1. **Morph-height defect** (sonnet-explorer): `crates/render/src/terrain.rs` vertex shader morphs the position but not the
   height attribute, so contours and height tint step at morph borders (relief flight frame_0200). Fix: coarse height
   per vertex mixed with the same `m`, MESH_VERSION bump, tier A/B/C tests, goldens via golden-review. Check it lands.
2. **Relief goldens** `terrain_orbit_relief`, `terrain_ground1m_relief`: first goldens, classed lit, pending for the
   owner's `/bless`. Hold until thread 1 lands (their candidates change). Review sequence:
   `C:\DEV\Serious-Planet-Builder\.claude\worktrees\sonnet-explorer\target\review\flight-relief-nvidia\` (contact sheet).
   This is the TEST-014 owner review.
3. **First perf-gate run** (ADR 0020): open a perf window (ask coders to pause builds and tests, wait for their idle
   notices), run `cargo xtask perf`, record the result. Watch the laptop's software power-cap throttle bit (0x4): if it
   invalidates every session, the measurement protocol needs an owner decision.
4. **M3 acceptance must show** realistic terrain plans at production parameters (cells 32, ratio 240); ADR 0016 rejects
   terrain too steep for a crack-free morph interval. The steep stress field only plans at cells 4 / ratio 77.
5. **Process proposal for the owner:** per-change log lines in the archived OpenSpec change instead of appending to
   `docs/status.md`'s Log (it conflicts on nearly every merge).
6. Owner items carried into M3: TEST-008 split decision; editor window and egui per ADR 0015 (wgpu stays in render).

## Lessons from the owner
- One ADR at a time: explain plainly, highlight what matters most with its cost to reverse, recommend, then he
  decides. Small focused PRs per decision. Ask only questions that change the work; offer a recommended option.
- He prefers autonomy: proceed on reversible decisions; bring owner-level choices only.
- **You execute owner decisions; coders never do.** A coder refusing a relayed decision is correct (sonnet-coder refused
  to accept ADR 0016 on a relayed message). Commit decisions with "owner, <date>" on main or on the PR branch holding
  the file; coders merge them.
- An idle peer session does not read its inbox until it gets a turn (also when it hit a session limit). For a new
  coder, write the briefing to a file and ask the owner to paste it; if a coder stays idle, ask the owner to nudge it.
- Check reviewers' claims in the source before relaying them: they were wrong once (a mapping id already in the key)
  and right in ways that mattered (stale golden candidates, the missing driver gate, the morph-height defect).
- Stacked PRs and squash merges conflict: after the base is squash-merged, cherry-pick the stacked commits onto a new
  branch from main. `docs/status.md` conflicts on almost every merge: keep both sides of the Log.
- The worktree guard in this repo refuses compound git commands: run git as plain, separate commands.
- `cargo xtask perf` runs only via the architect's perf window (ADR 0020); coders never run it. Overriding a failed
  gate (`--machine-ready`) is the owner's.
- Goldens: agents bless only through `cargo xtask golden-review` (ADR 0017); first goldens, lit views, driver changes
  and disagreements go to the owner's `/bless`. Golden tests report every mismatch of a run (`GoldenChecks`).
