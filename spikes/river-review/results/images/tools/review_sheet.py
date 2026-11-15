"""River review (lead #118): one top-down contact sheet per window.
Rows: 40 km and 10 km crops. Columns: five variants (start surfaces, no bake) and GLO-30.
Hillshade at full L resolution (101.8 m), river cells of the L route (area >= 75 km2) in blue.
Usage: review_sheet.py <window: r1h|r3a|r3b> <out.png>
"""
import sys, heapq
import numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib.patches import Rectangle

T = 'C:/DEV/Serious-Planet-Builder/.claude/worktrees/sonnet-spikes-36fc88/target/river-review/'
DEMF = 'C:/DEV/Serious-Planet-Builder/.claude/worktrees/sonnet-spikes-36fc88/target/spike116/sheet/dem-n40w078.f32'
CELL = 101.8
N = 4960
WIN = {
    'r1h': ('R1h: face 1, seed 14, regional cells i 8056-8675, j 760-1379 (the E1 region; stores r3a-e1, r3a-p2e)', 'f1-8056-760-none'),
    'r3a': ('R3a: face 4, seed 15, SPIKE-111 window i 168-1187, j 4520-5539; shown: its 620-cell core i 368-987, j 4720-5339', 'f4-368-4720-none'),
    'r3b': ('R3b: face 2, seed 15, SPIKE-111 window i 9768-10787, j 8104-9123; shown: its 620-cell core i 9968-10587, j 8304-8923', 'f2-9968-8304-none'),
}
VAR = [
    ('', 'today\n(B + regional)'),
    ('-round', 'one round at L only'),
    ('-td4', '4-neighbour drain only\n(SPIKE-116 hand-over)'),
    ('-td4-round', '4-neighbour drain\n+ one round at L'),
    ('-td4-schott', 'Schott 2024 fair\n(SPIKE-118, 2 rounds a scale)'),
]
THR = 75e6
EX = 1.0  # m2, the initiation area (HydroParams::default)


def shade(z, cell, az=315.0, alt=45.0, ex=1.0):
    gy, gx = np.gradient(z.astype(np.float64) * ex, cell)
    a, e = np.radians(az), np.radians(alt)
    # light from azimuth az (clockwise from north = up), elevation alt; rows run north-up after the flip below
    lx, ly, lz = np.sin(a) * np.cos(e), -np.cos(a) * np.cos(e), np.sin(e)  # image rows run south
    nx, ny, nz = -gx, -gy, np.ones_like(gx)
    s = (nx * lx + ny * ly + nz * lz) / np.sqrt(nx * nx + ny * ny + 1.0)
    return np.clip(s, 0, 1)


def load(tag):
    """The core of a dump: a core dump as is, a whole-window dump (8160 cells, margin 200 regional cells = 1600 L cells) cropped to the core."""
    import os
    if os.path.exists(T + tag + '-full-8160x8160.f32'):
        z = np.fromfile(T + tag + '-full-8160x8160.f32', dtype='<f4').reshape(8160, 8160)[1600:1600 + N, 1600:1600 + N]
        m = np.fromfile(T + tag + '-chan-8160x8160.u8', dtype=np.uint8).reshape(8160, 8160)[1600:1600 + N, 1600:1600 + N]
        return np.ascontiguousarray(z), np.ascontiguousarray(m)
    z = np.fromfile(T + tag + '-full-4960x4960.f32', dtype='<f4').reshape(N, N)
    m = np.fromfile(T + tag + '-chan-4960x4960.u8', dtype=np.uint8).reshape(N, N)
    return z, m


