"""SPIKE-127: exception maps. Hillshade of S0 (the E3 start surface dump) over the region's box at one pixel per H1 cell, the trunks of today's H1 and of K6'
(sills of S0), and the rise points of the reaches that fail the corridor bar (k_c 2 snapped, land-only snap): on S0 (circles) and on B's store (crosses).

Usage: g1a_map.py <window> <s0.f64> <g1a out dir> <out.png>
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
SEED = {'r1h': ('1', 14), 'r3a': ('4', 15), 'r3b': ('2', 15), 'm1': ('4', 15), 'f1': ('4', 15)}


def main():
    w, s0p, d, out = sys.argv[1:5]
    i0, j0 = ORIG[w]
    x0, y0 = (i0 - 200) * 8, (j0 - 200) * 8
    z = np.fromfile(s0p, '<f8').reshape(N, N)
    zc = z.reshape(N // 8, 8, N // 8, 8).min(axis=(1, 3))  # node heights (lowest L cell)
    zc = zc[200:820, 200:820]
    ls = LightSource(azdeg=315, altdeg=35)
    shade = ls.hillshade(np.maximum(zc, 0.0), vert_exag=0.02, dx=814, dy=814)
    sea = zc <= 0
    lines = {}
    with open(f'{d}/{w}-trunks.csv') as f:
        for r in csv.DictReader(f):
            lines.setdefault((r['graph'], int(r['reach'])), []).append((int(r['x']) / 8 - 200, int(r['y']) / 8 - 200, float(r['q_m3s'])))
    fails = {}
    with open(f'{d}/{w}-reaches.csv') as f:
        for r in csv.DictReader(f):
            if r['failed'] == 'true':
                fails.setdefault((r['graph'], r['surface']), []).append(((int(r['x']) - x0) / 8 - 200, (int(r['y']) - y0) / 8 - 200))
    fig, axes = plt.subplots(1, 2, figsize=(13, 6.8), dpi=80)
    for ax, g, title in ((axes[0], 'today', "today's H1 (K6 on z_s at cell centres)"), (axes[1], "K6'", "K6' (node heights and sills of S0)")):
        ax.imshow(shade, cmap='gray', origin='upper', extent=(-0.5, 619.5, 619.5, -0.5), vmin=0, vmax=1)
        ax.imshow(np.ma.masked_where(~sea, sea), cmap='Blues', alpha=0.5, origin='upper', extent=(-0.5, 619.5, 619.5, -0.5), vmin=0, vmax=1.5)
        for (gg, _), pts in lines.items():
            if gg != g:
                continue
            p = np.array(pts)
            lw = 0.4 + 0.5 * np.log10(max(p[:, 2].max(), 1.0))
            ax.plot(p[:, 0], p[:, 1], color='#1f5fbf', lw=lw, alpha=0.9)
        for surf, mk, col in (('S0', 'o', '#d62728'), ('B', 'x', '#ff7f0e')):
            p = np.array(fails.get((g, surf), [])).reshape(-1, 2)
            ax.scatter(p[:, 0], p[:, 1], s=10 if mk == 'o' else 28, marker=mk, facecolors='none' if mk == 'o' else col, edgecolors=col if mk == 'o' else None,
                       lw=0.8 if mk == 'o' else 1.4, label=f'fails on {surf} ({len(p)})')
        ax.set_xlim(-0.5, 619.5)
        ax.set_ylim(619.5, -0.5)
        ax.set_title(f'{w.upper()}: {title}', fontsize=10)
        ax.set_xticks([])
        ax.set_yticks([])
        ax.legend(loc='lower left', fontsize=8, framealpha=0.8)
    fig.suptitle(f'{w.upper()} (face {SEED[w][0]}, seed {SEED[w][1]}): region box 505 km wide, one pixel per 814 m cell; trunks q >= 1 m3/s (width by Q); '
                 'circles: rise points of reaches over the bar on S0, crosses: on B', fontsize=9)
    fig.tight_layout(rect=(0, 0, 1, 0.96))
    fig.savefig(out, dpi=80)
    print('wrote', out)


if __name__ == '__main__':
    main()
