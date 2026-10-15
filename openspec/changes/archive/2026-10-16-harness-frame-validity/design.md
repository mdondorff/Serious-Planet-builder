## Cause table

One pure function (`planet_perf::validity::SessionVerdict::assess`) decides; the harness, the log writer, the console and accept use it.

| cause | voids frame verdicts | voids GPU verdicts |
|---|---|---|
| smoke run | yes | yes |
| not the reference adapter | yes | yes |
| power scheme not a performance scheme, or unreadable | yes | yes |
| thermal / hardware throttling (0x8, 0x20, 0x40, 0x80, unparseable), noise above 10 % after the retry, not the production source (ADR 0020, 0036) | yes | yes |
| owner override of the readiness gate | yes | yes |
| a run started with the GPU in P5 or lower (idle start) | NO | yes (session) |
| a view's clock sample below 1,400 MHz or missing (ADR 0036) | NO | yes (that view; already per view) |
| software power cap 0x4 | no (recorded) | no |

The readiness gate (ADR 0020) refuses before measuring and is unchanged; its failures appear in the log only through the owner's override.

## Log fields

`valid_for_budgets` is unchanged (true only with no cause at all). New: `frame_valid`, `gpu_session_valid`, `gpu_only_causes`. `warnings` keeps every cause text. A log without `frame_valid` is frame-valid when `valid_for_budgets` is true, or when it is false and every warning is the idle-start warning (the warnings list every cause, so nothing else voided it); otherwise invalid. So the logs of window 1 (written before this change) are re-judged correctly by `accept M4`; if an old log cannot be classified it is refused and the window is re-run. GPU session validity of an old log is `valid_for_budgets`. `gpu_valid` and `gpu_clock_state` per run are unchanged.

## Console

`SESSION VALID for budgets: frame verdicts: yes; GPU verdicts: yes ...`, or `SESSION VALID for frame verdicts: yes; GPU verdicts: NOT VALID (<cause>)`, or `SESSION INVALID for budgets: frame verdicts: NO; GPU verdicts: NO ...` plus one line per cause. Printed once, from `SessionVerdict::lines()`.

## M4 exit criteria

No gated criterion is a GPU time (docs/status.md: `gpu_ms` in pipelined scenes is not needed for the M4 exit), so a window whose sessions started at P8 is judged on its frame, RAM, slots and lag verdicts.