def d8_rivers(z, cell, thr):
    """Rivers on a DEM crop by the same rule: priority flood (crop edge as outlet), D8 steepest descent on the filled surface, area >= thr."""
    h, w = z.shape
    f = z.astype(np.float64).copy()
    done = np.zeros((h, w), bool)
    pq = []
    for y in range(h):
        for x in (0, w - 1):
            heapq.heappush(pq, (f[y, x], y, x)); done[y, x] = True
    for x in range(1, w - 1):
        for y in (0, h - 1):
            heapq.heappush(pq, (f[y, x], y, x)); done[y, x] = True
    order = []
    rec = np.full((h, w, 2), -1, np.int32)
    nb = [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)]
    while pq:
        v, y, x = heapq.heappop(pq)
        order.append((y, x))
        for dy, dx in nb:
            yy, xx = y + dy, x + dx
            if 0 <= yy < h and 0 <= xx < w and not done[yy, xx]:
                done[yy, xx] = True
                f[yy, xx] = max(f[yy, xx], v + 1e-3)
                rec[yy, xx] = (y, x)  # flood parent: drains to it
                heapq.heappush(pq, (f[yy, xx], yy, xx))
    area = np.full((h, w), cell * cell)
    for y, x in reversed(order):
        ry, rx = rec[y, x]
        if ry >= 0:
            area[ry, rx] += area[y, x]
    return area >= thr


def overlay(ax, z, m, cell, title, box=None, ex=1.0):
    s = shade(z, cell, ex=ex)
    rgb = np.dstack([s, s, s]) * 0.9 + 0.05
    sea = z <= 0
    rgb[sea] = (0.55, 0.65, 0.78)
    rgb[m.astype(bool)] = (0.05, 0.25, 0.95)
    ax.imshow(rgb, origin='upper', interpolation='nearest')
    ax.set_title(title, fontsize=7)
    ax.set_xticks([]); ax.set_yticks([])
    if box is not None:
        y, x, n = box
        ax.add_patch(Rectangle((x - 0.5, y - 0.5), n, n, fill=False, ec='red', lw=0.8))
    # scale bar: 5 km on the 40 km crops, 2 km on the 10 km crops
    km = 5 if z.shape[0] * cell > 20000 else 2
    px = km * 1000 / cell
    h = z.shape[0]
    ax.plot([0.05 * h, 0.05 * h + px], [0.94 * h, 0.94 * h], color='white', lw=3)
    ax.plot([0.05 * h, 0.05 * h + px], [0.94 * h, 0.94 * h], color='black', lw=1.2)
    ax.text(0.05 * h, 0.90 * h, '%d km' % km, fontsize=6, color='black', bbox=dict(fc='white', ec='none', pad=0.5))


def pick(z, m, n, step, within=None):
    """The crop (row, col) with the largest height std over at least 95 % land, preferring crops with rivers; inside `within` = (y, x, size) when given."""
    best, at = -1.0, (0, 0)
    y0, x0, sz = within if within else (0, 0, N)
    for y in range(y0, y0 + sz - n + 1, step):
        for x in range(x0, x0 + sz - n + 1, step):
            c = z[y:y + n, x:x + n]
            if (c > 0).mean() < 0.95:
                continue
            v = float(c.std()) * (1.0 + 300.0 * float(m[y:y + n, x:x + n].mean()))
            if v > best:
                best, at = v, (y, x)
    return at


