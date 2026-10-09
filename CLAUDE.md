# Planet builder: agent operating manual

## What this is
A planet-scale generator, editor and real-time renderer: Rust + wgpu, one binary with three run modes (editor/viewer, `generate` headless CLI, `test-render` headless harness). Developed spec-first with OpenSpec. Primary machine: "Rasierklinge" (Windows 11, RTX 3080 Ti Laptop GPU, 16 GB).

## Sources of truth (highest first)
1. `docs/constitution.md`: non-negotiable rules. Read it at the start of every session. Never edit it; propose amendments as ADRs.
2. `docs/adr/`: accepted decisions. Never edit an accepted ADR except its status line.
3. `openspec/specs/`: current subsystem specs. Read only the 2–3 specs your task touches.
4. `docs/background/`: frozen research and setup docs. **Read only when an ADR, spec or task points at a specific section** (e.g. "background report §5"). Never treat them as requirements, never update them.

Working files: `docs/status.md` (progress log and stop reports), `docs/spikes.md` (spike register), `docs/learnings.md` (append-only findings).

## Commands (always via xtask)
- `cargo xtask test fast`: tier A for changed crates (< 10 s)
- `cargo xtask test`: tiers A + B on the software adapter
- `cargo xtask test gpu`: tier C goldens and debug views
- `cargo xtask spec-lint`: trace IDs, tiers, requirement coverage
- `cargo xtask accept <milestone>`: milestone acceptance
- `cargo xtask repro <bundle>`: replay a repro bundle
- `cargo xtask perf`: Rasierklinge only, and only after the owner confirms the machine is ready (charger, power profile)

Until an xtask command exists, create it in `xtask/` rather than writing a script. The shell is Git Bash on Windows; keep commands portable and never add `.sh` or `.ps1` files to the repo.

## How to work
1. Pick up work as an OpenSpec change (`openspec/changes/<id>/`): proposal, design, tasks, spec deltas. Keep changes small (one subsystem, one milestone step).
2. For each task: write the failing test at the cheapest tier that can falsify it, then implement. Every test cites its requirement ID (`// spec: COORD-003`).
3. Run the `numerics-reviewer` agent for changes in `core`, `generators` or shaders, and the `spec-guardian` agent before archiving a change.
4. Run the `verifier` agent before declaring any task done. Fix what it finds; don't argue with it.
5. Archive the change in OpenSpec when all its requirements pass, then log one line in `docs/status.md`.
6. A new decision → `/adr`. A time-boxed experiment → `/spike`. A surprise or a corrected assumption → `/learning`.

## Rules that are easy to break
- The renderer executes a `FramePlan`; decisions live in GPU-free crates (CON-03).
- f64 on the CPU, tile-local or camera-relative f32 on the GPU; no planet-radius values in shaders (CON-11).
- `libm` for transcendentals in generator code; no thread-count-dependent results (CON-14).
- Failure output must be actionable: numeric deltas, tile IDs, diff images in `target/review/`.
- Never bless golden images; queue candidates and ask the owner (`/bless` is owner-only).
- Never weaken, skip or delete a test to make a build green. Quarantine only with a tracking entry in `docs/status.md`.

## Git
- One branch per OpenSpec change (`change/<id>`). Small commits with clear messages.
- Push the branch and open a PR with `gh pr create`. Merge it yourself (squash) only when CI is green on Windows and Linux and the verifier passed.
- Never force-push, never rewrite `main`'s history, never commit secrets or large binaries (goldens are small PNGs; anything over 1 MB needs an ADR).

## When to stop and report
Write a stop report in `docs/status.md` (what was done, what is blocked, what you need) and stop when:
- a test can't be made deterministic;
- a decision is needed that no ADR or spec covers **and** it is hard to reverse (draft the ADR as "proposed" first). Easy-to-reverse decisions: draft a "proposed" ADR and keep going on it. Proposed ADRs from the background report's "decide now" list may be built on before the owner accepts them;
- golden images are waiting for blessing and further work depends on them;
- a performance measurement is needed on Rasierklinge;
- a milestone go/no-go gate is reached (after M2 and after M5);
- the constitution would have to change.
Otherwise, keep going.
