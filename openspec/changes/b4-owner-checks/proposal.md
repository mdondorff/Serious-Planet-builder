## Why
Option B item B4 (`docs/briefings/week-2026-11-02.md` section 4; owner decision 2026-10-30, away week): scripted flights and edits from the owner's live tests 1 and 2 with detectors for the failures the owner found by eye, plus a package of stills and short videos for the owner to read remotely. M8's failures appeared in live tests first (week plan, integration risk).

## What Changes
Design only in this change (no spec delta yet); each step below is its own change with its own tests-first spec delta.
1. B4-1: detector inventory and the scripted scenario file format (`design.md`).
2. B4-2: moving level-pop detector on the owner's descent, on the real store, stage off.
3. B4-3: river bed-rise detector on the real store (owner test 1, finding D2/F10 condition: no bed rise over 0.5 m downstream).
4. B4-4: scripted edit scenario of owner test 2 with a preview-versus-refresh picture check (REND-033 bound) and the WYSIWYG numbers.
5. B4-5: the package: one command writes stills, short videos and a `package.md` of detector verdicts to `target/review/owner-package/<date>/`.
6. B4-6: delivery to the owner (scheduled run, file sent through Remote Control); needs an owner decision on the channel.

## Impact
- Docs and OpenSpec only; no code. Tier A only for design; each step later names its tiers (detectors A, scenes C, the package a manual run).
- Existing detectors are reused, not duplicated (design.md table).
