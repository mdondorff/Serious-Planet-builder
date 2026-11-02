## Why
Tracked owner bug (2026-10-30): `editor --edit --site LAT,LON` opened at the default view with no descent, while `--marker` with G worked. The scripted flight is built over the site (`build_script_at`), but the editor mode edits from the free camera and took it over at the first frame, from the flight's first camera (the orbit view).

## What Changes
- With `--edit` and `--site` (not a smoke run) the editor mode starts off; the flight descends over the site; the mode turns on where the descent ends (the free camera takes the landing pose), or at once at a Tab.
- `window::descent_finished`; VIEW-022 gains the scenario Site descent (the statement is at its word limit) and its first bullet says `--edit` starts the mode on, except with `--site`; `site_descent_wanted` and `descent_wait_step` are the pure steps of the window; tier A test `site_descent` (the script starts high over the site and lands over it; the wait ends at the last camera or a Tab, only an `--edit --site` run waits).

## Capabilities
### Modified Capabilities
- `viewer`: VIEW-022
