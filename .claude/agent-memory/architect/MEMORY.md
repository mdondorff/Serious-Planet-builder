# Architect memory

Refreshed 2026-10-24 during M8 (session `opus-planner`, worktree `.claude/worktrees/opus-planner`). Confirm against `main` and `docs/status.md` before acting. The repo records decisions (ADRs, specs, log); this file keeps only what it does not. Next week's start, loop lines and the coordination protocol: `docs/briefings/week-2026-10-25.md`.

## Sessions
- **sonnet-explorer** (Desktop session, worktree `start-879e4b` since 2026-10-18; crusader `sonnet-crusader-c1a4f2`, spikes `sonnet-spikes-36fc88`): main lane; runs perf windows for its own work on the planner's go.
- **sonnet-crusader:** support lane; runs perf windows for its scenes on the planner's go; cleared 2026-10-24, so the next briefing must be self-contained.
- **sonnet-spikes:** scouts the next milestone's questions (owner, 2026-10-24); unreachable on 2026-10-24.
- Claude Desktop sessions, cleared or idle, wake on an incoming message; no paste needed. Check `ListAgents` for exact names.
- 2026-10-25: this planner session is `opus-planner [fa3069]`; a stale Remote Control row `opus-planner [d8d159]` shares the name, so coders address `[fa3069]`. After an app restart the sonnet-* Desktop sessions were missing from `ListAgents` until opened.
- 2026-10-26: after a clear a lane gets a new ref (crusader is now `sonnet-crusader [3f46e7]`, explorer `[cb2991]`, spikes `[198570]`); re-check ListAgents after every clear.

## State (2026-10-26 early)
- Owner scope (2026-10-25): M8 = plausible relief, hydrology right, horizon shadows; realistic terrain is M9. ADR 0081 accepted; ADR 0080 and ADR 0082 (node slides, #695) proposed, owner to accept.
- Done overnight: #686 key table, #689 stall probe, #691 heavy lock, #692 routing (ADR 0081), #694 run CLI, #698 fade stability, #699 coarse-link tributaries (ANET 7, fine tier = M9), SPIKE-43 (bounded breach local within the 375 halo; open: halo ground differs between separate solves).
- In flight: crusader closes m8-chunk-fade (stale-mesh clock fix, reviewers, archive; #696), then Keys tab, dark spots A+B; explorer closes M8-3 then proposes M8-6 driver maps (proposal to planner before code); spikes idle.
- Owner items: /bless; ADR 0080/0082; D3 before M8-7; look review 2; store compaction (24 GB); approve a real-elevation download source for M9 scouting.
- Shared store is current (bake v5, run stored, coarse horizon written; verified 2026-10-25 by crusader).
- Quota: 11 % of the weekly quota used on 2026-10-25 (first day after reset) with three lanes and the planner most of the day.

- Shared store (owner, 2026-10-25): `C:/DEV/SeriousPlanetData/global16384`; worktrees and the root link to it by junction (a .lnk shortcut does not work); one writer at a time via writer.lock. Owner idea for later: stores behind a local or remote data server (roadmap).

## Practice
- Owner decisions are committed in the planner session; coders merge them.
- Estimates for the owner: PR count, calendar dates, quota share. Observed rate: two lanes about 1 % per hour, about 0.25 % per PR.
- Escalate only exit criteria, constitution and open trade-offs (push notification); decide reversible design questions yourself and say so.
- Check a coder's headline number against what the criterion actually measures (2026-10-24: a "0.19 m step" was measured against the pyramid, not against the base; the real entry step was metres).
- Mass deletions and archive steps run from the planner's session after the owner's go.
- Repro hunts are time-boxed to about 1 h.
