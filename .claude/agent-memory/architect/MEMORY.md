# Architect memory

Refreshed 2026-10-24 during M8 (session `opus-planner`, worktree `.claude/worktrees/opus-planner`). Confirm against `main` and `docs/status.md` before acting. The repo records decisions (ADRs, specs, log); this file keeps only what it does not. Next week's start, loop lines and the coordination protocol: `docs/briefings/week-2026-10-25.md`.

## Sessions
- **sonnet-explorer** (worktree `sonnet-m4`, address `sonnet-explorer [fa41e9]`): main lane; runs perf windows for its own work on the planner's go.
- **sonnet-crusader:** support lane; runs perf windows for its scenes on the planner's go; cleared 2026-10-24, so the next briefing must be self-contained.
- **sonnet-spikes:** scouts the next milestone's questions (owner, 2026-10-24); unreachable on 2026-10-24.
- Claude Desktop sessions, cleared or idle, wake on an incoming message; no paste needed. Check `ListAgents` for exact names.

## State (2026-10-24 evening)
- M8 natural landscape: M8-2 archived (#621); M8-3 steps 1 to 4 merged (A-net in the viewer, 8 m cells, top level 15, chunk 4.2 s indicative); M8-5 to frame data (#628), render design #629; M8-V tile-streaming curve (#619); global run 545 s for 100 steps (criterion 4 met, synthetic input).
- Criterion 3 amended by the owner (#634). Open owner item: D3 (edits on top of or under erosion) before M8-7.
- Look review 1 (M8-4) is next after explorer's step 5 renders.

## Practice
- Owner decisions are committed in the planner session; coders merge them.
- Estimates for the owner: PR count, calendar dates, quota share. Observed rate: two lanes about 1 % per hour, about 0.25 % per PR.
- Escalate only exit criteria, constitution and open trade-offs (push notification); decide reversible design questions yourself and say so.
- Check a coder's headline number against what the criterion actually measures (2026-10-24: a "0.19 m step" was measured against the pyramid, not against the base; the real entry step was metres).
- Mass deletions and archive steps run from the planner's session after the owner's go.
- Repro hunts are time-boxed to about 1 h.
