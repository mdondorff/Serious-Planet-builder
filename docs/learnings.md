# Learnings

Append-only. Added with `/learning`. Each entry names the assumption it changes (spec ID, ADR number, constitution rule, or background report section) and the proposed follow-up.

## 2026-10-10 wgpu 30 headless API differs from older documentation
What happened: first compile of the hello triangle against wgpu 30.0.1 failed on API drift: `RequestAdapterOptions` needs `apply_limit_buckets`, `Instance::new` takes `InstanceDescriptor::new_without_display_handle()`, `RenderPipelineDescriptor` has `multiview_mask`, `get_mapped_range` returns a `Result`, error scopes are popped with `scope.pop()`. Fixed from compiler output in `crates/render/src/lib.rs`; the software adapter (`force_fallback_adapter`) selects WARP on Windows (`Dx12 Microsoft Basic Render Driver`, device type Cpu) and rendered correctly.
Assumption it changes: background report §9/§17 item 18 (wgpu API churn): confirmed, not refuted.
Proposed follow-up: none beyond ADR 0004's pin-and-upgrade-at-milestones rule; record each upgrade here.

## 2026-10-10 nextest rejects a filterset that matches no binary
What happened: `cargo nextest run -E 'not binary(~gpu_) and not binary(~oracle_)'` failed with "operator didn't match any binary names" while no `oracle_*` test binary existed. Tier selection by binary-name prefix therefore needs at least one binary per tier at all times.
Assumption it changes: ADR 0009 (tier naming convention): refined, not refuted.
Proposed follow-up: `crates/render/tests/oracle_shader_validation.rs` (REND-002) is the standing tier-B binary; if it is ever removed, another tier-B test must replace it (noted in the M0 design).

## 2026-10-10 GitHub CLI is not installed on Rasierklinge
What happened: `gh` is not on PATH in Git Bash or PowerShell, so `gh pr create`, CI status checks and the "merge only when CI is green" rule in CLAUDE.md cannot be followed. The remote `origin` is configured (https://github.com/mdondorff/Serious-Planet-builder.git).
Assumption it changes: starter doc §2 (install table) assumed `gh` present.
Proposed follow-up: owner installs `winget install GitHub.cli` and runs `gh auth login`; until then work stays on local branches and CI has not run (listed under "Waiting for the owner" in docs/status.md).
