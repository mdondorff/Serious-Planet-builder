# 0005 Development environment and workflow
Status: proposed
Date: 2026-10-10
Source: background report §9 (Development workflow), Key Finding 8; starter doc; constitution CON-25, CON-26

## Context
The reference machine "Rasierklinge" runs Windows 11. WSL2 reaches the GPU only through D3D12 passthrough and Mesa's non-conformant dzn Vulkan layer, which is unsuitable for GPU tuning. The owner decided on 2026-10-06 (report Revision 2).

## Options (with trade-offs)
1. **Windows-native** (VS Code + Claude Code, MSVC Rust toolchain), Linux through CI.
2. Windows + WSL for Linux parity: two checkouts, sync cost, no GPU benefit.
3. WSL-primary: wrong GPU path for performance work.

## Decision
Option 1. All commands go through `cargo xtask` (std-only crate, no shell or PowerShell scripts in the repo). CI runs Windows (WARP) and Linux (lavapipe) from the first push. Tests run through `cargo-nextest`; tier selection is by test binary name (`gpu_*` tier C, `oracle_*` tier B, everything else tier A). Adapter choice for tests: `PLANET_ADAPTER=software|hardware` (default software). Licences are checked by `cargo deny` in CI (CON-27).

## Consequences
- Contributors need Git, MSVC Build Tools, rustup, `cargo-nextest`, `cargo-deny`; Node and OpenSpec for specs; `gh` for PRs. Missing tools are reported by the task that needs them, not worked around.
- Easier: Claude can run the real-GPU harness and read the screenshots locally. Harder: Linux-only failures appear only in CI.

## Licences of adopted code or techniques
Tooling only (cargo-nextest, cargo-deny: MIT/Apache-2.0).
