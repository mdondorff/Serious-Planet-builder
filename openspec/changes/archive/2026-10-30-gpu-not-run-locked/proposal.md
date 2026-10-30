## Why
Owner decision 2026-10-30: real-GPU window checks report NOT RUN while the Windows session is locked or in modern standby, instead of failing or timing out. Explorer measured the cause: present and acquire stalls of 1.3 to 2.7 s and device losses only while the session is locked; the dense-cluster sweep takes 26 s unlocked and times out at 300 s locked. Headless and software-adapter runs are unaffected.

## What changes
- `planet_perf::desktop`: a cheap, locale-independent detector (the lock screen process `LogonUI.exe` in `tasklist /FO CSV`; elsewhere unlocked), a `guarded` runner (NOT RUN before the run, or after a failure when the desktop is locked by the end, with the times), the NOT RUN line and a marker file in `PLANET_NOT_RUN_DIR`.
- `editor --smoke` on the discrete GPU, the `gpu_window` tests on the hardware adapter and `cargo xtask gate` (real store) report NOT RUN on a locked desktop.
- `cargo xtask test` prints the NOT RUN checks of a run; `cargo xtask accept` marks a step with NOT RUN checks INCOMPLETE and returns an error: never PASS.

## Not changed
The software adapter, headless scenes and CI. The rule against skipping tests (CLAUDE.md) holds: this is a reported, reasoned NOT RUN by owner decision, counted in the summaries.

## Limits
The probe sees a locked session, not a display that is off or modern standby without the lock screen; a check in those states fails as before. A failed probe reads as unlocked.
