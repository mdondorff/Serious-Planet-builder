## ADDED Requirements

### Requirement: TEST-072 Real-GPU window checks are NOT RUN on a locked desktop
While the Windows session is locked (the lock screen process `LogonUI.exe` is running), a real-GPU window check (the `gpu_window` tests on the hardware adapter, `editor --smoke` on the discrete GPU, `cargo xtask gate` on the real store) SHALL print a `NOT RUN` line with the reason and the time, leave a marker, and return, never a pass. A failure of such a check with the desktop locked by the end of the run SHALL be a NOT RUN with the times. `cargo xtask` SHALL count the NOT RUN checks of a run, and `accept` SHALL mark a step with any INCOMPLETE and fail. The software adapter, headless scenes and CI are not affected.

Verify: A
Status: active
Source: owner (2026-10-30), explorer's measurement of the locked-session stalls, CLAUDE.md

#### Scenario: Locked at the start
- **WHEN** the detector says the desktop is locked and a hardware check is guarded
- **THEN** the check does not run, a NOT RUN line with the reason and a marker are written, and the software adapter runs the check as usual

#### Scenario: Locked mid-run
- **WHEN** the desktop is unlocked at the start, the check fails and the desktop is locked at the end
- **THEN** the result is a NOT RUN naming when the run started and ended, and a failure with the desktop unlocked stays a failure

#### Scenario: Cut off by the timeout
- **WHEN** a hardware window test is still running after 240 s and the desktop has been seen locked meanwhile
- **THEN** a watchdog reports the NOT RUN with the times and ends the process with success before nextest's timeout kills it; a desktop never seen locked leaves the test to its own limit

#### Scenario: Accept
- **WHEN** an acceptance step has NOT RUN checks
- **THEN** its row says INCOMPLETE and not pass, and the acceptance run returns an error
