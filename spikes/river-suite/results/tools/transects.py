"""SPIKE-119: fixed river transects and cross-section figures (P3 E1).

pick  <window> : choose the window's 3 transects on today's surface (B + regional) and append them to transects.csv.
plot  <window> <out.png> : one figure: a locator map, then one cross-section per transect with every variant and a GLO-30 valley at matched relief, at one vertical scale.

Rule (deterministic, written once, the same for every window):
- River cells and drained area: main's routing (route_rules on the plain flood, epsilon 1 mm, seed 7) over the dump, from grid_gate's area file.
  A core dump (620 regional cells) routes without its margin, so a river entering from outside carries only its area inside the core: the csv says which.
- Candidates: river cells (area >= 75 km2, land) in the central 60 % of the dump, at least 6 km apart along the stream.
- T1 trunk: the candidate with the largest drained area. T2 mid: area within 0.5-2 x 1,000 km2. T3 small: area within 0.5-2 x 150 km2.
  For T2 and T3, of the candidates in the band, the one whose valley depth (ring 2-4 p50 minus the cell) is the band's median (a typical, not an extreme, valley).
- Line: through the river cell, normal to the stream's mean direction over 8 cells up and 8 down (steepest-area path); half-length 5 km (T1: 10 km on windows whose trunk
  drains over 100,000 km2), sampled bilinearly every 50.9 m (half an L cell).
- GLO-30 valley at matched relief: over the benchmark tiles of the same relief pool, the river cells of the same area band, each with its own transect by the same rule;
  the one whose relief along the line (max - min) is closest to the window transect's relief on today's surface (ties: the smaller tile name, then the cell index).
  T1: no tile drains a trunk's area, so its band is area >= min(half the target, 2,000 km2), the tiles' large rivers; the legend gives the matched river's area.
- Heights are plotted relative to the river cell of each profile (the window's today surface for all variants, so a variant that lowers the river shows it).
- One vertical scale per figure: every panel spans the same number of metres; the exaggeration is stated.
"""
import sys, os, csv, math
import numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt

ROOT = 'C:/DEV/Serious-Planet-Builder/.claude/worktrees/sonnet-spikes-36fc88/'
RES = ROOT + 'spikes/river-suite/results/'
CSV = RES + 'transects.csv'
BENCH = ROOT + 'target/river-suite/bench/'
CELL = 101.8
THR = 75e6

# name: (face, i0, i1, j0, j1, seed, dump dir, dump stem, dump kind, label, DEM pool)
WIN = {
    'r1h': (1, 8056, 8675, 760, 1379, 14, ROOT + 'target/river-review/', 'f1-8056-760-none', 'window', 'R1h', 'high'),
    'r3a': (4, 368, 987, 4720, 5339, 15, ROOT + 'target/river-review/', 'f4-368-4720-none', 'core', 'R3a', 'mid'),
    'r3b': (2, 9968, 10587, 8304, 8923, 15, ROOT + 'target/river-review/', 'f2-9968-8304-none', 'core', 'R3b', 'mid'),
    'm1': (4, 6600, 7219, 10200, 10819, 15, ROOT + 'target/river-suite/dumps/', 'f4-6600-10200-none', 'core', 'M1 (mountain)', 'high'),
    'f1': (4, 4800, 5419, 3300, 3919, 15, ROOT + 'target/river-suite/dumps/', 'f4-4800-3300-none', 'core', 'F1 (low gradient)', 'low'),
}
VAR = [('', 'today (B + regional)', '#444444'), ('-round', 'one round at L', '#1f77b4'), ('-td4', '4-neighbour drain', '#ff7f0e'),
       ('-td4-round', '4-neighbour drain + round', '#2ca02c'), ('-td4-schott', 'Schott 2024 fair', '#d62728')]
# Benchmark pools by slope p95 (SPIKE-108's frozen bands; the six tiles added by SPIKE-119 placed by their own slope p95, results.md).
POOL = {
    'high': ['N27E086', 'N40W078', 'N42W123', 'N45E014', 'N47E011', 'N48E008', 'N57W005', 'N61E006', 'N36W113', 'N46W122', 'S44E170', 'N36W117'],
    'mid': ['N31W004', 'N41W096', 'N43W102', 'S25E015'],
    'low': ['N35W076', 'N52E012', 'N70W154', 'N33W092', 'S04W061'],
}
BANDS = {'T2': 1000e6, 'T3': 150e6}


