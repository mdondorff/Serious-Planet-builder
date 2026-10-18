# 0064 The render floor's roots load outside the per-frame upload cap
Status: accepted (opus-planner, 2026-10-18, under the owner's delegation; easy to reverse). Amends ADR 0033 point 9.
Date: 2026-10-18
Source: `render-floor-3` (#303, sonnet-crusader); spec-guardian finding on STRM-006

## Context
- ADR 0062 point 3 sets a render floor at level 3; its 384 floor tiles (the roots) must all be resident before the first frame, because nothing coarser is drawn.
- ADR 0033 point 9 caps tile uploads per frame (STRM-006) and defers the excess to later frames.
- Under the cap the roots' start-up load (about 3.4 MiB) spreads over several frames, so the first frames would draw an incomplete planet.
- The roots load before frame 0, outside any measured frame (REND-017's no-initialisation-in-a-measured-frame rule).

## Decision
1. The roots' load (the floor level, at any floor including 0) is exempt from the per-frame upload cap. Every other tile upload stays under the cap.
2. The exemption is stated in STRM-001 and STRM-006; the numbers are the specs'.

## Consequences
- The first frame shows the whole planet at the floor level; a cold start costs the roots' generation or load time once (17.8 s for 384 roots with the pack ignored, indicative; the pre-fill avoids it).
- Two scheduler digests move (floor 0's six roots are exempt too).
- The CON-21 2 ms upload limit is untouched: the roots load before frame 0.

## Licences of adopted code or techniques
None.
