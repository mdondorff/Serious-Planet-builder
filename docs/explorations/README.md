# Explorations

Non-binding design explorations: mocks and sketches made to talk through requirements. They are not specs and decide nothing; the requirements they fed are in `docs/status.md` (or a spec or ADR once decided). One folder per exploration, `<date>-<desc>/`.

| Folder | What it is |
|---|---|
| `2026-10-18-editor-mock/` | The interactive editor MVP mock from the owner interview (2026-10-18): map-first layout, 3D and globe views, stamps, lines and regions, the inspector and the "Surface overlay" debug tint. Published as a private Design artifact (https://claude.ai/artifact/Srj3NGEYUkGQsfhne7kdCs); `Main.dc.html` is its source and needs the artifact's runtime (`support.js`), so it does not open standalone. Some data in it still names continental edits, which are out of the MVP. Later decisions supersede it: camera-driven relevance and handle-first stamp editing (status.md, 2026-10-19). |
