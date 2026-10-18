# Development screenshots

Progress snapshots the owner wants to keep: states of the project that are not goldens. They are not test references, and nothing reads them.

Name: `<YYYY-MM-DD>-<description>.png` (lowercase, dashes). Keep each file under 1 MB (anything larger needs an ADR).

| File | What it shows |
|---|---|
| `2026-10-17-atmosphere-orbit.png` | The viewer at orbit (altitude 7,225 km) with lighting and atmosphere on (EV 13.9, adapted), while the bake loads. The first look at the atmosphere and the ocean coastlines. Rendered by the software adapter (WARP) during a coder's test run, hence 1 fps and 249 ms of GPU time. Captured over remote desktop. |
| `2026-10-18-bug-water-tiles-missing-in-motion.png` | Bug report (owner, RTX, branch change/remove-vertex-render-path, #304): free camera at 82 m over a coast; dark triangle-edged patches on the sea near the camera, where the water surface is missing and the seabed shows. They appear only while the camera moves or rotates, never when it is still. Tracked in docs/status.md. |
