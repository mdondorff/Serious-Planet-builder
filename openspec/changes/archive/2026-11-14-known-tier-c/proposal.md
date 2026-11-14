## Why
- The local tier C on Rasierklinge has not run green since the display changes (2026-11-09): WARP aborts 0xc0000005 in changing tests, the gpu_skeleton pipelined-loop timeout under oversubscription, gpu_preflight's 8 s bound under load (docs/status.md Tracked).
- The architect's rule of 2026-11-09 let a push go when only those classes failed, but each case cost a round trip to the lead; three for one PR on 2026-11-14 (lead, explorer #149).

## What changes
- BUILD-056: `xtask/known-tier-c.txt` lists the known classes (test pattern, outcome, cited Tracked entry). When `test gpu` fails in prepush, prepush reads the run's log and passes the step only when every failure is in a listed class whose Tracked entry exists, the run ended and no candidate golden was written; it prints "tier C: N known-class failure(s), listed" for the PR body.
- Local only; no test or tolerance changes; CI runs tier C in full.

## Impact
xtask (knowntierc.rs, prepush.rs, testcmd.rs `candidates` made public, main.rs).