def dims(stem_dir, stem, kind):
    n = 8160 if kind == 'window' else 4960
    return n, n


def load(win, var):
    face, i0, i1, j0, j1, seed, d, stem, kind, label, pool = WIN[win]
    n = 8160 if kind == 'window' else 4960
    z = np.fromfile(f'{d}{stem}{var}-full-{n}x{n}.f32', '<f4').astype(np.float64).reshape(n, n)
    ox, oy = ((i0 - 200) * 8, (j0 - 200) * 8) if kind == 'window' else (i0 * 8, j0 * 8)
    return z, ox, oy


def area_of(win):
    face, i0, i1, j0, j1, seed, d, stem, kind, label, pool = WIN[win]
    n = 8160 if kind == 'window' else 4960
    return np.fromfile(f'{ROOT}target/river-suite/area/{win}-today.area.f32', '<f4').astype(np.float64).reshape(n, n)


def downstream_path(area, y, x, steps, up):
    """Follow the neighbour of largest area below (down) or the upstream neighbour of largest area (up)."""
    h, w = area.shape
    path = [(y, x)]
    for _ in range(steps):
        cy, cx = path[-1]
        best, ba = None, None
        for dy in (-1, 0, 1):
            for dx in (-1, 0, 1):
                if dy == 0 and dx == 0:
                    continue
                ny, nx = cy + dy, cx + dx
                if not (0 <= ny < h and 0 <= nx < w):
                    continue
                a = area[ny, nx]
                if up:
                    ok = a < area[cy, cx]
                    better = ba is None or a > ba
                else:
                    ok = a > area[cy, cx]
                    better = ba is None or a < ba  # the receiver: the smallest larger neighbour
                if ok and better:
                    best, ba = (ny, nx), a
        if best is None:
            break
        path.append(best)
    return path


def line_for(area, y, x, half_m):
    up = downstream_path(area, y, x, 8, True)
    dn = downstream_path(area, y, x, 8, False)
    (uy, ux), (dy_, dx_) = up[-1], dn[-1]
    vx, vy = dx_ - ux, dy_ - uy
    nrm = math.hypot(vx, vy) or 1.0
    vx, vy = vx / nrm, vy / nrm
    px, py = -vy, vx  # normal
    hc = half_m / CELL
    return (x - px * hc, y - py * hc, x + px * hc, y + py * hc)


def sample(z, x0, y0, x1, y1, step_cells=0.5):
    n = int(math.hypot(x1 - x0, y1 - y0) / step_cells) + 1
    t = np.linspace(0, 1, n)
    xs, ys = x0 + (x1 - x0) * t, y0 + (y1 - y0) * t
    h, w = z.shape
    xs, ys = np.clip(xs, 0, w - 1.001), np.clip(ys, 0, h - 1.001)
    xi, yi = np.floor(xs).astype(int), np.floor(ys).astype(int)
    fx, fy = xs - xi, ys - yi
    v = (z[yi, xi] * (1 - fx) * (1 - fy) + z[yi, xi + 1] * fx * (1 - fy) + z[yi + 1, xi] * (1 - fx) * fy + z[yi + 1, xi + 1] * fx * fy)
    dist = (t - 0.5) * math.hypot(x1 - x0, y1 - y0) * CELL
    return dist, v


def ring_depth(z, y, x):
    r = z[max(0, y - 4):y + 5, max(0, x - 4):x + 5]
    m = np.ones_like(r, dtype=bool)
    cy, cx = min(4, y), min(4, x)
    m[max(0, cy - 1):cy + 2, max(0, cx - 1):cx + 2] = False
    return float(np.median(r[m]) - z[y, x])


