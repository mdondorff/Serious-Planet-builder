# Briefing for sonnet-explorer: M5 Atmosphere & ocean (opus-planner, 2026-10-16)

The owner accepted M4 on 2026-10-16 and started M5. Your queue is "Queued for M5" in `docs/status.md`; this file tells you how to run it.

## Read first
1. `docs/constitution.md` (v1.4: CON-07 fields, CON-11 geometry strict / shading by tested bound) and `CLAUDE.md`.
2. `docs/status.md`: the M5 design constraints, the owner's M5 decisions and the queue (items 0-10).
3. ADRs 0035 (fields vs presentation data), 0037 (M5 goldens, look reviewed at the gate), 0038-0042 (the M5 design), 0043 (CON-11 v1.4), 0045 point 5 (readiness report), 0046 (worker priority). Specs: only those an item names.

## How M5 runs without the owner's attention
- **Autonomy:** the owner reviews the look once, at the M5 gate (ADR 0037). New M5 goldens go through `golden-review` and are blessed under its policy; escalations go to the owner's `/bless` as before.
- **Defaults:** look parameters are data with physically based Earth defaults (look file, item 3).
- **Time boxes and fallbacks:** each ADR names a time box and a fallback (Bruneton 2008 for the atmosphere, flat water for the waves). Take the fallback without asking when the box runs out, and report it.
- **Every change** opens its spec deltas with each attribute's class (field or presentation) and bound (ADR 0035). Fields follow CON-07 and CON-14 (CPU reference, `libm`, known answers); presentation shading declares and tests its bound, including temporal stability at 1 m where planet-sized values are used (CON-11 v1.4).
- **Perf windows:** ask opus-planner. It authorizes the window; you run it from your session (owner, 2026-10-16). Validity: `load_calib_before_single_us` <= 50,000 and `load_calib_before_parallel_median_us` <= 100,000 per gated scene. Never start a window without the authorization.
- **`test fast` budget:** 20 s (owner, 2026-10-16).
- **Parallel spikes:** sonnet-spikes may run explorations in its own worktree. It writes only its own files; opus-planner coordinates.

## Order
Items 0-10 in `docs/status.md`, one OpenSpec change and one PR each:
0. The housekeeping list, then the 20 s budget in xtask.
1. The viewer window with the adapter gate.
2. Render graph and HDR.
3. The look file.
4. Sun and base shading.
5. The atmosphere.
6. The ocean bake and the water level.
7. The water pass.
8. The M5 perf scenes.
9. The tier-B perf smoke.
10. M5 acceptance.

Item 9 may move earlier if a change needs it. Item 8's harness logging of system-wide CPU load is useful before the first M5 window.

## Stop and report when
A perf measurement is needed (ask for a window), a fallback was taken, an ADR's text does not fit what the code needs (draft an amendment as proposed), golden escalations wait for the owner, or the M5 gate is reached (milestone gate: stop for the owner).
