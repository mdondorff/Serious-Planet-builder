## Why
The owner's E3 smoke (S-Region-Authoring, 2026-11-28): `editor --seed 14 --regions FILE` exited, because the viewer fell back to the default store `target/accept/global16384`, which holds no bake of the current keys.

## What Changes
- `--regions` without `--store` computes the base in memory at the 1,024-cell size `--commit` uses outside its tiles; it never opens a default store (`window::viewer_store`).
- A default store that cannot be read is refused with the ways out: `--commit DIR`, `--global-res N`, `cargo xtask store`.

## Impact
- Specs: VIEW-052 modified (the store rule: no default store with `--commit` or `--regions`, the base in memory, the refusal's ways out); VIEW-077's text unchanged, two scenarios added.
- Code: `crates/app/src/window.rs`; its tier-A test cites VIEW-077.
