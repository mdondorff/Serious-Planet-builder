# Development screenshots

Progress snapshots the owner wants to keep: states of the project that are not goldens. They are not test references, and nothing reads them.

Name: `<YYYY-MM-DD>-<description>.png` (lowercase, dashes). Keep each file under 1 MB (anything larger needs an ADR).

| File | What it shows |
|---|---|
| `2026-10-17-atmosphere-orbit.png` | The viewer at orbit (altitude 7,225 km) with lighting and atmosphere on (EV 13.9, adapted), while the bake loads. The first look at the atmosphere and the ocean coastlines. Rendered by the software adapter (WARP) during a coder's test run, hence 1 fps and 249 ms of GPU time. Captured over remote desktop. |
| `2026-10-18-bug-water-tiles-missing-in-motion.png` | Bug report (owner, RTX, branch change/remove-vertex-render-path, #304): free camera at 82 m over a coast; dark triangle-edged patches on the sea near the camera, where the water surface is missing and the seabed shows. They appear only while the camera moves or rotates, never when it is still. Tracked in docs/status.md. |
| `2026-10-19-wireframe-overlay-lod-levels.png` | The first wireframe overlay (VIEW-014, #354, F3 in the editor) over the lit terrain: one colour band per level of detail (cyan nearest, then green, yellow), the bright squares are tile outlines (each tile a 32 x 32 cell grid whose inner lines fade where cells are under about a pixel). Owner, RTX. |
| `2026-10-19-bug-atmosphere-colour-jump-descent.png` | Bug report (owner, RTX, editor descent video 0:26-0:28): three frames at 25.5, 27 and 28.5 s; in the middle frame the aerial-perspective haze over land and sea turns green and the sky band changes, then returns. Tracked in docs/status.md. |
| `2026-10-19-bug-sea-ends-at-tile-edge.png` | Bug report (owner, RTX, editor descent video 0:53): low over a coast, the sea surface ends at a straight diagonal edge across open water; beyond it the dark seabed shows. Visible from 0:35 to the end. Tracked in docs/status.md. |
| `2026-10-23-bug-sun-glint-still.png` | Bug report (owner, RTX, 2026-10-23): low over a coast with the sun ahead, camera still: the sun glint on the sea is a soft band of streaks. Tracked in docs/status.md. |
| `2026-10-23-bug-sun-glint-blocky-while-turning.png` | The same view while the camera turns (pitch and yaw only, no movement): the glint breaks into a blocky grid of rectangles; it returns to the still image almost at once when the turn stops. Tracked in docs/status.md. |
| `2026-10-25-gorge-shadows-landing.png` | Horizon shadows landing in the editor (owner, RTX, 16384 store over the shared store, M8-5 and the A-net anchor fix #655): at 1.3 km altitude near the day/night line, a gorge casts its shadow across the plain at the bottom left; at the top the shadow stops at a straight line where the chunk horizon maps had not landed yet. The bright straight line below is a channel wall running dead straight for kilometres. The editor recomputed the maps after a stamped hill for minutes, with coder test runs in parallel (8 to 12 fps, CPU 79 ms a frame). |
| `2026-10-25-hill-shadow-over-gorges.png` | The same place after the maps landed (owner, RTX): the stamped "Hill 1" casts a long shadow at low sun across eroded ground with gorges; the earlier gap is filled. A first progress view of M8's chunk erosion with horizon shadows. Editor at 8 fps, CPU 120 ms, GPU 13 ms a frame (tracked in docs/status.md). |
