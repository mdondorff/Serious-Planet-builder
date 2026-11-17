"""SPIKE-129: exception maps. Hillshade of S0 over the region's box (one pixel per 814 m cell, the lowest L cell), the L trunk images (lines, every second
vertex; exceptional reaches orange) and the rise_over_budget markers (red). Usage: kg_map.py <window> <s0.f64> <kg out dir> <tag> <out.png>
"""
import sys
import csv
import numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib.colors import LightSource

N = 8160
ORIG = {'r1h': (8056, 760), 'r3a': (368, 4720), 'r3b': (9968, 8304), 'm1': (6600, 10200), 'f1': (4800, 3300)}
FACE = {'r1h': (1, 14), 'r3a': (4, 15), 'r3b': (2, 15), 'm1': (4, 15), 'f1': (4, 15)}


def main():
    w, s0p, d, tag, out = sys.argv[1:6]
    i0, j0 = ORIG[w]
    x0, y0 = (i0 - 200) * 8, (j0 - 200) * 8
    z = np.fromfile(s0p, '<f8').reshape(N, N)
    zc = z.reshape(N // 8, 8, N // 8, 8).min(axis=(1, 3))[200:820, 200:820]
    shade = LightSource(azdeg=315, altdeg=35).hillshade(np.maximum(zc, 0.0), vert_exag=0.02, dx=814, dy=814)
    fig, ax = plt.subplots(figsize=(8.2, 8.4), dpi=80)
    ax.imshow(shade, cmap='gray', origin='upper', extent=(-0.5, 619.5, 619.5, -0.5), vmin=0, vmax=1)
    ax.imshow(np.ma.masked_where(zc > 0, zc <= 0), cmap='Blues', alpha=0.5, origin='upper', extent=(-0.5, 619.5, 619.5, -0.5), vmin=0, vmax=1.5)
    lines = {}
    with open(f'{d}/{tag}-lines.csv') as f:
        for r in csv.DictReader(f):
            lines.setdefault(int(r['reach']), []).append(((int(r['x']) - x0) / 8 - 200, (int(r['y']) - y0) / 8 - 200, float(r['q_w']), r['exception'] == '1'))
    n_exc = 0
    for pts in lines.values():
        p = np.array([(a, b, c) for a, b, c, _ in pts])
        exc = pts[0][3]
        n_exc += exc
        lw = 0.3 + 0.4 * np.log10(max(p[:, 2].max(), 1.0))
        ax.plot(p[:, 0], p[:, 1], color='#ff9f1c' if exc else '#1f5fbf', lw=lw, alpha=0.9)
    xs, ys = [], []
    with open(f'{d}/{tag}-exceptions.csv') as f:
        for r in csv.DictReader(f):
            xs.append((int(r['x']) - x0) / 8 - 200)
            ys.append((int(r['y']) - y0) / 8 - 200)
    ax.scatter(xs, ys, s=6, c='#d62728', lw=0, label=f'rise_over_budget markers ({len(xs)})')
    ax.plot([], [], color='#1f5fbf', label=f'L trunk images ({len(lines) - n_exc})')
    ax.plot([], [], color='#ff9f1c', label=f'exceptional reaches ({n_exc})')
    ax.set_xlim(-0.5, 619.5)
    ax.set_ylim(619.5, -0.5)
    ax.set_xticks([])
    ax.set_yticks([])
    ax.legend(loc='lower left', fontsize=8, framealpha=0.85)
    base = "today's K6" if tag.endswith('today') else "K6'"
    ax.set_title(f'{w.upper()} (face {FACE[w][0]}, seed {FACE[w][1]}): KG-trunk on {base} trunks, region box 505 km; lowest-path centrelines on KS0', fontsize=9)
    fig.tight_layout()
    fig.savefig(out, dpi=80)
    print('wrote', out)


if __name__ == '__main__':
    main()
