## Decision

The evidence is the scene's own code, not a result: spec-lint checks that the scene which measures the requirement exists and names it; whether it passes is the architect's perf window (CON-20). The rule is deliberately narrow (function name `perf_*`, directory `crates/app/src`) so a stray comment cannot satisfy a tier-C requirement. Scene citations count for tier C only. The constitution is untouched (CON-16 and CON-17 stay as written).