def main():
    win, out = sys.argv[1], sys.argv[2]
    title, base = WIN[win]
    n40, n10 = int(round(40000 / CELL)), int(round(10000 / CELL))
    z0, m0 = load(base)
    # Rows are written south to north (y grows with the face's t); flip so north is up in every panel.
    flip = lambda a: a[::-1]
    y40, x40 = pick(z0, m0, n40, n40 // 4)
    # One vertical exaggeration per window for all five variants (GLO-30 stays x1): the slope p90 of today's 40 km crop raised to GLO-30's, rounded to 1, 2, 5 or 10.
    global EX
    gy, gx = np.gradient(z0[y40:y40 + n40, x40:x40 + n40].astype(np.float64), CELL)
    p90 = float(np.percentile(np.hypot(gx, gy), 90))
    EX = next((e for e in (1, 2, 5, 10, 20) if p90 * e >= 0.25), 20)
    y10, x10 = pick(z0, m0, n10, n10 // 4, (y40, x40, n40))
    dem = np.fromfile(DEMF, dtype='<f4').reshape(3600, 3600).astype(np.float64)
    # GLO-30 N40 W078 at 1 arcsec: ~30.9 m N-S, ~23.7 m E-W at 40.5 N; steps 3 and 4 give ~93-95 m, near L's 101.8 m.
    dl = dem[::3, ::4][:1200, :900]
    dcell = 94.0
    nd40, nd10 = int(40000 / dcell), int(10000 / dcell)
    dy40, dx40 = 150, 100
    d40 = dl[dy40:dy40 + nd40, dx40:dx40 + nd40]
    r40 = d8_rivers(d40, dcell, THR)
    best, at = -1, (0, 0)
    for y in range(0, nd40 - nd10, nd10 // 4):
        for x in range(0, nd40 - nd10, nd10 // 4):
            v = d40[y:y + nd10, x:x + nd10].std()
            if v > best:
                best, at = v, (y, x)
    d10 = d40[at[0]:at[0] + nd10, at[1]:at[1] + nd10]
    fig, ax = plt.subplots(2, 6, figsize=(21, 7.6))
    for k, (suf, lab) in enumerate(VAR):
        try:
            z, m = (z0, m0) if suf == '' else load(base + suf)
        except FileNotFoundError:
            for r in (0, 1):
                ax[r, k].text(0.5, 0.5, 'missing ' + suf, ha='center'); ax[r, k].axis('off')
            continue
        a40 = flip(z[y40:y40 + n40, x40:x40 + n40]); b40 = flip(m[y40:y40 + n40, x40:x40 + n40])
        a10 = flip(z[y10:y10 + n10, x10:x10 + n10]); b10 = flip(m[y10:y10 + n10, x10:x10 + n10])
        box = (n40 - (y10 - y40) - n10, x10 - x40, n10)
        overlay(ax[0, k], a40, b40, CELL, '%s\n40 km, start surface, no bake, z %.0f-%.0f m, height x%g' % (lab, a40.min(), a40.max(), EX), box, EX)
        overlay(ax[1, k], a10, b10, CELL, '10 km (red box above), z %.0f-%.0f m, height x%g' % (a10.min(), a10.max(), EX), None, EX)
    overlay(ax[0, 5], d40, r40, dcell, 'GLO-30 N40 W078 (Appalachians), ~94 m\n40 km, rivers by the same rule, z %.0f-%.0f m' % (d40.min(), d40.max()), (at[0], at[1], nd10))
    overlay(ax[1, 5], d10, r40[at[0]:at[0] + nd10, at[1]:at[1] + nd10], dcell, 'GLO-30, 10 km, z %.0f-%.0f m' % (d10.min(), d10.max()))
    fig.suptitle('%s. The v axis of the face up (north on faces 1 and 2; GLO-30 north up), hillshade (light from the upper left, 45 deg),\nriver cells blue (L route, area >= 75 km2), sea light blue. Heights of the five variants exaggerated x%g (GLO-30 x1). Crops: 40 km at core cells (%d, %d), 10 km at (%d, %d) of the 4960-cell core.\n'
                 'Start surfaces from the drain_base probe at 101.8 m (spike code, NOT the production path, no bake). '
                 'Produced using Copernicus WorldDEM-30 (c) DLR e.V. 2010-2014 and (c) Airbus 2014-2018, provided under COPERNICUS by the EU and ESA.' % (title, EX, x40, y40, x10, y10), fontsize=8)
    fig.tight_layout(rect=[0, 0, 1, 0.91], h_pad=2.5)
    fig.savefig(out, dpi=118)
    print('wrote', out, 'crops', (y40, x40), (y10, x10))


main()
