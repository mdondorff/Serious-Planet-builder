## Why

The owner accepted the CON-14 amendment in ADR 0010 (2026-10-10): the former "1e-4 relative error" bound was undefined near zero and allowed 0.4-0.8 m height error, about 400x CON-13's 1 mm budget.

## What Changes

- Modified: GEN-008 (per-field mixed bounds; neighbouring tiles share one `implementation_id`; new scenario). Still planned: no GPU generator exists.

## Impact

Specs only. No code, no goldens.
