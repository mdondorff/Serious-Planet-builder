//! Self-contained single-file HTML renderer for `cargo xtask cockpit` (`BUILD-064`).
//! Zero external network assets, zero CDN fonts/scripts, safe offline `file://` image paths.

/// Renders the standalone HTML5 cockpit embedding `safe_bundle_json` inside an `application/json` script block.
pub fn render_html(page_title: &str, safe_bundle_json: &str) -> String {
    let escaped_title = escape_html_text(page_title);
    let template = include_str_inline();
    template.replace("__TITLE__", &escaped_title).replace("__BUNDLE_JSON__", safe_bundle_json)
}

fn escape_html_text(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

fn include_str_inline() -> &'static str {
    r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>__TITLE__</title>
<style>
:root {
  --bg: #0c1016;
  --panel: #131923;
  --panel-2: #192130;
  --panel-3: #202a3c;
  --border: #2a364c;
  --text: #e6edf5;
  --muted: #8ea0b8;
  --accent: #52a8ff;
  --accent-dim: rgba(82, 168, 255, 0.14);
  --ok: #3ad29f;
  --ok-dim: rgba(58, 210, 159, 0.15);
  --warn: #f5b942;
  --warn-dim: rgba(245, 185, 66, 0.15);
  --bad: #ff6b6b;
  --bad-dim: rgba(255, 107, 107, 0.16);
  --purple: #b088ff;
  --mono: ui-monospace, "Cascadia Mono", "SFMono-Regular", Menlo, Consolas, monospace;
  --sans: system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;
}
* { box-sizing: border-box; }
html, body { margin: 0; padding: 0; background: var(--bg); color: var(--text); font-family: var(--sans); font-size: 13px; line-height: 1.45; }
button, select, input { font: inherit; color: inherit; }
code, pre, .mono { font-family: var(--mono); }
.topbar {
  position: sticky; top: 0; z-index: 20;
  background: rgba(12, 16, 22, 0.96); border-bottom: 1px solid var(--border);
  padding: 10px 16px; display: flex; flex-wrap: wrap; align-items: center; gap: 12px; justify-content: space-between;
}
.brand { display: flex; flex-direction: column; gap: 2px; }
.brand h1 { margin: 0; font-size: 16px; font-weight: 650; letter-spacing: 0.01em; }
.brand .sub { font-size: 11px; color: var(--muted); }
.chips { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
.chip {
  display: inline-flex; align-items: center; gap: 5px;
  padding: 2px 8px; border-radius: 999px; border: 1px solid var(--border);
  background: var(--panel-2); font-size: 11px; color: var(--text);
}
.chip.ok { border-color: rgba(58, 210, 159, 0.45); background: var(--ok-dim); color: #9ff3d5; }
.chip.accent { border-color: rgba(82, 168, 255, 0.45); background: var(--accent-dim); color: #a8d4ff; }
.chip.warn { border-color: rgba(245, 185, 66, 0.45); background: var(--warn-dim); color: #ffd98a; }
.chip.bad { border-color: rgba(255, 107, 107, 0.45); background: var(--bad-dim); color: #ffb3b3; }
.tabs { display: flex; gap: 6px; }
.tab-btn {
  padding: 6px 12px; border-radius: 6px; border: 1px solid var(--border);
  background: var(--panel); color: var(--muted); cursor: pointer; font-weight: 550;
}
.tab-btn:hover { border-color: var(--accent); color: var(--text); }
.tab-btn.active { background: var(--accent-dim); border-color: var(--accent); color: #fff; }
.main { padding: 14px 16px 32px; max-width: 1720px; margin: 0 auto; }
.tab-panel { display: none; }
.tab-panel.active { display: block; }
.controls-bar {
  display: flex; flex-wrap: wrap; gap: 10px; align-items: center;
  background: var(--panel); border: 1px solid var(--border); border-radius: 8px;
  padding: 10px 12px; margin-bottom: 12px;
}
.ctrl { display: inline-flex; align-items: center; gap: 6px; font-size: 12px; color: var(--muted); }
.ctrl select, .ctrl input[type="text"], .ctrl input[type="search"] {
  background: var(--panel-2); border: 1px solid var(--border); border-radius: 5px;
  padding: 4px 8px; color: var(--text); min-width: 120px;
}
.ctrl label { cursor: pointer; display: inline-flex; align-items: center; gap: 5px; color: var(--text); }
.btn {
  background: var(--panel-2); border: 1px solid var(--border); border-radius: 5px;
  padding: 4px 10px; cursor: pointer; color: var(--text); font-size: 12px;
}
.btn:hover { border-color: var(--accent); }
.btn.primary { background: var(--accent-dim); border-color: var(--accent); }
.grid-two { display: grid; grid-template-columns: minmax(420px, 1.35fr) minmax(360px, 1fr); gap: 14px; align-items: start; }
@media (max-width: 1080px) { .grid-two { grid-template-columns: 1fr; } }
.card {
  background: var(--panel); border: 1px solid var(--border); border-radius: 8px;
  padding: 12px; margin-bottom: 12px;
}
.card h2, .card h3 { margin: 0 0 8px; font-size: 14px; font-weight: 650; }
.lane { margin-bottom: 12px; }
.lane-title {
  font-size: 11px; text-transform: uppercase; letter-spacing: 0.06em; color: var(--muted);
  margin: 0 0 6px; display: flex; justify-content: space-between; align-items: center;
}
.stage-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(235px, 1fr)); gap: 8px; }
.stage-card {
  border: 1px solid var(--border); border-radius: 7px; padding: 9px 10px;
  background: var(--panel-2); cursor: pointer; transition: border-color 0.12s, background 0.12s;
  display: flex; flex-direction: column; gap: 5px;
}
.stage-card:hover { border-color: var(--accent); }
.stage-card.selected { border-color: var(--accent); box-shadow: 0 0 0 1px var(--accent) inset; }
.stage-card.active-stage { border-left: 3px solid var(--ok); }
.stage-card.inactive-stage { opacity: 0.52; }
.stage-head { display: flex; align-items: center; justify-content: space-between; gap: 6px; }
.stage-title { font-weight: 600; font-size: 12.5px; color: var(--text); }
.stage-id { font-family: var(--mono); font-size: 11px; color: var(--muted); }
.badges { display: flex; flex-wrap: wrap; gap: 4px; }
.badge {
  font-size: 10px; padding: 1px 6px; border-radius: 4px; border: 1px solid var(--border);
  background: var(--panel-3); color: var(--muted); font-family: var(--mono);
}
.badge.k { color: #b7dbff; border-color: rgba(82, 168, 255, 0.4); }
.badge.scope-global { color: #9ff3d5; }
.badge.scope-region { color: #ffd98a; }
.badge.scope-tile { color: #d5c1ff; }
.badge.probe { color: #ffd98a; border-color: rgba(245, 185, 66, 0.45); }
.badge.img { color: #9ff3d5; border-color: rgba(58, 210, 159, 0.45); }
.kv { display: grid; grid-template-columns: 128px 1fr; gap: 5px 10px; font-size: 12px; margin-bottom: 10px; }
.kv .k { color: var(--muted); }
.pill-list { display: flex; flex-wrap: wrap; gap: 4px; }
.link-pill {
  padding: 2px 7px; border-radius: 4px; border: 1px solid var(--border);
  background: var(--panel-2); color: var(--accent); cursor: pointer; font-family: var(--mono); font-size: 11px;
}
.link-pill:hover { border-color: var(--accent); background: var(--accent-dim); }
.chain-list { display: flex; flex-direction: column; gap: 6px; }
.chain-item {
  border: 1px solid var(--border); border-radius: 6px; background: var(--panel-2);
  padding: 7px 10px; display: flex; justify-content: space-between; align-items: center; gap: 8px;
}
.chain-arrow { text-align: center; color: var(--muted); font-size: 11px; line-height: 1; }
.code-box {
  background: #0a0e14; border: 1px solid var(--border); border-radius: 6px;
  padding: 8px 10px; font-family: var(--mono); font-size: 11.5px; overflow-x: auto; white-space: pre-wrap; word-break: break-word;
}
.inspector-layout { display: grid; grid-template-columns: 280px 1fr; gap: 14px; align-items: start; }
@media (max-width: 960px) { .inspector-layout { grid-template-columns: 1fr; } }
.side-list { display: flex; flex-direction: column; gap: 4px; }
.side-item {
  text-align: left; width: 100%; padding: 8px 10px; border-radius: 6px;
  border: 1px solid var(--border); background: var(--panel-2); cursor: pointer;
  display: flex; justify-content: space-between; align-items: center; gap: 6px;
}
.side-item:hover { border-color: var(--accent); }
.side-item.active { border-color: var(--accent); background: var(--accent-dim); }
.img-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(290px, 1fr)); gap: 10px; }
.img-card {
  border: 1px solid var(--border); border-radius: 7px; background: var(--panel-2); overflow: hidden;
  display: flex; flex-direction: column;
}
.img-card .thumb-wrap {
  background: #070a0f; display: flex; align-items: center; justify-content: center;
  min-height: 165px; max-height: 250px; overflow: hidden; cursor: zoom-in;
}
.img-card img { max-width: 100%; max-height: 250px; object-fit: contain; display: block; }
.img-meta { padding: 8px 10px; font-size: 11.5px; display: flex; flex-direction: column; gap: 3px; }
.compare-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
@media (max-width: 900px) { .compare-grid { grid-template-columns: 1fr; } }
.compare-pane { border: 1px solid var(--border); border-radius: 7px; background: var(--panel-2); padding: 10px; }
.compare-pane img { width: 100%; max-height: 460px; object-fit: contain; background: #070a0f; border-radius: 4px; cursor: zoom-in; }
table { width: 100%; border-collapse: collapse; font-size: 12px; }
th, td { text-align: left; padding: 6px 8px; border-bottom: 1px solid var(--border); vertical-align: top; }
th { color: var(--muted); font-weight: 600; position: sticky; top: 0; background: var(--panel); }
tr:hover td { background: rgba(255, 255, 255, 0.025); }
.muted { color: var(--muted); }
.banner {
  border: 1px solid var(--border); border-radius: 6px; padding: 9px 12px; margin-bottom: 10px; font-size: 12px;
}
.banner.warn { border-color: rgba(245, 185, 66, 0.45); background: var(--warn-dim); color: #ffe09e; }
.banner.bad { border-color: rgba(255, 107, 107, 0.45); background: var(--bad-dim); color: #ffb5b5; }
.banner.ok { border-color: rgba(58, 210, 159, 0.45); background: var(--ok-dim); color: #b7f7df; }
.modal {
  position: fixed; inset: 0; background: rgba(5, 8, 13, 0.9); z-index: 50;
  display: none; flex-direction: column; padding: 18px; gap: 10px;
}
.modal.open { display: flex; }
.modal-head { display: flex; justify-content: space-between; align-items: center; }
.modal-body { flex: 1; display: flex; align-items: center; justify-content: center; overflow: auto; }
.modal-body img { max-width: 100%; max-height: 100%; object-fit: contain; }
.modal-body img.native { max-width: none; max-height: none; }
</style>
</head>
<body>
<header class="topbar">
  <div class="brand">
    <h1>__TITLE__</h1>
    <div class="sub" id="subtitle">Offline Pipeline Interface &amp; Global Authoring Step Inspector (BUILD-063 / BUILD-064)</div>
  </div>
  <div class="chips" id="world-chips"></div>
  <nav class="tabs">
    <button class="tab-btn active" data-tab="pipeline">1. Pipeline DAG &amp; Switches</button>
    <button class="tab-btn" data-tab="inspector">2. Authoring &amp; Step Inspector</button>
    <button class="tab-btn" data-tab="pins">3. Version Pins &amp; Probe Ledger</button>
    <button class="btn" id="btn-cli-help" title="Show live cargo xtask cockpit CLI usage &amp; examples">CLI Help (--help)</button>
  </nav>
</header>

<main class="main">
  <section class="tab-panel active" id="tab-pipeline">
    <div class="controls-bar">
      <button class="btn primary" id="btn-use-resolved">Use Resolved Target</button>
      <div class="ctrl"><span>height_field</span><select id="sw-hf"></select></div>
      <div class="ctrl"><span>erosion</span>
        <select id="sw-erosion">
          <option value="none">none (off)</option>
          <option value="m9-layer">m9-layer (--erosion on)</option>
          <option value="k5-k7-commit">k5-k7-commit (--commit DIR)</option>
        </select>
      </div>
      <div class="ctrl"><label><input type="checkbox" id="sw-relief"> relief_lattice</label></div>
      <div class="ctrl"><label><input type="checkbox" id="sw-channels"> channels</label></div>
      <div class="ctrl"><label><input type="checkbox" id="sw-stamps"> ADR 0007 stamps</label></div>
      <div class="ctrl"><label><input type="checkbox" id="sw-water"> WaterBounded cull</label></div>
      <div class="ctrl"><span>scope</span>
        <select id="filter-scope">
          <option value="all">all</option>
          <option value="global">global</option>
          <option value="region">region</option>
          <option value="tile">tile</option>
        </select>
      </div>
      <div class="ctrl"><input type="search" id="filter-stage" placeholder="Filter stages, IDs, specs..."></div>
    </div>

    <div class="grid-two">
      <div id="dag-lanes"></div>
      <div>
        <div class="card" id="chain-panel"></div>
        <div class="card" id="stage-detail-panel"></div>
      </div>
    </div>
  </section>

  <section class="tab-panel" id="tab-inspector">
    <div class="controls-bar">
      <div class="ctrl"><span>view</span>
        <select id="insp-mode">
          <option value="gallery">Gallery + Reports</option>
          <option value="compare">Side-by-Side Compare</option>
        </select>
      </div>
      <div class="ctrl"><span>variant filter</span>
        <select id="insp-variant"><option value="all">all variants</option></select>
      </div>
      <div class="ctrl" style="margin-left:auto;">
        <span class="muted" id="insp-summary"></span>
      </div>
    </div>
    <div class="inspector-layout">
      <div class="card">
        <h3>Preview &amp; Stage Steps</h3>
        <div class="side-list" id="insp-stage-list"></div>
      </div>
      <div id="insp-main"></div>
    </div>
  </section>

  <section class="tab-panel" id="tab-pins">
    <div class="card">
      <div style="display:flex; justify-content:space-between; align-items:center; gap:10px; margin-bottom:8px;">
        <h2 style="margin:0;">Version &amp; Default Constant Pins</h2>
        <div class="ctrl"><input type="search" id="filter-pins" placeholder="Filter pins, const path, spec ID..."></div>
      </div>
      <div style="overflow-x:auto; max-height:460px;">
        <table>
          <thead><tr><th>Key</th><th>Value</th><th>Rust Constant</th><th>Used by Stages</th><th>Spec IDs</th></tr></thead>
          <tbody id="pins-tbody"></tbody>
        </table>
      </div>
    </div>

    <div class="card">
      <div style="display:flex; justify-content:space-between; align-items:center; gap:10px; margin-bottom:8px;">
        <h2 style="margin:0;">Committed Stage Manifest Probes (<span class="mono">docs/tools/stages/manifest.txt</span>)</h2>
        <span class="muted" id="manifest-count"></span>
      </div>
      <div style="overflow-x:auto;">
        <table>
          <thead><tr><th>Probe Stage</th><th>Field</th><th>Catalog Stage</th><th>Cells</th><th>Min</th><th>p5</th><th>p50</th><th>p95</th><th>Max</th><th>Hash</th><th>Extras</th></tr></thead>
          <tbody id="manifest-tbody"></tbody>
        </table>
      </div>
    </div>
  </section>
</main>

<div class="modal" id="lightbox">
  <div class="modal-head">
    <div class="mono" id="lightbox-title"></div>
    <div style="display:flex; gap:8px;">
      <button class="btn" id="lightbox-zoom">Toggle 1:1 Pixels</button>
      <button class="btn" id="lightbox-close">Close (Esc)</button>
    </div>
  </div>
  <div class="modal-body"><img id="lightbox-img" alt="Preview zoom"></div>
</div>

<div class="modal" id="help-modal">
  <div class="modal-head">
    <div class="mono">Living CLI Reference (<span class="mono">cargo xtask cockpit --help</span>)</div>
    <div style="display:flex; gap:8px;">
      <button class="btn" id="help-copy">Copy Help</button>
      <button class="btn" id="help-close">Close (Esc)</button>
    </div>
  </div>
  <div class="modal-body" style="align-items:flex-start;">
    <pre class="code-box" id="help-modal-body" style="max-width:980px; width:100%; max-height:80vh; overflow:auto;"></pre>
  </div>
</div>

<script id="cockpit-bundle" type="application/json">__BUNDLE_JSON__</script>
<script>
(function () {
  "use strict";
  const bundle = JSON.parse(document.getElementById("cockpit-bundle").textContent);
  const catalog = bundle.catalog || {};
  const stages = catalog.stages || [];
  const pins = catalog.version_pins || [];
  const matrix = catalog.switch_matrix || [];
  const artifacts = bundle.artifacts || { images: [], texts: [] };
  const manifestRows = bundle.manifest_rows || [];
  const world = bundle.world || null;

  const pinByKey = new Map(pins.map((p) => [p.key, p]));
  const stageById = new Map(stages.map((s) => [s.id, s]));

  function esc(v) {
    return String(v == null ? "" : v)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#39;");
  }

  function fmtBytes(n) {
    if (n < 1024) return n + " B";
    if (n < 1024 * 1024) return (n / 1024).toFixed(1) + " KiB";
    return (n / (1024 * 1024)).toFixed(2) + " MiB";
  }

  function copyText(text, btn) {
    const prev = btn ? btn.textContent : "";
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(text).then(() => {
        if (btn) { btn.textContent = "Copied"; setTimeout(() => { btn.textContent = prev; }, 1200); }
      }).catch(() => {});
    }
  }

  // Header chips
  function renderHeader() {
    const chips = [];
    chips.push('<span class="chip accent">schema v' + esc(catalog.schema_format_version) + '</span>');
    chips.push('<span class="chip">' + stages.length + ' stages / ' + pins.length + ' pins</span>');
    chips.push('<span class="chip ' + (artifacts.images.length ? 'ok' : '') + '">' + artifacts.images.length + ' PNGs / ' + artifacts.texts.length + ' reports</span>');
    if (world) {
      chips.push('<span class="chip ok">seed ' + esc(world.seed) + '</span>');
      chips.push('<span class="chip mono" title="WorldDefinition base_node_hash">base ' + esc(world.base_node_hash) + '</span>');
      chips.push('<span class="chip mono" title="WorldDefinition definition_hash">def ' + esc(world.definition_hash) + '</span>');
      chips.push('<span class="chip accent">' + esc(world.height_field) + ' (' + esc(world.erosion_mode) + ')</span>');
    } else {
      chips.push('<span class="chip warn">catalog-only mode</span>');
    }
    document.getElementById("world-chips").innerHTML = chips.join("");
  }

  // Tab switching
  const tabButtons = document.querySelectorAll(".tab-btn");
  function openTab(name) {
    tabButtons.forEach((b) => b.classList.toggle("active", b.dataset.tab === name));
    document.querySelectorAll(".tab-panel").forEach((p) => p.classList.toggle("active", p.id === "tab-" + name));
  }
  tabButtons.forEach((b) => b.addEventListener("click", () => openTab(b.dataset.tab)));

  // Pipeline switches & simulation
  const swHf = document.getElementById("sw-hf");
  const swErosion = document.getElementById("sw-erosion");
  const swRelief = document.getElementById("sw-relief");
  const swChannels = document.getElementById("sw-channels");
  const swStamps = document.getElementById("sw-stamps");
  const swWater = document.getElementById("sw-water");
  const filterScope = document.getElementById("filter-scope");
  const filterStage = document.getElementById("filter-stage");
  const btnResolved = document.getElementById("btn-use-resolved");

  (catalog.known_height_fields || []).forEach((id) => {
    const opt = document.createElement("option");
    opt.value = id;
    opt.textContent = id;
    swHf.appendChild(opt);
  });

  let useResolvedExact = Boolean(world);
  let selectedStageId = stages.length ? stages[0].id : null;

  function applyWorldToControls() {
    if (world) {
      swHf.value = world.height_field || catalog.default_height_field;
      swErosion.value = world.erosion_mode || "none";
      swRelief.checked = Boolean(world.relief_lattice);
      swChannels.checked = Boolean(world.channels);
      swStamps.checked = Boolean(world.stamp_layers && world.stamp_layers.length);
      swWater.checked = bundle.walked_chain ? bundle.walked_chain.water_cull_level_m != null : false;
      useResolvedExact = true;
    } else {
      swHf.value = catalog.default_height_field || "cpu-erosion-v1";
      swErosion.value = "none";
      swRelief.checked = false;
      swChannels.checked = false;
      swStamps.checked = false;
      swWater.checked = false;
      useResolvedExact = false;
    }
    btnResolved.style.display = world ? "" : "none";
  }

  function onControlChange() {
    useResolvedExact = false;
    if (swErosion.value === "k5-k7-commit") {
      swHf.value = "cpu-erosion-c1-v2";
    }
    renderPipeline();
  }

  [swHf, swErosion, swRelief, swChannels, swStamps, swWater].forEach((el) => {
    el.addEventListener("change", onControlChange);
  });
  filterScope.addEventListener("change", renderPipeline);
  filterStage.addEventListener("input", renderPipeline);
  btnResolved.addEventListener("click", () => {
    applyWorldToControls();
    renderPipeline();
  });

  function computeActivePrediction() {
    if (useResolvedExact && world && bundle.walked_chain) {
      return {
        exact: true,
        activeSet: new Set(bundle.active_stages || []),
        chain: bundle.walked_chain,
        error: null,
      };
    }
    const hf = swHf.value;
    const mode = swErosion.value;
    const erosionOn = mode !== "none";
    const relief = mode === "m9-layer" ? swRelief.checked : false;
    const channels = mode === "m9-layer" ? swChannels.checked : false;
    const needsBake = hf !== "cpu-ref-height-v2";

    if (mode === "k5-k7-commit" && hf !== "cpu-erosion-c1-v2") {
      return { exact: false, activeSet: new Set(), chain: null, error: "Commit mode requires cpu-erosion-c1-v2." };
    }
    const row = matrix.find((r) =>
      r.height_field === hf &&
      r.erosion_layer === erosionOn &&
      r.relief_lattice === relief &&
      r.channels === channels
    );
    if (!row) {
      return {
        exact: false,
        activeSet: new Set(["world-definition"]),
        chain: null,
        error: "Combination (" + hf + ", " + mode + ", relief=" + relief + ", channels=" + channels + ") is refused by field_chain_erosion.",
      };
    }

    const active = ["world-definition"];
    if (needsBake) {
      active.push(
        "continents-and-plates", "shelf-bathymetry", "lithology", "tectonic-uplift",
        "orographic-climate", "arid-class-rules", "ice-history", "global-bake-lattice", "coupled-h0"
      );
    }
    if (mode === "m9-layer") active.push("m9-pass1-face-carve", "m9-pass2-tile-layer");
    if (mode === "k5-k7-commit") {
      active.push("k5-landmass-and-window-cut", "k5-regional-rounds", "k6-h1-hydrology-and-lakes", "k6-regional-sediment", "k7-local-refinement");
    }
    active.push("runtime-base-field");
    if (swStamps.checked) active.push("authored-stamp-stages");
    active.push("water-cull-and-tile-plan");

    const links = [];
    if (swWater.checked) {
      const wPin = pinByKey.get("water_surface");
      links.push({ implementation_id: "water-bounded-v1", version: wPin ? wPin.value : 1, has_source_key: true, stage_layer: null });
    }
    if (swStamps.checked) {
      const sPin = pinByKey.get("stamp_stage");
      links.push({
        implementation_id: "cpu-stamp-stage-v1",
        version: sPin ? sPin.value : 1,
        has_source_key: false,
        stage_layer: { name: (catalog.layer_stack && catalog.layer_stack[0]) || "base", stack_index: 0 },
      });
    }
    links.push({
      implementation_id: row.implementation_id,
      version: row.version,
      has_source_key: true,
      stage_layer: null,
    });

    return {
      exact: false,
      activeSet: new Set(active),
      chain: {
        turned_noise: Boolean(catalog.turned_noise_default),
        has_driver_channels: mode === "k5-k7-commit" ? false : channels,
        water_cull_level_m: swWater.checked ? (world ? world.sea_level_m : 0.0) : null,
        links_top_down: links,
      },
      error: null,
    };
  }

  function laneOfStage(s) {
    const k = s.k_label || "";
    if (k === "K1" || k === "K2") return "K1-K2 Global Inputs & Authoring Drivers";
    if (k === "K3" || k === "K4") return "K3-K4 Global-Coarse Bake & Coupled H0";
    if (k.startsWith("M9")) return "M9 Erosion Layer (Legacy/On-the-Fly --erosion on)";
    if (k === "K5" || k === "K6" || k === "K7") return "K5-K7 Commit Erosion Chain (--commit DIR)";
    return "K8 / K11 Runtime Field Stack & Tile Plan";
  }

  function imagesForStage(stageId) {
    return artifacts.images.filter((im) => im.stage_id === stageId);
  }
  function textsForStage(stageId) {
    return artifacts.texts.filter((tx) => tx.stage_id === stageId);
  }
  function probesForStage(stageId) {
    return manifestRows.filter((r) => r.stage_id === stageId);
  }

  function renderPipeline() {
    const pred = computeActivePrediction();
    const scopeVal = filterScope.value;
    const q = filterStage.value.trim().toLowerCase();

    const groups = new Map();
    stages.forEach((s) => {
      if (scopeVal !== "all" && s.scope !== scopeVal) return;
      if (q) {
        const hay = [s.id, s.title, s.k_label, s.summary, (s.spec_ids || []).join(" "), (s.version_pins || []).join(" ")].join(" ").toLowerCase();
        if (!hay.includes(q)) return;
      }
      const lane = laneOfStage(s);
      if (!groups.has(lane)) groups.set(lane, []);
      groups.get(lane).push(s);
    });

    const lanesEl = document.getElementById("dag-lanes");
    let html = "";
    groups.forEach((list, laneName) => {
      html += '<div class="card lane"><div class="lane-title"><span>' + esc(laneName) + '</span><span>' + list.length + ' stages</span></div><div class="stage-grid">';
      list.forEach((s) => {
        const isActive = pred.activeSet.has(s.id);
        const isSel = selectedStageId === s.id;
        const imgs = imagesForStage(s.id);
        const probes = probesForStage(s.id);
        const cls = ["stage-card", isActive ? "active-stage" : "inactive-stage", isSel ? "selected" : ""].join(" ");
        html += '<div class="' + cls + '" data-stage-id="' + esc(s.id) + '">';
        html += '<div class="stage-head"><span class="stage-title">' + esc(s.title) + '</span></div>';
        html += '<div class="stage-id">' + esc(s.id) + '</div>';
        html += '<div class="badges">';
        html += '<span class="badge k">' + esc(s.k_label) + '</span>';
        html += '<span class="badge scope-' + esc(s.scope) + '">' + esc(s.scope) + '</span>';
        if (s.edit_preview_level) html += '<span class="badge">' + esc(s.edit_preview_level) + '</span>';
        if (probes.length) html += '<span class="badge probe">' + probes.length + ' probes</span>';
        if (imgs.length) html += '<span class="badge img">' + imgs.length + ' PNG</span>';
        html += '</div></div>';
      });
      html += '</div></div>';
    });
    if (!html) html = '<div class="banner warn">No stages match the active filters.</div>';
    lanesEl.innerHTML = html;

    lanesEl.querySelectorAll(".stage-card").forEach((el) => {
      el.addEventListener("click", () => {
        selectedStageId = el.dataset.stageId;
        renderPipeline();
      });
    });

    renderChainPanel(pred);
    renderStageDetail();
  }

  function renderChainPanel(pred) {
    const panel = document.getElementById("chain-panel");
    let h = '<h2>' + (pred.exact ? 'Walked Runtime Stack (<span class="mono">HeightField::upstream()</span>)' : 'Predicted Switch Stack') + '</h2>';
    if (pred.error) {
      h += '<div class="banner bad">' + esc(pred.error) + '</div>';
      panel.innerHTML = h;
      return;
    }
    const c = pred.chain;
    h += '<div class="badges" style="margin-bottom:8px;">';
    h += '<span class="badge ' + (pred.exact ? 'img' : 'k') + '">' + (pred.exact ? 'walked live HeightField' : 'simulated from switch_matrix') + '</span>';
    h += '<span class="badge">turned_noise: ' + esc(c.turned_noise) + '</span>';
    h += '<span class="badge">driver_channels: ' + esc(c.has_driver_channels) + '</span>';
    h += '<span class="badge">water_cull_m: ' + esc(c.water_cull_level_m == null ? "off" : c.water_cull_level_m) + '</span>';
    h += '</div>';
    h += '<div class="chain-list">';
    (c.links_top_down || []).forEach((lk, i) => {
      if (i > 0) h += '<div class="chain-arrow">↓ upstream()</div>';
      h += '<div class="chain-item">';
      h += '<div><div class="mono" style="font-weight:600;">' + esc(lk.implementation_id) + '</div>';
      if (lk.stage_layer) {
        h += '<div class="muted" style="font-size:11px;">ADR 0007 layer: <span class="mono">' + esc(lk.stage_layer.name) + '</span> (idx ' + esc(lk.stage_layer.stack_index) + ')</div>';
      }
      h += '</div>';
      h += '<div class="badges"><span class="badge k">v' + esc(lk.version) + '</span><span class="badge">source_key: ' + esc(lk.has_source_key) + '</span></div>';
      h += '</div>';
    });
    h += '</div>';
    panel.innerHTML = h;
  }

  function renderStageDetail() {
    const panel = document.getElementById("stage-detail-panel");
    const s = stageById.get(selectedStageId) || stages[0];
    if (!s) { panel.innerHTML = '<div class="muted">No stage selected.</div>'; return; }

    let h = '<div style="display:flex; justify-content:space-between; align-items:start; gap:8px;">';
    h += '<div><h2 style="margin-bottom:2px;">' + esc(s.title) + '</h2><div class="mono muted" style="font-size:11px; margin-bottom:8px;">' + esc(s.id) + '</div></div>';
    h += '<div class="badges"><span class="badge k">' + esc(s.k_label) + '</span><span class="badge scope-' + esc(s.scope) + '">' + esc(s.scope) + '</span></div></div>';
    h += '<p style="margin:0 0 10px;">' + esc(s.summary) + '</p>';
    h += '<div class="kv">';
    h += '<div class="k">active_when</div><div class="mono">' + esc(s.active_when) + '</div>';
    h += '<div class="k">edit_preview</div><div class="mono">' + esc(s.edit_preview_level || "none") + '</div>';
    h += '<div class="k">reads</div><div class="pill-list">' + ((s.reads && s.reads.length) ? s.reads.map((r) => '<button class="link-pill" data-jump-stage="' + esc(r) + '">' + esc(r) + '</button>').join("") : '<span class="muted">none</span>') + '</div>';
    h += '<div class="k">outputs</div><div>' + (s.outputs || []).map((o) => '<div>• ' + esc(o) + '</div>').join("") + '</div>';
    h += '<div class="k">spec_ids</div><div class="pill-list">' + (s.spec_ids || []).map((id) => '<span class="badge">' + esc(id) + '</span>').join("") + '</div>';
    h += '</div>';

    if (s.version_pins && s.version_pins.length) {
      h += '<h3>Bound Version Pins</h3><div class="kv">';
      s.version_pins.forEach((pk) => {
        const pin = pinByKey.get(pk);
        h += '<div class="k mono">' + esc(pk) + '</div>';
        h += '<div class="mono">' + (pin ? ('v' + esc(pin.value) + ' — <span class="muted">' + esc(pin.const_path) + '</span>') : 'missing') + '</div>';
      });
      h += '</div>';
    }

    if (s.probe_hooks && s.probe_hooks.length) {
      const matched = probesForStage(s.id);
      h += '<h3>Probe Hooks (<span class="mono">docs/tools/stages/manifest.txt</span>)</h3>';
      h += '<div class="badges" style="margin-bottom:6px;">' + s.probe_hooks.map((hk) => '<span class="badge probe">' + esc(hk.kind_id) + ': ' + esc((hk.fields || []).join(", ")) + '</span>').join("") + '</div>';
      if (matched.length) {
        h += '<div class="code-box">' + matched.map((r) => esc(r.stage + ' | ' + r.field + ' | cells ' + r.cells + ' | min ' + r.min + ' | p50 ' + r.p50 + ' | max ' + r.max + ' | hash ' + r.hash)).join("\n") + '</div>';
      }
    }

    if (s.preview) {
      const cmdSeed = world ? world.seed : 14;
      const cmd = (s.preview.command_template || "")
        .replace("{seed}", String(cmdSeed))
        .replace("{out_dir}", "target/review/cockpit/previews/seed-" + cmdSeed + "/" + s.id);
      h += '<h3>Preview Contract</h3>';
      h += '<div class="code-box" style="margin-bottom:6px;">' + esc(cmd) + '</div>';
      h += '<div style="display:flex; gap:6px; flex-wrap:wrap; margin-bottom:6px;">';
      h += '<button class="btn" id="btn-copy-cmd">Copy Command</button>';
      h += '<button class="btn primary" id="btn-open-insp">Inspect Stage Outputs (' + imagesForStage(s.id).length + ' PNG)</button>';
      h += '</div>';
      h += '<div class="muted" style="font-size:11px;">Declared files: <span class="mono">' + esc((s.preview.files || []).join(", ")) + '</span></div>';
    }

    panel.innerHTML = h;
    panel.querySelectorAll("[data-jump-stage]").forEach((b) => {
      b.addEventListener("click", () => {
        selectedStageId = b.dataset.jumpStage;
        renderPipeline();
      });
    });
    const copyBtn = document.getElementById("btn-copy-cmd");
    if (copyBtn && s.preview) {
      const cmdSeed = world ? world.seed : 14;
      const cmd = (s.preview.command_template || "")
        .replace("{seed}", String(cmdSeed))
        .replace("{out_dir}", "target/review/cockpit/previews/seed-" + cmdSeed + "/" + s.id);
      copyBtn.addEventListener("click", () => copyText(cmd, copyBtn));
    }
    const openInsp = document.getElementById("btn-open-insp");
    if (openInsp) {
      openInsp.addEventListener("click", () => {
        inspStageId = s.id;
        openTab("inspector");
        renderInspector();
      });
    }
  }

  // Tab 2: Authoring & Step Inspector
  const inspectorStages = stages.filter((s) => s.preview || imagesForStage(s.id).length || textsForStage(s.id).length);
  let inspStageId = (inspectorStages.find((s) => imagesForStage(s.id).length > 0) || inspectorStages[0] || {}).id || null;
  const inspMode = document.getElementById("insp-mode");
  const inspVariant = document.getElementById("insp-variant");

  const variantSet = new Set();
  artifacts.images.forEach((im) => { if (im.variant) variantSet.add(im.variant); });
  artifacts.texts.forEach((tx) => { if (tx.variant) variantSet.add(tx.variant); });
  Array.from(variantSet).sort().forEach((v) => {
    const opt = document.createElement("option");
    opt.value = v;
    opt.textContent = v;
    inspVariant.appendChild(opt);
  });

  inspMode.addEventListener("change", renderInspector);
  inspVariant.addEventListener("change", renderInspector);

  function renderInspector() {
    document.getElementById("insp-summary").textContent =
      artifacts.images.length + " PNGs and " + artifacts.texts.length + " reports indexed";

    const listEl = document.getElementById("insp-stage-list");
    listEl.innerHTML = inspectorStages.map((s) => {
      const cnt = imagesForStage(s.id).length;
      const active = s.id === inspStageId ? " active" : "";
      return '<button class="side-item' + active + '" data-insp-stage="' + esc(s.id) + '">' +
        '<span><span class="badge k">' + esc(s.k_label) + '</span> ' + esc(s.title) + '</span>' +
        '<span class="badge ' + (cnt ? 'img' : '') + '">' + cnt + '</span></button>';
    }).join("");

    listEl.querySelectorAll("[data-insp-stage]").forEach((b) => {
      b.addEventListener("click", () => {
        inspStageId = b.dataset.inspStage;
        renderInspector();
      });
    });

    const mainEl = document.getElementById("insp-main");
    const s = stageById.get(inspStageId);
    if (!s) {
      mainEl.innerHTML = '<div class="card">No preview stages available.</div>';
      return;
    }
    const vFilter = inspVariant.value;
    const imgs = imagesForStage(s.id).filter((im) => vFilter === "all" || im.variant === vFilter);
    const texts = textsForStage(s.id).filter((tx) => vFilter === "all" || tx.variant === vFilter);
    const allStageImgs = imagesForStage(s.id);

    let h = '<div class="card"><div style="display:flex; justify-content:space-between; align-items:center; gap:8px; flex-wrap:wrap;">';
    h += '<div><h2 style="margin:0;">' + esc(s.title) + '</h2><div class="muted" style="font-size:12px;">' + esc(s.summary) + '</div></div>';
    if (s.preview) h += '<span class="badge mono">' + esc(s.preview.command_template) + '</span>';
    h += '</div></div>';

    if (!allStageImgs.length && !texts.length) {
      const seedHint = world ? world.seed : 14;
      const slugHint = s.id.includes("world") || s.id === "continents-and-plates" ? "world"
        : s.id.includes("k7") ? "stages" : "drivers";
      const runCmd = "cargo xtask cockpit --seed " + seedHint + " --run " + slugHint;
      h += '<div class="banner warn">No preview PNGs or text reports found on disk for <span class="mono">' + esc(s.id) + '</span> yet.<br>' +
        'Run <code class="mono">' + esc(runCmd) + '</code> (or <code class="mono">--run all</code>) to generate and inspect them here.</div>';
      mainEl.innerHTML = h;
      return;
    }

    if (inspMode.value === "compare" && allStageImgs.length > 0) {
      let variants = Array.from(new Set(allStageImgs.map((i) => i.variant || "default")));
      const leftIdx = 0;
      let rightIdx = Math.min(1, allStageImgs.length - 1);
      if (variants.length >= 2) {
        const firstName = allStageImgs[0].file_name;
        const exactMatch = allStageImgs.findIndex((i) => (i.variant || "default") === variants[1] && i.file_name === firstName);
        const secondVarIdx = exactMatch >= 0 ? exactMatch : allStageImgs.findIndex((i) => (i.variant || "default") === variants[1]);
        if (secondVarIdx >= 0) rightIdx = secondVarIdx;
      }
      const makeOpts = (selectedIdx) => allStageImgs.map((im, idx) =>
        '<option value="' + idx + '"' + (idx === selectedIdx ? ' selected' : '') + '>' + esc((im.variant ? im.variant + " / " : "") + im.file_name) + '</option>'
      ).join("");
      h += '<div class="card"><h3>Side-by-Side Compare</h3><div class="compare-grid">';
      h += '<div class="compare-pane"><div class="ctrl" style="margin-bottom:8px;"><span>Left</span><select id="cmp-left">' + makeOpts(leftIdx) + '</select></div><div id="cmp-left-box"></div></div>';
      h += '<div class="compare-pane"><div class="ctrl" style="margin-bottom:8px;"><span>Right</span><select id="cmp-right">' + makeOpts(rightIdx) + '</select></div><div id="cmp-right-box"></div></div>';
      h += '</div></div>';
    } else if (imgs.length) {
      h += '<div class="card"><h3>Rendered PNGs (' + imgs.length + ')</h3><div class="img-grid">';
      imgs.forEach((im) => {
        h += '<div class="img-card">';
        h += '<div class="thumb-wrap" data-zoom-src="' + esc(im.rel_path) + '" data-zoom-title="' + esc(im.rel_path + " (" + im.width + "x" + im.height + ")") + '">';
        h += '<img src="' + esc(im.rel_path) + '" alt="' + esc(im.file_name) + '" loading="lazy">';
        h += '</div><div class="img-meta">';
        h += '<div class="mono" style="font-weight:600;">' + esc(im.file_name) + '</div>';
        h += '<div class="badges"><span class="badge accent">' + esc(im.variant || "default") + '</span>';
        if (im.width && im.height) h += '<span class="badge">' + im.width + 'x' + im.height + '</span>';
        h += '<span class="badge">' + esc(fmtBytes(im.bytes)) + '</span></div>';
        h += '<div class="muted mono" style="font-size:10.5px; word-break:break-all;">' + esc(im.rel_path) + '</div>';
        h += '</div></div>';
      });
      h += '</div></div>';
    }

    if (texts.length) {
      h += '<div class="card"><h3>Text Reports &amp; Metrics (' + texts.length + ')</h3>';
      texts.forEach((tx) => {
        h += '<div style="margin-bottom:10px;"><div class="badges" style="margin-bottom:4px;"><span class="badge accent">' + esc(tx.variant || "default") + '</span><span class="badge mono">' + esc(tx.file_name) + '</span>';
        if (tx.truncated) h += '<span class="badge warn">truncated to 64 KiB</span>';
        h += '</div><pre class="code-box">' + esc(tx.content) + '</pre></div>';
      });
      h += '</div>';
    }

    mainEl.innerHTML = h;

    function bindZooms(scopeEl) {
      scopeEl.querySelectorAll("[data-zoom-src]").forEach((el) => {
        el.addEventListener("click", () => openLightbox(el.dataset.zoomSrc, el.dataset.zoomTitle));
      });
    }
    bindZooms(mainEl);

    const cmpLeft = document.getElementById("cmp-left");
    const cmpRight = document.getElementById("cmp-right");
    function paintComparePane(selEl, boxEl) {
      if (!selEl || !boxEl) return;
      const im = allStageImgs[Number(selEl.value)] || allStageImgs[0];
      if (!im) return;
      boxEl.innerHTML = '<img src="' + esc(im.rel_path) + '" data-zoom-src="' + esc(im.rel_path) + '" data-zoom-title="' + esc(im.rel_path) + '">' +
        '<div class="muted mono" style="font-size:11px; margin-top:6px;">' + esc(im.rel_path) + ' (' + im.width + 'x' + im.height + ', ' + fmtBytes(im.bytes) + ')</div>';
      bindZooms(boxEl);
    }
    if (cmpLeft && cmpRight) {
      const lb = document.getElementById("cmp-left-box");
      const rb = document.getElementById("cmp-right-box");
      paintComparePane(cmpLeft, lb);
      paintComparePane(cmpRight, rb);
      cmpLeft.addEventListener("change", () => paintComparePane(cmpLeft, lb));
      cmpRight.addEventListener("change", () => paintComparePane(cmpRight, rb));
    }
  }

  // Tab 3: Pins & Manifest
  const filterPins = document.getElementById("filter-pins");
  filterPins.addEventListener("input", renderPins);

  function renderPins() {
    const q = filterPins.value.trim().toLowerCase();
    const usedBy = new Map();
    stages.forEach((s) => {
      (s.version_pins || []).forEach((k) => {
        if (!usedBy.has(k)) usedBy.set(k, []);
        usedBy.get(k).push(s.id);
      });
    });

    const rows = pins.filter((p) => {
      if (!q) return true;
      return [p.key, p.const_path, (p.spec_ids || []).join(" "), (usedBy.get(p.key) || []).join(" ")].join(" ").toLowerCase().includes(q);
    });

    document.getElementById("pins-tbody").innerHTML = rows.map((p) =>
      '<tr><td class="mono">' + esc(p.key) + '</td>' +
      '<td class="mono"><span class="badge k">' + esc(p.value) + '</span></td>' +
      '<td class="mono">' + esc(p.const_path) + '</td>' +
      '<td class="mono muted">' + esc((usedBy.get(p.key) || []).join(", ")) + '</td>' +
      '<td>' + (p.spec_ids || []).map((id) => '<span class="badge">' + esc(id) + '</span>').join(" ") + '</td></tr>'
    ).join("");

    document.getElementById("manifest-count").textContent = manifestRows.length + " probe entries";
    document.getElementById("manifest-tbody").innerHTML = manifestRows.map((r) =>
      '<tr><td class="mono">' + esc(r.stage) + '</td>' +
      '<td class="mono">' + esc(r.field) + '</td>' +
      '<td class="mono">' + esc(r.stage_id || "-") + '</td>' +
      '<td class="mono">' + esc(r.cells) + '</td>' +
      '<td class="mono">' + esc(r.min) + '</td>' +
      '<td class="mono">' + esc(r.p5) + '</td>' +
      '<td class="mono">' + esc(r.p50) + '</td>' +
      '<td class="mono">' + esc(r.p95) + '</td>' +
      '<td class="mono">' + esc(r.max) + '</td>' +
      '<td class="mono">' + esc(r.hash) + '</td>' +
      '<td class="mono muted">' + esc(r.extras) + '</td></tr>'
    ).join("");
  }

  // Lightbox & CLI Help Modal
  const lightbox = document.getElementById("lightbox");
  const lightboxImg = document.getElementById("lightbox-img");
  const lightboxTitle = document.getElementById("lightbox-title");
  function openLightbox(src, title) {
    lightboxImg.classList.remove("native");
    lightboxImg.src = src;
    lightboxTitle.textContent = title || src;
    lightbox.classList.add("open");
  }
  document.getElementById("lightbox-close").addEventListener("click", () => lightbox.classList.remove("open"));
  document.getElementById("lightbox-zoom").addEventListener("click", () => lightboxImg.classList.toggle("native"));
  lightbox.addEventListener("click", (e) => { if (e.target === lightbox) lightbox.classList.remove("open"); });

  const helpModal = document.getElementById("help-modal");
  const helpBody = document.getElementById("help-modal-body");
  const helpTextStr = bundle.cli_help || "Run `cargo xtask cockpit --help` for full CLI usage.";
  helpBody.textContent = helpTextStr;
  document.getElementById("btn-cli-help").addEventListener("click", () => helpModal.classList.add("open"));
  document.getElementById("help-close").addEventListener("click", () => helpModal.classList.remove("open"));
  const helpCopyBtn = document.getElementById("help-copy");
  helpCopyBtn.addEventListener("click", () => copyText(helpTextStr, helpCopyBtn));
  helpModal.addEventListener("click", (e) => { if (e.target === helpModal) helpModal.classList.remove("open"); });

  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      lightbox.classList.remove("open");
      helpModal.classList.remove("open");
    }
  });

  renderHeader();
  applyWorldToControls();
  renderPipeline();
  renderInspector();
  renderPins();
})();
</script>
</body>
</html>
"#
}