def candidates(z, area, central=0.6, spacing_cells=59):
    h, w = z.shape
    mh, mw = int(h * (1 - central) / 2), int(w * (1 - central) / 2)
    sub = area[mh:h - mh, mw:w - mw]
    zz = z[mh:h - mh, mw:w - mw]
    ys, xs = np.nonzero((sub >= THR) & (zz > 0))
    ys, xs = ys + mh, xs + mw
    # thin: one candidate per spacing x spacing block, the cell of largest area in it
    keep = {}
    for y, x in zip(ys, xs):
        k = (y // spacing_cells, x // spacing_cells)
        if k not in keep or area[y, x] > area[keep[k]]:
            keep[k] = (y, x)
    return sorted(keep.values())


def pick(win):
    z, ox, oy = load(win, '')
    area = area_of(win)
    face, i0, i1, j0, j1, seed, d, stem, kind, label, pool = WIN[win]
    cand = candidates(z, area)
    rows = []
    t1 = max(cand, key=lambda c: (area[c], -c[0], -c[1]))
    half1 = 10000.0 if area[t1] >= 1e11 else 5000.0
    rows.append(('T1', t1, half1))
    for tid, target in BANDS.items():
        band = [c for c in cand if 0.5 * target <= area[c] <= 2 * target]
        if not band:
            continue
        depths = sorted((ring_depth(z, c[0], c[1]), c) for c in band)
        rows.append((tid, depths[len(depths) // 2][1], 5000.0))
    out = []
    for tid, (y, x), half in rows:
        x0, y0, x1, y1 = line_for(area, y, x, half)
        dist, v = sample(z, x0, y0, x1, y1)
        out.append(dict(window=win, id=tid, face=face, seed=seed, gx0=round(ox + x0, 2), gy0=round(oy + y0, 2), gx1=round(ox + x1, 2), gy1=round(oy + y1, 2),
                        river_gx=ox + x, river_gy=oy + y, area_km2=round(area[y, x] / 1e6, 1), area_scope=('window' if kind == 'window' else 'core only'),
                        relief_m=round(float(v.max() - v.min()), 1), half_km=half / 1000))
    rows_old = []
    if os.path.exists(CSV):
        with open(CSV) as f:
            rows_old = [r for r in csv.DictReader(f) if r['window'] != win]
    keys = list(out[0].keys())
    with open(CSV, 'w', newline='') as f:
        wr = csv.DictWriter(f, fieldnames=keys)
        wr.writeheader()
        for r in rows_old + out:
            wr.writerow(r)
    for r in out:
        print(r)


def dem_match(pool, tid, area_target, relief_target, half_m):
    best = None
    for tile in POOL[pool]:
        sz = open(BENCH + 'sizes.txt').read().split('\n')
        ent = [l.split() for l in sz if l.startswith('glo-' + tile + ' ')]
        if not ent:
            continue
        w, h = int(ent[0][1]), int(ent[0][2])
        z = np.fromfile(BENCH + f'glo-{tile}.f64', '<f8').reshape(h, w)
        area = np.fromfile(BENCH + f'glo-{tile}.area.f32', '<f4').astype(np.float64).reshape(h, w)
        cand = candidates(z, area, central=0.8, spacing_cells=30)
        if tid == 'T1':
            # No 1-degree tile drains a trunk's area (largest 640-8,193 km2): T1 matches the tile's large rivers, area >= min(half the target, 2,000 km2).
            band = [c for c in cand if area[c] >= min(0.5 * area_target, 2000e6)]
        else:
            band = [c for c in cand if 0.5 * area_target <= area[c] <= 2 * area_target]
        for (y, x) in band:
            x0, y0, x1, y1 = line_for(area, y, x, half_m)
            dist, v = sample(z, x0, y0, x1, y1)
            rel = float(v.max() - v.min())
            key = (abs(rel - relief_target), tile, y * w + x)
            if best is None or key < best[0]:
                best = (key, tile, dist, v - z[y, x], rel, area[y, x])
    return best


def plot(win, out_png):
    face, i0, i1, j0, j1, seed, d, stem, kind, label, pool = WIN[win]
    with open(CSV) as f:
        rows = [r for r in csv.DictReader(f) if r['window'] == win]
    surf = {}
    for var, name, col in VAR:
        try:
            surf[var] = load(win, var)
        except FileNotFoundError:
            pass
    z0, ox, oy = surf['']
    profiles = []
    for r in rows:
        x0, y0, x1, y1 = float(r['gx0']) - ox, float(r['gy0']) - oy, float(r['gx1']) - ox, float(r['gy1']) - oy
        rx, ry = int(float(r['river_gx']) - ox), int(float(r['river_gy']) - oy)
        base = z0[ry, rx]
        lines = []
        for var, name, col in VAR:
            if var in surf:
                dist, v = sample(surf[var][0], x0, y0, x1, y1)
                lines.append((name, col, dist, v - base))
        half = float(r['half_km']) * 1000
        dm = dem_match(pool, r['id'], float(r['area_km2']) * 1e6, float(r['relief_m']), half)
        if dm:
            print(f"MATCH {win} {r['id']} GLO-30 {dm[1]} relief {dm[4]:.1f} m area {dm[5] / 1e6:.0f} km2")
        else:
            print(f"MATCH {win} {r['id']} none")
        profiles.append((r, lines, dm))
    span = 0.0
    for r, lines, dm in profiles:
        allv = np.concatenate([l[3] for l in lines] + ([dm[3]] if dm else []))
        span = max(span, float(allv.max() - allv.min()))
    span *= 1.1
    fig = plt.figure(figsize=(16, 4.2 * len(profiles) + 1))
    gs = fig.add_gridspec(len(profiles), 3, width_ratios=[1.2, 1, 1])
    axm = fig.add_subplot(gs[:, 0])
    step = max(1, z0.shape[0] // 1000)
    zz = z0[::step, ::step]
    gy, gx = np.gradient(zz, CELL * step)
    hs = np.clip((-gx * 0.5 + gy * 0.5 + 1) / np.sqrt(gx * gx + gy * gy + 1), 0, 1.2)
    axm.imshow(hs, cmap='gray', origin='lower', extent=[0, z0.shape[1] * CELL / 1000, 0, z0.shape[0] * CELL / 1000])
    for r, lines, dm in profiles:
        x0, y0, x1, y1 = [(float(r[k]) - (ox if 'x' in k else oy)) * CELL / 1000 for k in ('gx0', 'gy0', 'gx1', 'gy1')]
        axm.plot([x0, x1], [y0, y1], 'r-', lw=2)
        axm.text(x1, y1, ' ' + r['id'], color='red', fontsize=11, weight='bold')
    axm.set_title(f'{label}: face {face}, regional i {i0}-{i1}, j {j0}-{j1}, seed {seed}\ntoday surface, hillshade; transects in red', fontsize=9)
    axm.set_xlabel('km (face u)'); axm.set_ylabel('km (face v)')
    for k, (r, lines, dm) in enumerate(profiles):
        ax = fig.add_subplot(gs[k, 1:])
        for name, col, dist, v in lines:
            ax.plot(dist / 1000, v, color=col, lw=1.4, label=name)
        if dm:
            key, tile, dd, dv, rel, ar = dm
            ax.plot(dd / 1000, dv, 'k--', lw=1.4, label=f'GLO-30 {tile} (relief {rel:.0f} m, area {ar / 1e6:.0f} km2)')
        allv = np.concatenate([l[3] for l in lines] + ([dm[3]] if dm else []))
        mid = 0.5 * (allv.max() + allv.min())
        ax.set_ylim(mid - span / 2, mid + span / 2)
        half = float(r['half_km'])
        ax.set_xlim(-half, half)
        ax.axvline(0, color='#bbbbbb', lw=0.8)
        ex = (2 * half * 1000 / ax.get_position().width / fig.get_figwidth()) / (span / ax.get_position().height / fig.get_figheight())
        ax.set_title(f"{r['id']}: drained area {r['area_km2']} km2 ({r['area_scope']}), relief along the line {r['relief_m']} m on today's surface; "
                     f"vertical exaggeration about x{ex:.1f}", fontsize=9)
        ax.set_xlabel('km across the river (0 = the river cell)'); ax.set_ylabel("m, relative to today's river cell")
        ax.legend(fontsize=7, loc='upper center', ncol=3)
        ax.grid(alpha=0.3)
    fig.suptitle(f'{label}: river cross-sections, every variant (start surfaces from the drain_base probe, not baked) and a GLO-30 valley at matched relief; '
                 f'one vertical scale ({span:.0f} m a panel)', fontsize=11)
    fig.text(0.01, 0.003, 'GLO-30: produced using Copernicus WorldDEM-30 © DLR e.V. 2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under COPERNICUS by the European Union '
             'and ESA; all rights reserved. The organisations in charge of the Copernicus programme by law or by delegation do not incur any liability for any use of the Copernicus WorldDEM-30.',
             fontsize=6)
    fig.tight_layout(rect=[0, 0.015, 1, 0.97])
    fig.savefig(out_png, dpi=80)
    print('wrote', out_png)


if __name__ == '__main__':
    if sys.argv[1] == 'pick':
        pick(sys.argv[2])
    elif sys.argv[1] == 'plot':
        plot(sys.argv[2], sys.argv[3])
