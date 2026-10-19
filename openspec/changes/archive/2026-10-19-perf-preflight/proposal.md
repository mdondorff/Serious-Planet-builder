## Why
A perf window costs the owner's quiet laptop for hours. Several windows were lost or voided after they started: a busy machine, a slow planner terminal (ADR 0065), a battery. The M5 process review (improvement 3, approved by the owner, relayed by opus-planner 2026-10-19) asks for a short check before a full window that says why it must not start.

## What Changes
- **Scene `preflight`** (TEST-040): `cargo xtask perf preflight` runs the readiness gate (ADR 0020) and the CPU calibration (ADR 0065) three times, about 30 s apart, over a load-sampled period of about 70 s, and records the top 5 CPU processes, the AC state and the power plan. It refuses (non-zero exit, every reason with its measured value and limit) when the gate fails or a calibration reading lies over a limit of ADR 0065 before a case. It writes `preflight.md` into `--out DIR`.
- **Decision logic** is a pure function (`planet_perf::preflight::decide`) over the measured facts, with a tier-A test per refusal and one for a pass.
- **`--smoke`** runs one round on any adapter, prints the same report and never refuses (a smoke run is no measurement).
- **`--machine-ready`** (owner only) prints the failures and passes, as at the gate.
- The window protocol (`docs/perf/window-13-plan.md`) names the preflight as the first step.

## Impact
- Specs: TEST-040 (added, testing).
- Code: `perf` (`preflight.rs`), `app` (the scene), `xtask` (`perf preflight`).
- Goldens: none. Perf: the preflight is not a measurement and takes no perf window.
- Fidelity: the limits are those of ADR 0065 (the quiet-machine spread plus about 5 %); the worst of three readings must pass, so a clean machine passes the preflight as often as a scene passes its own check.
- spec-guardian before archive; numerics-reviewer not needed.
