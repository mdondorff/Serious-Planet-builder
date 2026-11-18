"""SPIKE-136 addendum (spikes #137): per window, two images. Analysis renders (CPU), not the production path.
- ridge: the H1 quads KH splits along a D8 step that put S0 ridge cells (contract section 9) within 1 L cell of the split diagonal, coloured by how far KH + r lies under S0 there
  (over 10 m orange, over 50 m red; other stepped quads grey), over a low-sun hillshade of S0 (SPIKE-136's block means, 4 L cells a pixel) of the region box.
- facets: the crop of the box with the largest p50 1 km relief, every L cell a pixel, hillshade at sun altitude 10 degrees, azimuth 315: C1(z_s) + r (S0) left, KH(z_s) + r right.
Usage: kh_sheets.py <ridge dir> <SPIKE-136 dir with <tag>-s0-1240x1240.f32> <tag> <window name> <out prefix>
"""
import sys
import numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib.colors import LightSource, to_rgb
from matplotlib.patches import Patch

L_M = 101.75


def scale_bar(ax, n, px_m, km, colour='black'):
    bar = km * 1000 / px_m
    x0, y0 = n * 0.05, n * 0.95
    ax.plot([x0, x0 + bar], [y0, y0], color=colour, lw=4)
    ax.text(x0 + bar / 2, y0 - n * 0.015, f'{km} km', ha='center', va='bottom', fontsize=9, bbox=dict(facecolor='white', alpha=0.7, lw=0))


def ridge_map(rd, hd, tag, name, out):
    cx, cy, crop, rw, rh = map(int, open(f'{rd}/{tag}-crop.txt').read().strip().split(','))
    cut = np.fromfile(f'{rd}/{tag}-ridgecut-{rw}x{rh}.f32', '<f4').reshape(rh, rw)
    stepped = np.fromfile(f'{rd}/{tag}-stepped-{rw}x{rh}.u8', np.uint8).reshape(rh, rw)
    z = np.fromfile(f'{hd}/{tag}-s0-1240x1240.f32', '<f4').reshape(1240, 1240).astype(float)
    m = (rw - 620) // 2
    px = 4 * L_M
    shade = LightSource(azdeg=315, altdeg=20).hillshade(np.maximum(z, 0.0), vert_exag=2.0, dx=px, dy=px)
    rgb = np.dstack([shade] * 3) * 0.85 + 0.15
    rgb[z <= 0] = [0.95, 0.93, 0.86]
    box = lambda a: np.repeat(np.repeat(a[m:m + 620, m:m + 620], 2, axis=0), 2, axis=1)
    st, c = box(stepped) > 0, box(cut)
    rgb[st & (c <= 10)] = rgb[st & (c <= 10)] * 0.6 + 0.4 * np.array(to_rgb('#1f78b4'))
    rgb[c > 10] = to_rgb('#ff9f1c')
    rgb[c > 50] = to_rgb('#e31a1c')
    fig, ax = plt.subplots(figsize=(10.4, 10.6), dpi=120)
    ax.imshow(rgb, interpolation='nearest')
    ax.set_xticks([])
    ax.set_yticks([])
    scale_bar(ax, 1240, px, 20)
    # the facet crop's frame
    fx, fy = (cx - m * 8) / 4, (cy - m * 8) / 4
    ax.add_patch(plt.Rectangle((fx, fy), crop / 4, crop / 4, fill=False, ec='black', lw=1.0, ls='--'))
    n10, n50 = int(((cut[m:m + 620, m:m + 620]) > 10).sum()), int(((cut[m:m + 620, m:m + 620]) > 50).sum())
    ax.legend(handles=[Patch(color='#1f78b4', alpha=0.5, label=f'quad split along a D8 step ({int(stepped[m:m + 620, m:m + 620].sum())} quads)'),
                       Patch(color='#ff9f1c', label=f'S0 ridge cut over 10 m on the split diagonal ({n10} quads)'),
                       Patch(color='#e31a1c', label=f'... over 50 m ({n50} quads)'),
                       Patch(fill=False, ec='black', ls='--', label='the facet render\'s crop')], loc='upper right', fontsize=8, framealpha=0.85)
    ax.set_title(f'{name}: KH\'s D8-step splits against S0\'s ridges; region box, 2 px an 814 m quad. Analysis render, not the production path', fontsize=10)
    fig.tight_layout()
    fig.savefig(out, dpi=120, pil_kwargs={'quality': 80})
    print('wrote', out)


def facets(rd, tag, name, out):
    cx, cy, crop, rw, rh = map(int, open(f'{rd}/{tag}-crop.txt').read().strip().split(','))
    a = np.fromfile(f'{rd}/{tag}-crop-s0-{crop}.f32', '<f4').reshape(crop, crop).astype(float)
    b = np.fromfile(f'{rd}/{tag}-crop-khr-{crop}.f32', '<f4').reshape(crop, crop).astype(float)
    ls = LightSource(azdeg=315, altdeg=10)
    fig, axes = plt.subplots(1, 2, figsize=(16, 8.6), dpi=100)
    for ax, z, title in [(axes[0], a, 'C1(z_s) + r (S0, today)'), (axes[1], b, 'KH(z_s) + r (h2, flow-diagonal triangulation)')]:
        sh = ls.hillshade(np.maximum(z, 0.0), vert_exag=1.0, dx=L_M, dy=L_M)
        rgb = np.dstack([sh] * 3)
        rgb[z <= 0] = [0.95, 0.93, 0.86]
        ax.imshow(rgb, interpolation='nearest', vmin=0, vmax=1)
        ax.set_xticks([])
        ax.set_yticks([])
        scale_bar(ax, crop, L_M, 10)
        ax.set_title(title, fontsize=11)
    km = crop * L_M / 1000
    fig.suptitle(f'{name}: {km:.0f} km crop, 1 px an L cell (101.8 m), sun altitude 10 deg from the NW, no vertical exaggeration. Analysis render, not the production path', fontsize=11)
    fig.tight_layout(rect=(0, 0, 1, 0.95))
    fig.savefig(out, dpi=100, pil_kwargs={'quality': 85})
    print('wrote', out)


if __name__ == '__main__':
    rd, hd, tag, name, prefix = sys.argv[1:6]
    ridge_map(rd, hd, tag, name, f'{prefix}kh-ridge-{tag}.jpg')
    facets(rd, tag, name, f'{prefix}kh-facets-{tag}.jpg')
