"""SPIKE-134: stills of the three deepest kept H1 basins on trunks. Per basin: a low-sun hillshade of S0 at L (sun azimuth 315, altitude 10) around the basin,
the basin's H1 cells outlined and the channel chain through it; and the profile of S0 along that chain (sampled every L cell between the H1 cell centres) with
the basin's floor and spill levels. Analysis renders (CPU), not the production path.
Usage: deep_stills.py <window> <s0.f64> <run dir> <tag> <out.png> <title>
"""
import sys
import csv
import numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib.colors import LightSource

N = 8160
L = 8


def main():
    w, s0p, d, tag, out, title = sys.argv[1:7]
    z = np.fromfile(s0p, '<f8').reshape(N, N)
    basins = {}
    with open(f'{d}/{tag}-basins.csv') as f:
        for r in csv.DictReader(f):
            basins[int(r['basin'])] = r
    cells, chains, order = {}, {}, []
    with open(f'{d}/{tag}-deep.csv') as f:
        for r in csv.DictReader(f):
            b, rank = int(r['basin']), int(r['rank'])
            if b not in order:
                order.append(b)
            (cells if r['kind'] == 'cell' else chains).setdefault(b, []).append((int(r['i']), int(r['j'])))
    fig, axes = plt.subplots(len(order), 2, figsize=(12, 4.2 * len(order)), dpi=80, gridspec_kw={'width_ratios': [1, 1.3]})
    axes = np.atleast_2d(axes)
    ls = LightSource(azdeg=315, altdeg=10)
    for row, b in enumerate(order):
        info = basins[b]
        ci = np.array(cells[b])
        ch = np.array(chains.get(b, []))
        allij = np.vstack([ci, ch]) if len(ch) else ci
        i0, i1 = allij[:, 0].min() - 4, allij[:, 0].max() + 4
        j0, j1 = allij[:, 1].min() - 4, allij[:, 1].max() + 4
        span = max(i1 - i0, j1 - j0)
        ic, jc = (i0 + i1) / 2, (j0 + j1) / 2
        i0, i1 = int(max(0, ic - span / 2)), int(min(N // L - 1, ic + span / 2))
        j0, j1 = int(max(0, jc - span / 2)), int(min(N // L - 1, jc + span / 2))
        crop = z[j0 * L:(j1 + 1) * L, i0 * L:(i1 + 1) * L]
        step = max(1, crop.shape[0] // 700)
        cz = crop[::step, ::step]
        shade = ls.hillshade(np.maximum(cz, 0.0), vert_exag=1.0, dx=101.75 * step, dy=101.75 * step)
        ax = axes[row, 0]
        ext = (i0 - 0.5, i1 + 0.5, j1 + 0.5, j0 - 0.5)
        ax.imshow(shade, cmap='gray', extent=ext, vmin=0, vmax=1)
        mask = np.zeros((j1 - j0 + 1, i1 - i0 + 1), bool)
        for i, j in ci:
            if i0 <= i <= i1 and j0 <= j <= j1:
                mask[j - j0, i - i0] = True
        ax.contour(np.arange(i0, i1 + 1), np.arange(j0, j1 + 1), mask.astype(float), levels=[0.5], colors='#1f9bff', linewidths=1.0)
        if len(ch):
            ax.plot(ch[:, 0], ch[:, 1], color='#ff3b30', lw=1.0)
        ax.set_xticks([])
        ax.set_yticks([])
        km = (i1 - i0 + 1) * 0.814
        ax.set_title(f'basin {b}: depth {float(info["depth_m"]):.0f} m, {float(info["area_km2"]):.0f} km2, {info["class"]}, origin {info["origin"]} ({km:.0f} km wide)', fontsize=9)
        # profile of S0 along the chain, every L cell
        ax2 = axes[row, 1]
        if len(ch) >= 2:
            pts = []
            for (a, bb), (c2, d2) in zip(ch[:-1], ch[1:]):
                xa, ya = a * L + 3.5, bb * L + 3.5
                xb, yb = c2 * L + 3.5, d2 * L + 3.5
                n = int(max(abs(xb - xa), abs(yb - ya)))
                for t in range(n):
                    pts.append((xa + (xb - xa) * t / n, ya + (yb - ya) * t / n))
            pts.append((ch[-1][0] * L + 3.5, ch[-1][1] * L + 3.5))
            p = np.array(pts)
            zs = z[np.clip(np.round(p[:, 1]).astype(int), 0, N - 1), np.clip(np.round(p[:, 0]).astype(int), 0, N - 1)]
            dist = np.concatenate([[0], np.cumsum(np.hypot(np.diff(p[:, 0]), np.diff(p[:, 1])))]) * 0.10175
            inb = np.array([((int(round((x - 3.5) / L)), int(round((y - 3.5) / L))) in set(map(tuple, ci))) for x, y in p])
            ax2.plot(dist, zs, color='#333333', lw=0.8, label='S0 along the channel chain')
            ax2.fill_between(dist, zs.min() - 5, zs.max() + 5, where=inb, color='#1f9bff', alpha=0.15, label='inside the H1 basin')
            ax2.axhline(float(info['spill_m']), color='#1f9bff', lw=1.0, ls='--', label=f'H1 spill {float(info["spill_m"]):.0f} m')
            ax2.axhline(float(info['floor_m']), color='#ff9f1c', lw=1.0, ls=':', label=f'H1 floor {float(info["floor_m"]):.0f} m')
            ax2.set_xlabel('distance along the chain, km')
            ax2.set_ylabel('height, m')
            ax2.legend(fontsize=7, loc='best')
        ax2.set_title(f'profile: q_in {float(info["q_in_m3s"]):.1f} m3/s, budget {float(info["budget_m"]):.0f} m, crossed: {info["deep"]}', fontsize=9)
    fig.suptitle(title, fontsize=10)
    fig.tight_layout(rect=(0, 0, 1, 0.97))
    fig.savefig(out, dpi=80)
    print('wrote', out)


if __name__ == '__main__':
    main()
