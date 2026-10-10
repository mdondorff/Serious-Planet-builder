## Context
Report §8, §10, §15 (M2); SPIKE-01, SPIKE-02; ADRs 0013, 0014.

## Decisions
- The controller is a pure state machine (heading, pitch, mode) with an injected terrain-height function; orbit derives its view from the position so the basis is never degenerate.
- The render graph is declaration plus analysis only; the terrain executor is not yet driven by it (one pass).
- The instance culling reference defines what REND-008 must match; no GPU culling pass is built in M2.

## Risks
- Declarations can drift from executors until a second pass exists; the graph tests compile a representative frame.

## Review follow-ups (verifier)

- Controller: non-finite input and negative or NaN steps are ignored; long steps are split (at most half the height above terrain per sub-step) so a large `dt` cannot tunnel through the planet; pitch no longer accumulates in orbit mode; orbit "up" now moves screen-up. The tests now cover terrain-aware speed, boost, an engaged clearance, pitch and yaw, sideways and orbit motion and zoom, and bad input; 15 surviving mutants of the first version were the motivation.
- Culling reference: documented as conservative (four side planes, no near/far plane); tests place instances near plane boundaries and check radius sensitivity and exact slot contents. Render graph: tests for latest-producer, side-effect roots, class and overlap rules, unclassified resources.
- Claims corrected: draw counts in ADR 0014 and SPIKE-02, SPIKE-01 caveats, spike list status, ADR 0013 wording.
