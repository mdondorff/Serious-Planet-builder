## Context
Verifier finding on M1 (PLANET_GOLDEN_STRICT is 0 in CI and a deleted golden passed).

## Decisions
- Per-adapter strictness instead of a global switch: the Windows adapter is blessed, lavapipe is not yet.
- Candidate listing compares bytes with the golden.
