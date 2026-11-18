"""SPIKE-138 (spikes #143, #145): one sheet per window from kh_smooth's --dump-f surfaces (8160 x 8160 f32, the window's L cells). #137's crop of the region box (512 x 512 L
cells), 1 px an L cell, hillshade at sun altitude 10 degrees from the north-west, no vertical exaggeration. Panels: today's S0; F = KH(h4c) + r' - KS1 on today's base (#141);
F on the new base (sel-post, A+ band); F on the new base with reading 23 post. Analysis render (CPU), not the production path.
Usage: s138_sheet.py <today_dir> <new_dir> <tag> <window name> <cx,cy> <out.jpg> [<new tag> <label>] (the new base's tag and its label, e.g. m1-bc 'band drain capped at 50 m')
"""
import sys
import numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib.colors import LightSource

L_M = 101.75
N = 8160
CROP = 512


def crop(path, cx, cy):
    z = np.memmap(path, dtype='<f4', mode='r', shape=(N, N))
    return np.array(z[cy:cy + CROP, cx:cx + CROP], dtype=float)


def main():
    td, nd, tag, name, at, out = sys.argv[1:7]
    nt, lab = (sys.argv[7], ', ' + sys.argv[8]) if len(sys.argv) > 8 else (tag, '')
    cx, cy = (int(v) for v in at.split(','))
    panels = [(f'{td}/{tag}-today-S0-{N}x{N}.f32', "S0 = C1(z_s) + r (today)"),
              (f'{td}/{tag}-today-F-{N}x{N}.f32', "Today's base: KH(h4c) + r' - KS1 (#141)"),
              (f'{nd}/{nt}-F-{N}x{N}.f32', f"New base (sel-post) + A+ band{lab}: KH + r'' - KS1"),
              (f'{nd}/{nt}-r23p-F-{N}x{N}.f32', "The same with reading 23 (drain after KS, caps = budget)")]
    ls = LightSource(azdeg=315, altdeg=10)
    fig, axes = plt.subplots(2, 2, figsize=(14, 14.6), dpi=100)
    for ax, (path, title) in zip(axes.flat, panels):
        z = crop(path, cx, cy)
        sh = ls.hillshade(np.maximum(z, 0.0), vert_exag=1.0, dx=L_M, dy=L_M)
        rgb = np.dstack([sh] * 3)
        rgb[z <= 0] = [0.95, 0.93, 0.86]
        ax.imshow(rgb, interpolation='nearest', vmin=0, vmax=1)
        ax.set_xticks([])
        ax.set_yticks([])
        bar = 10_000 / L_M
        x0, y0 = CROP * 0.05, CROP * 0.95
        ax.plot([x0, x0 + bar], [y0, y0], color='black', lw=4)
        ax.text(x0 + bar / 2, y0 - CROP * 0.015, '10 km', ha='center', va='bottom', fontsize=9, bbox=dict(facecolor='white', alpha=0.7, lw=0))
        ax.set_title(title, fontsize=11)
    fig.suptitle(f'{name}: {CROP * L_M / 1000:.0f} km crop, 1 px an L cell (101.8 m), sun altitude 10 deg from the NW, no vertical exaggeration.\nAnalysis render, not the production path',
                 fontsize=12)
    fig.tight_layout(rect=(0, 0, 1, 0.95), h_pad=2.5)
    fig.savefig(out, dpi=100, pil_kwargs={'quality': 85})
    print('wrote', out)


if __name__ == '__main__':
    main()
